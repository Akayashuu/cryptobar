# cryptobar

Cours de cryptomonnaies pour Waybar, le terminal et le bureau. Sans clé d'API,
sans démon, sans dépendance au réseau pour s'afficher.

```
BTC 66 040 € ▁▁▁▂▂▃▅█ +6,0 %
```

## Ce que ça fait

- **Module Waybar** — une cryptomonnaie à la fois, sparkline Unicode intégrée,
  molette pour passer à la suivante, infobulle listant toutes les autres avec
  leurs variations 1 h / 24 h / 7 j.
- **Graphique terminal** — courbe braille pleine résolution, dimensionnée sur la
  taille réelle du terminal.
- **Fenêtre GTK4** — prix, variations et courbe vectorielle, ouverte au clic.
- **Recherche d'identifiants** — `cryptobar coins solana` trouve ce qu'il faut
  écrire dans la configuration.

## Installation

### Depuis les sources

```bash
git clone https://github.com/Akayashuu/cryptobar
cd cryptobar
./install.sh
```

Le script compile, installe dans `~/.local/bin`, écrit la configuration par
défaut et dépose les exemples d'intégration dans `~/.config`. Il n'écrase
jamais votre configuration Waybar ou Hyprland : les deux ajouts finaux sont à
faire à la main, le script vous les rappelle.

### Sans la fenêtre GTK

```bash
cargo install --path . --no-default-features
```

Le binaire n'a alors plus aucune dépendance GTK.

## Configuration

`~/.config/cryptobar/config.toml`, créé au premier lancement :

```toml
# Identifiants CoinGecko ; cherchez-les avec `cryptobar coins <terme>`.
coins = ["bitcoin", "ethereum", "solana"]

# Devise de cotation : eur, usd, gbp, chf, cad, jpy, btc, eth
currency = "eur"

# Intervalle de rafraîchissement des prix, en secondes (minimum 15).
refresh_interval = 60

# Rotation automatique entre les coins, en secondes. 0 = rotation manuelle à la molette.
cycle_interval = 0

# Sparkline inline dans la barre.
sparkline = true
sparkline_width = 8

# Décimales : "auto" s'adapte à l'ordre de grandeur, ou un entier pour les figer.
decimals = "auto"

# Émulateur de terminal utilisé par le clic droit.
terminal = "kitty"

# Signal temps réel utilisé pour rafraîchir Waybar : SIGRTMIN+<n>, 1..=30.
waybar_signal = 8
```

`cryptobar config check` valide le fichier et explique ce qui cloche.

## Intégration Waybar

Dans `~/.config/waybar/config.jsonc` :

```jsonc
"custom/crypto": {
  "exec": "cryptobar bar",
  "return-type": "json",
  "interval": 30,
  "signal": 8,
  "on-click": "cryptobar popup",
  "on-click-right": "cryptobar chart --window",
  "on-click-middle": "cryptobar refresh",
  "on-scroll-up": "cryptobar next",
  "on-scroll-down": "cryptobar prev"
}
```

Puis ajoutez `"custom/crypto"` à `modules-right`, et importez le style :

```css
@import "cryptobar.css";
```

`"signal"` doit valoir la même chose que `waybar_signal` dans la configuration :
c'est ce qui fait que la molette met la barre à jour instantanément au lieu
d'attendre le prochain `interval`.

L'`interval` de Waybar et le `refresh_interval` de cryptobar sont indépendants.
Waybar peut ticker toutes les 5 secondes sans générer la moindre requête : le
cache disque sert les données tant qu'elles sont fraîches.

## Commandes

| Commande | Effet |
|---|---|
| `cryptobar bar` | Émet la ligne JSON attendue par Waybar |
| `cryptobar bar --watch` | Reste en avant-plan et réémet à chaque intervalle |
| `cryptobar next` / `prev` | Change la cryptomonnaie affichée |
| `cryptobar refresh` | Invalide le cache et réveille Waybar |
| `cryptobar chart [COIN] --range 7d` | Graphique dans le terminal courant |
| `cryptobar chart --window` | Même graphique, nouvelle fenêtre de terminal |
| `cryptobar popup` | Fenêtre GTK4 |
| `cryptobar coins <terme>` | Cherche un identifiant CoinGecko |
| `cryptobar config init\|path\|check` | Gère le fichier de configuration |

Plages acceptées par `--range` : `1h`, `24h`, `7d`, `30d`, `1y`.

## Comment ça tient en ressources

Le module s'exécute à chaque tick de Waybar, il ne peut donc pas se permettre
d'être lourd.

- **3 à 5 ms par tick**, mesuré en release sur cache chaud.
- **Pas de démon** — rien ne tourne entre deux ticks. Le mode `--watch` existe
  mais n'est pas nécessaire.
- **Pas de runtime asynchrone** — un client HTTP synchrone (`ureq` + `rustls`)
  suffit pour une requête toutes les 60 secondes et évite d'embarquer `tokio`.
- **Une requête pour toutes les cryptomonnaies**, sparklines 7 jours comprises.
- **Verrou consultatif** sur le cache : deux ticks simultanés ne partent jamais
  en double requête.
- **Écriture atomique** du cache et de l'état : une coupure n'a jamais laissé de
  fichier à moitié écrit.
- **Repli progressif** — en cas de panne réseau, les dernières données connues
  sont servies avec la classe CSS `stale` plutôt que de laisser un trou dans la
  barre. Un backoff exponentiel (60 s → 30 min) évite de marteler l'API.

Côté sûreté : `unsafe` est interdit à la compilation (`unsafe_code = "forbid"`),
`unwrap`, `expect`, `panic` et l'indexation directe sont refusés par les lints,
les identifiants sont validés à la construction avant de pouvoir atteindre une
URL ou un chemin, les redirections HTTP sont refusées, la taille des réponses
est plafonnée à 1 Mio, et toute donnée venant du réseau est échappée avant
d'atteindre le balisage Pango de Waybar.

## Source des données

[CoinGecko](https://www.coingecko.com), API publique, sans clé. Elle limite le
débit aux alentours de 10 à 30 requêtes par minute selon la charge ; le
`refresh_interval` minimum de 15 secondes et le cache gardent l'usage très
en dessous de cette limite.

## Développement

```bash
cargo test --all-features
cargo clippy --all-targets --all-features -- -D warnings
```

Les tests ne touchent jamais le réseau : `CRYPTOBAR_API_BASE` redirige le client
et les tests bout en bout pré-remplissent le cache. `CRYPTOBAR_CONFIG_DIR`,
`CRYPTOBAR_CACHE_DIR` et `CRYPTOBAR_RUNTIME_DIR` isolent les chemins XDG.

## Licence

MIT.
