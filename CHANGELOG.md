# Journal des modifications

Le format suit [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/) et le
versionnage sémantique.

## [Non publié]

### Ajouté

- Intégration Quickshell pour dots-hyprland (`quickshell/`) : service singleton
  partagé par tous les écrans, widget horizontal, widget vertical, infobulle,
  installateur idempotent.
- `cryptobar bar --markup none` : sortie sans balisage Pango pour les barres
  qui affichent du texte brut.
- Champs `symbol`, `price`, `percent` et `sparkline` dans la ligne JSON, pour
  les mises en page trop étroites pour le champ `text`.

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
