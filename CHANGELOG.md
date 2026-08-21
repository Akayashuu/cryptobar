# Journal des modifications

Le format suit [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/) et le
versionnage sémantique.

## [0.2.0] — 2026-08-22

### Ajouté

- Portefeuille en lecture seule suivi par **adresse publique** : soldes lus sur
  mempool.space (Bitcoin), `ethereum-rpc.publicnode.com` (Ethereum) et
  `api.mainnet-beta.solana.com` (Solana), sans clé d'API. Une clé privée ou une
  phrase de récupération est refusée avant toute écriture sur le disque.
- Commande `cryptobar portfolio` : valorisation, parts et gain sur 24 h dans le
  terminal.
- Alertes de prix (`[[alerts]]`) sur seuil haut, seuil bas et amplitude de
  variation sur 24 h, avec réarmement au retour sous le seuil et notification
  de bureau depuis `cryptobar bar --watch`.
- Commande `cryptobar select <N>` pour afficher une cryptomonnaie donnée.
- Fenêtre GTK4 complète : recherche de cryptomonnaies avec leur logo, courbe
  sur cinq plages, export PNG et CSV, liste des cryptomonnaies suivies,
  portefeuille, alertes et réglages groupés — toute la configuration se modifie
  depuis la fenêtre, en préservant commentaires et ordre du fichier TOML.
- Logos officiels des cryptomonnaies, mis en cache sur disque, dans la fenêtre
  comme dans les widgets Quickshell.
- Intégration Quickshell pour dots-hyprland (`quickshell/`) : service singleton
  partagé par tous les écrans, widget horizontal, widget vertical, infobulle,
  installateur idempotent.
- `cryptobar bar --markup none` : sortie sans balisage Pango pour les barres
  qui affichent du texte brut.
- Ligne JSON étendue : `symbol`, `price`, `percent`, `sparkline`, `trend`,
  `stale`, `icon`, `glyph`, `color`, `foreground`, `coins` (toutes les
  cryptomonnaies suivies avec leur série normalisée dans `[0, 1]`), `alerts` et
  `portfolio`.

### Modifié

- Les widgets Quickshell dessinent la courbe au canevas et affichent un panneau
  natif au lieu d'un bloc de texte monospace. La barre verticale est réduite au
  logo, au prix et à la variation.

## [0.1.0] — 2026-08-21

Première version.

### Ajouté

- Module Waybar : sparkline Unicode, cyclage à la molette, infobulle détaillée
  avec les variations 1 h / 24 h / 7 j, classes CSS suivant la tendance.
- Graphique braille dans le terminal, dimensionné sur la taille réelle de
  celui-ci, avec axe des ordonnées étiqueté.
- Fenêtre GTK4 avec courbe vectorielle, derrière une fonctionnalité optionnelle
  activée par défaut.
- Recherche d'identifiants CoinGecko (`cryptobar coins <terme>`).
- Gestion de la configuration (`init`, `path`, `check`) avec validation stricte
  et messages explicites.
- Cache disque à TTL, écriture atomique, verrou consultatif et backoff
  exponentiel ; dégradation en données périmées plutôt qu'en module vide.
- Rafraîchissement instantané de Waybar par `SIGRTMIN+n`.
