# Intégration Quickshell

Widget de barre pour [Quickshell](https://quickshell.outfoxxed.me/), conçu
pour la configuration `ii` de [dots-hyprland](https://github.com/end-4/dots-hyprland).
Les barres horizontale et verticale sont toutes deux prises en charge.

## Installation

```bash
./quickshell/install.sh          # dans ~/.config/quickshell/ii
QS_DIR=/autre/chemin ./quickshell/install.sh
qs -c ii kill && qs -c ii -d     # redémarrer pour charger le widget
```

Le script est idempotent : il repère sa propre marque `// cryptobar` et ne
recâble pas deux fois. Il sauvegarde chaque fichier amont modifié en
`<fichier>.bak-cryptobar` avant sa première intervention.

> Une mise à jour de dots-hyprland écrase `Config.qml`, `BarContent.qml` et
> `VerticalBarContent.qml`. Relancez le script après coup ; les widgets, eux,
> vivent dans des fichiers qui n'appartiennent pas à l'amont.

## Fichiers

| Fichier | Rôle |
| --- | --- |
| `services/Crypto.qml` | Singleton : un seul `cryptobar bar --watch` pour toute la session, quel que soit le nombre d'écrans. Expose `text`, `symbol`, `price`, `percent`, `sparkline`, `tooltip`, `trend`, `stale`. |
| `modules/ii/bar/CryptoWidget.qml` | Barre horizontale : ligne complète avec sparkline. |
| `modules/ii/bar/CryptoWidgetPopup.qml` | Infobulle commune aux deux orientations. |
| `modules/ii/verticalBar/VerticalCryptoWidget.qml` | Barre verticale : symbole, prix et variation empilés. |

## Interactions

| Geste | Effet |
| --- | --- |
| Molette haut / bas | Pièce suivante / précédente |
| Clic gauche | Fenêtre détaillée (GTK) |
| Clic droit | Graphique dans un terminal |
| Clic milieu | Rafraîchissement forcé |
| Survol | Toutes les cotations, avec 1 h / 24 h / 7 j |

Après une action, le widget relit immédiatement la valeur courante au lieu
d'attendre le prochain intervalle.

## Options

Dans `~/.config/illogical-impulse/config.json` :

```json
{ "bar": { "crypto": { "enable": true, "binary": "" } } }
```

`binary` vide pointe sur `~/.local/bin/cryptobar`, l'emplacement d'installation
par défaut. Un chemin absolu est nécessaire : Quickshell n'hérite pas du `PATH`
d'un shell interactif.

Les pièces suivies, la devise, l'intervalle et la largeur de la sparkline se
règlent dans `~/.config/cryptobar/config.toml` (`cryptobar config init`).

## Autres barres

Le widget consomme la sortie de `cryptobar bar --markup none` : une ligne de
JSON par intervalle, sans balisage Pango. Ags et Eww peuvent s'en servir tels
quels. Waybar utilise le mode par défaut, `--markup pango`.
