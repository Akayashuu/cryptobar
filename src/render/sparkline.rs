//! Sparkline en blocs Unicode. Fonction pure, sans E/S.

/// Les huit niveaux de remplissage, du plus bas au plus haut.
const LEVELS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
/// Niveau utilisé quand tous les points sont égaux : le milieu de l'échelle.
const FLAT_LEVEL: char = '▄';

/// Réduit la série à au plus `width` points en moyennant des tranches contiguës.
/// Moyenner plutôt qu'échantillonner évite qu'un pic isolé disparaisse ou domine.
fn downsample(points: &[f64], width: usize) -> Vec<f64> {
    if points.len() <= width {
        return points.to_vec();
    }
    let mut out = Vec::with_capacity(width);
    for bucket in 0..width {
        // `width` est non nul : `downsample` n'est appelée que depuis `sparkline`,
        // qui retourne tôt sur une largeur nulle.
        #[allow(clippy::integer_division, clippy::arithmetic_side_effects)]
        let start = bucket.saturating_mul(points.len()) / width;
        #[allow(clippy::integer_division, clippy::arithmetic_side_effects)]
        let end = bucket.saturating_add(1).saturating_mul(points.len()) / width;
        let end = end.max(start.saturating_add(1));
        if let Some(slice) = points.get(start..end.min(points.len()))
            && !slice.is_empty()
        {
            let sum: f64 = slice.iter().sum();
            #[allow(clippy::cast_precision_loss)]
            out.push(sum / slice.len() as f64);
        }
    }
    out
}

/// Rend une série numérique sous forme de sparkline d'au plus `width` caractères.
///
/// Les valeurs non finies sont écartées. Une série vide, une largeur nulle ou une
/// série entièrement non finie donnent une chaîne vide. Une série constante rend un
/// niveau médian plutôt que de diviser par une amplitude nulle.
#[must_use]
pub fn sparkline(points: &[f64], width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let finite: Vec<f64> = points.iter().copied().filter(|v| v.is_finite()).collect();
    if finite.is_empty() {
        return String::new();
    }

    let sampled = downsample(&finite, width);
    let min = sampled.iter().copied().fold(f64::INFINITY, f64::min);
    let max = sampled.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let amplitude = max - min;

    if !amplitude.is_finite() || amplitude <= f64::EPSILON {
        return std::iter::repeat_n(FLAT_LEVEL, sampled.len()).collect();
    }

    let last_level = LEVELS.len().saturating_sub(1);
    sampled
        .iter()
        .map(|value| {
            #[allow(
                clippy::cast_precision_loss,
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss
            )]
            let index = (((value - min) / amplitude) * last_level as f64).round() as usize;
            LEVELS
                .get(index.min(last_level))
                .copied()
                .unwrap_or(FLAT_LEVEL)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serie_croissante_monte_par_paliers() {
        assert_eq!(sparkline(&[1.0, 2.0, 3.0, 4.0], 4), "▁▃▆█");
    }

    #[test]
    fn serie_plate_donne_un_niveau_median_sans_division_par_zero() {
        assert_eq!(sparkline(&[5.0, 5.0, 5.0], 3), "▄▄▄");
    }

    #[test]
    fn serie_vide_ou_largeur_nulle_donne_une_chaine_vide() {
        assert_eq!(sparkline(&[], 8), "");
        assert_eq!(sparkline(&[1.0, 2.0], 0), "");
    }

    #[test]
    fn valeur_unique_donne_un_seul_caractere() {
        assert_eq!(sparkline(&[42.0], 8), "▄");
    }

    #[test]
    fn les_valeurs_non_finies_sont_ignorees() {
        let out = sparkline(&[1.0, f64::NAN, 3.0, f64::INFINITY], 3);
        assert_eq!(out.chars().count(), 2);
        assert!(!out.contains('\u{fffd}'));
    }

    #[test]
    fn la_serie_est_sous_echantillonnee_a_la_largeur_demandee() {
        let points: Vec<f64> = (0..1000).map(f64::from).collect();
        assert_eq!(sparkline(&points, 8).chars().count(), 8);
    }

    #[test]
    fn une_serie_plus_courte_que_la_largeur_nest_pas_etiree() {
        assert_eq!(sparkline(&[1.0, 2.0, 3.0], 8).chars().count(), 3);
    }

    proptest::proptest! {
        #[test]
        fn ne_panique_jamais_et_respecte_la_largeur(
            points in proptest::collection::vec(proptest::num::f64::ANY, 0..200),
            width in 0usize..64,
        ) {
            let out = sparkline(&points, width);
            proptest::prop_assert!(out.chars().count() <= width);
        }
    }
}
