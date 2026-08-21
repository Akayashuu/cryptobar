//! Cache disque avec TTL, écriture atomique et verrou consultatif.
//!
//! Objectif : quel que soit le nombre de ticks Waybar, l'API n'est appelée qu'une
//! fois par intervalle, et deux processus concurrents ne partent jamais en double
//! requête.

use std::fs::File;
use std::io::Write as _;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::{PriceSource, SearchHit, SourceError};
use crate::model::{CoinId, Currency, Quote, Range, Series};

/// Premier délai de backoff après un échec réseau.
const BACKOFF_MIN: u64 = 60;
/// Plafond du backoff : au-delà, inutile d'espacer davantage.
const BACKOFF_MAX: u64 = 1800;

/// Valeur issue du cache, avec l'instant du dernier succès réseau.
#[derive(Debug, Clone)]
pub struct Cached<T> {
    pub value: T,
    pub fetched_at: u64,
    /// `true` si la valeur servie n'a pas pu être rafraîchie.
    pub stale: bool,
}

#[derive(Serialize, Deserialize)]
struct Entry<T> {
    fetched_at: u64,
    value: T,
}

#[derive(Serialize, Deserialize, Default)]
struct Backoff {
    /// Horodatage Unix avant lequel aucune requête ne doit être tentée.
    retry_after: u64,
    /// Délai courant, doublé à chaque échec consécutif.
    delay: u64,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Nom de fichier sûr dérivé d'une clé logique. La clé ne contient que des
/// caractères déjà validés, mais on filtre à nouveau pour qu'aucun chemin ne
/// puisse s'échapper du répertoire de cache.
fn sanitize(key: &str) -> String {
    key.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .take(120)
        .collect()
}

/// Enveloppe une source avec un cache disque.
pub struct CachedSource<S: PriceSource> {
    source: S,
    dir: PathBuf,
    ttl: Duration,
}

impl<S: PriceSource> CachedSource<S> {
    pub fn new(source: S, dir: PathBuf, ttl: Duration) -> Self {
        Self { source, dir, ttl }
    }

    #[must_use]
    pub fn source(&self) -> &S {
        &self.source
    }

    fn path(&self, key: &str) -> PathBuf {
        self.dir.join(format!("{}.json", sanitize(key)))
    }

    fn read<T: for<'de> Deserialize<'de>>(&self, key: &str) -> Option<Entry<T>> {
        let raw = std::fs::read_to_string(self.path(key)).ok()?;
        // Un cache corrompu est traité comme absent : on ne veut jamais échouer là-dessus.
        serde_json::from_str(&raw).ok()
    }

    fn write<T: Serialize>(&self, key: &str, entry: &Entry<T>) -> std::io::Result<()> {
        crate::paths::ensure_dir(&self.dir)?;
        let final_path = self.path(key);
        let tmp_path = final_path.with_extension("json.tmp");
        {
            let mut file = File::create(&tmp_path)?;
            let bytes = serde_json::to_vec(entry)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
            file.write_all(&bytes)?;
            file.sync_all()?;
        }
        std::fs::rename(&tmp_path, &final_path)
    }

    /// Prend le verrou d'écriture. `None` si un autre processus l'a déjà :
    /// dans ce cas l'appelant sert le cache existant au lieu d'attendre.
    fn try_lock(&self) -> Option<File> {
        crate::paths::ensure_dir(&self.dir).ok()?;
        let file = File::create(self.dir.join(".lock")).ok()?;
        match rustix::fs::flock(&file, rustix::fs::FlockOperation::NonBlockingLockExclusive) {
            Ok(()) => Some(file),
            Err(_) => None,
        }
    }

    fn backoff_path(&self) -> PathBuf {
        self.dir.join("backoff.state")
    }

    fn backoff(&self) -> Backoff {
        std::fs::read_to_string(self.backoff_path())
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    fn record_failure(&self, retry_after: Option<u64>) {
        let previous = self.backoff();
        let delay = retry_after.unwrap_or_else(|| {
            if previous.delay == 0 {
                BACKOFF_MIN
            } else {
                previous.delay.saturating_mul(2)
            }
        });
        let delay = delay.clamp(BACKOFF_MIN, BACKOFF_MAX);
        let next = Backoff {
            retry_after: now().saturating_add(delay),
            delay,
        };
        if let Ok(bytes) = serde_json::to_vec(&next) {
            let _ = crate::paths::ensure_dir(&self.dir);
            let _ = std::fs::write(self.backoff_path(), bytes);
        }
    }

    fn clear_failure(&self) {
        let _ = std::fs::remove_file(self.backoff_path());
    }

    /// Cœur de la logique de cache, partagé par toutes les méthodes.
    fn get_or_refresh<T, F>(&self, key: &str, fetch: F) -> Result<Cached<T>, SourceError>
    where
        T: Serialize + for<'de> Deserialize<'de> + Clone,
        F: FnOnce() -> Result<T, SourceError>,
    {
        let existing = self.read::<T>(key);
        let now_secs = now();

        if let Some(entry) = &existing {
            let age = now_secs.saturating_sub(entry.fetched_at);
            if age < self.ttl.as_secs() {
                return Ok(Cached {
                    value: entry.value.clone(),
                    fetched_at: entry.fetched_at,
                    stale: false,
                });
            }
        }

        let blocked = now_secs < self.backoff().retry_after;
        let lock = if blocked { None } else { self.try_lock() };

        if lock.is_none() {
            // Backoff actif ou verrou pris ailleurs : on sert ce qu'on a.
            return match existing {
                Some(entry) => Ok(Cached {
                    value: entry.value,
                    fetched_at: entry.fetched_at,
                    stale: true,
                }),
                None if blocked => Err(SourceError::Http(
                    "backoff actif après un échec réseau, aucune donnée en cache".into(),
                )),
                None => fetch().map(|value| {
                    let _ = self.write(
                        key,
                        &Entry {
                            fetched_at: now_secs,
                            value: value.clone(),
                        },
                    );
                    Cached {
                        value,
                        fetched_at: now_secs,
                        stale: false,
                    }
                }),
            };
        }

        match fetch() {
            Ok(value) => {
                self.clear_failure();
                let _ = self.write(
                    key,
                    &Entry {
                        fetched_at: now_secs,
                        value: value.clone(),
                    },
                );
                Ok(Cached {
                    value,
                    fetched_at: now_secs,
                    stale: false,
                })
            }
            Err(err) => {
                let retry_after = match &err {
                    SourceError::RateLimited { retry_after } => *retry_after,
                    _ => None,
                };
                self.record_failure(retry_after);
                match existing {
                    Some(entry) => Ok(Cached {
                        value: entry.value,
                        fetched_at: entry.fetched_at,
                        stale: true,
                    }),
                    None => Err(err),
                }
            }
        }
    }

    /// Cotations, servies depuis le cache tant qu'il est frais.
    ///
    /// # Errors
    /// Échoue uniquement si le réseau échoue **et** qu'aucun cache n'existe.
    pub fn markets_cached(
        &self,
        ids: &[CoinId],
        vs: &Currency,
    ) -> Result<Cached<Vec<Quote>>, SourceError> {
        let key = format!(
            "markets-{}-{}",
            vs.as_str(),
            ids.iter().map(CoinId::as_str).collect::<Vec<_>>().join("_")
        );
        self.get_or_refresh(&key, || self.source.markets(ids, vs))
    }

    /// Série de prix, cachée par couple (cryptomonnaie, plage).
    ///
    /// # Errors
    /// Échoue uniquement si le réseau échoue **et** qu'aucun cache n'existe.
    pub fn series_cached(
        &self,
        id: &CoinId,
        range: Range,
        vs: &Currency,
    ) -> Result<Cached<Series>, SourceError> {
        let key = format!("series-{}-{}-{}", vs.as_str(), id.as_str(), range.slug());
        self.get_or_refresh(&key, || self.source.series(id, range, vs))
    }

    /// Recherche : jamais cachée, elle est ponctuelle et interactive.
    ///
    /// # Errors
    /// Voir [`SourceError`].
    pub fn search(&self, term: &str) -> Result<Vec<SearchHit>, SourceError> {
        self.source.search(term)
    }

    /// Supprime tout le contenu du cache et remet le backoff à zéro.
    ///
    /// # Errors
    /// Remonte les erreurs d'E/S autres que « fichier absent ».
    pub fn invalidate(&self) -> std::io::Result<()> {
        self.clear_failure();
        let entries = match std::fs::read_dir(&self.dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e),
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "json") {
                std::fs::remove_file(path)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::cell::Cell;

    /// Source de test : compte les appels et peut être forcée en échec.
    struct FakeSource {
        calls: Cell<usize>,
        fail: bool,
    }

    impl FakeSource {
        fn new(fail: bool) -> Self {
            Self {
                calls: Cell::new(0),
                fail,
            }
        }
    }

    impl PriceSource for FakeSource {
        fn markets(&self, ids: &[CoinId], _vs: &Currency) -> Result<Vec<Quote>, SourceError> {
            self.calls.set(self.calls.get().saturating_add(1));
            if self.fail {
                return Err(SourceError::Http("panne simulée".into()));
            }
            Ok(ids
                .iter()
                .map(|id| Quote {
                    id: id.clone(),
                    symbol: "X".into(),
                    name: "X".into(),
                    price: 1.0,
                    change_1h: None,
                    change_24h: Some(1.0),
                    change_7d: None,
                    sparkline: vec![1.0, 2.0],
                    image: None,
                    market_cap: None,
                    volume_24h: None,
                    rank: None,
                    ath: None,
                    ath_date: None,
                    icon: None,
                })
                .collect())
        }

        fn series(&self, id: &CoinId, range: Range, _vs: &Currency) -> Result<Series, SourceError> {
            self.calls.set(self.calls.get().saturating_add(1));
            if self.fail {
                return Err(SourceError::Http("panne simulée".into()));
            }
            Ok(Series {
                id: id.clone(),
                range,
                points: vec![1.0, 2.0, 3.0],
            })
        }

        fn search(&self, _term: &str) -> Result<Vec<SearchHit>, SourceError> {
            Ok(Vec::new())
        }
    }

    fn setup(ttl: Duration, fail: bool) -> (tempfile::TempDir, CachedSource<FakeSource>) {
        let dir = tempfile::tempdir().unwrap();
        let cached = CachedSource::new(FakeSource::new(fail), dir.path().to_path_buf(), ttl);
        (dir, cached)
    }

    fn ids() -> Vec<CoinId> {
        vec![CoinId::parse("bitcoin").unwrap()]
    }

    #[test]
    fn le_second_appel_dans_le_ttl_ne_refait_pas_de_requete() {
        let (_dir, cached) = setup(Duration::from_secs(3600), false);
        let vs = Currency::default();
        let first = cached.markets_cached(&ids(), &vs).unwrap();
        let second = cached.markets_cached(&ids(), &vs).unwrap();
        assert_eq!(cached.source().calls.get(), 1);
        assert!(!first.stale);
        assert!(!second.stale);
    }

    #[test]
    fn un_ttl_expire_declenche_une_nouvelle_requete() {
        let (_dir, cached) = setup(Duration::from_secs(0), false);
        let vs = Currency::default();
        cached.markets_cached(&ids(), &vs).unwrap();
        cached.markets_cached(&ids(), &vs).unwrap();
        assert_eq!(cached.source().calls.get(), 2);
    }

    #[test]
    fn une_panne_reseau_sert_le_cache_marque_perime() {
        let dir = tempfile::tempdir().unwrap();
        let vs = Currency::default();
        {
            let ok = CachedSource::new(
                FakeSource::new(false),
                dir.path().to_path_buf(),
                Duration::from_secs(0),
            );
            ok.markets_cached(&ids(), &vs).unwrap();
        }
        let broken = CachedSource::new(
            FakeSource::new(true),
            dir.path().to_path_buf(),
            Duration::from_secs(0),
        );
        let result = broken.markets_cached(&ids(), &vs).unwrap();
        assert!(result.stale);
        assert_eq!(result.value.len(), 1);
    }

    #[test]
    fn une_panne_sans_cache_remonte_lerreur() {
        let (_dir, cached) = setup(Duration::from_secs(60), true);
        assert!(cached.markets_cached(&ids(), &Currency::default()).is_err());
    }

    #[test]
    fn le_backoff_empeche_de_marteler_lapi_apres_un_echec() {
        let dir = tempfile::tempdir().unwrap();
        let vs = Currency::default();
        {
            let ok = CachedSource::new(
                FakeSource::new(false),
                dir.path().to_path_buf(),
                Duration::from_secs(0),
            );
            ok.markets_cached(&ids(), &vs).unwrap();
        }
        let broken = CachedSource::new(
            FakeSource::new(true),
            dir.path().to_path_buf(),
            Duration::from_secs(0),
        );
        broken.markets_cached(&ids(), &vs).unwrap();
        broken.markets_cached(&ids(), &vs).unwrap();
        // Le second appel est bloqué par le backoff : une seule requête réseau.
        assert_eq!(broken.source().calls.get(), 1);
    }

    #[test]
    fn changer_de_devise_ou_de_liste_change_la_cle_de_cache() {
        let (_dir, cached) = setup(Duration::from_secs(3600), false);
        cached
            .markets_cached(&ids(), &Currency::parse("eur").unwrap())
            .unwrap();
        cached
            .markets_cached(&ids(), &Currency::parse("usd").unwrap())
            .unwrap();
        assert_eq!(cached.source().calls.get(), 2);
    }

    #[test]
    fn invalidate_force_une_nouvelle_requete() {
        let (_dir, cached) = setup(Duration::from_secs(3600), false);
        let vs = Currency::default();
        cached.markets_cached(&ids(), &vs).unwrap();
        cached.invalidate().unwrap();
        cached.markets_cached(&ids(), &vs).unwrap();
        assert_eq!(cached.source().calls.get(), 2);
    }

    #[test]
    fn un_cache_corrompu_est_traite_comme_absent() {
        let (dir, cached) = setup(Duration::from_secs(3600), false);
        let vs = Currency::default();
        cached.markets_cached(&ids(), &vs).unwrap();
        for entry in std::fs::read_dir(dir.path()).unwrap().flatten() {
            if entry.path().extension().is_some_and(|e| e == "json") {
                std::fs::write(entry.path(), b"{ pas du json").unwrap();
            }
        }
        cached.markets_cached(&ids(), &vs).unwrap();
        assert_eq!(cached.source().calls.get(), 2);
    }

    #[test]
    fn les_series_sont_cachees_par_plage() {
        let (_dir, cached) = setup(Duration::from_secs(3600), false);
        let vs = Currency::default();
        let id = CoinId::parse("bitcoin").unwrap();
        cached.series_cached(&id, Range::D7, &vs).unwrap();
        cached.series_cached(&id, Range::D7, &vs).unwrap();
        assert_eq!(cached.source().calls.get(), 1);
        cached.series_cached(&id, Range::H24, &vs).unwrap();
        assert_eq!(cached.source().calls.get(), 2);
    }
}
