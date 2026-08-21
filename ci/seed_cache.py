"""Pré-remplit le cache pour la mesure de performance en CI.

Le budget de démarrage doit être mesuré sur cache chaud : c'est le cas
courant, celui qui s'exécute à chaque tick de Waybar.
"""

import json
import os
import sys
import time

CACHE_DIR = sys.argv[1]

ENTRY = {
    "fetched_at": int(time.time()),
    "value": [
        {
            "id": "bitcoin",
            "symbol": "btc",
            "name": "Bitcoin",
            "price": 42000.0,
            "change_1h": 0.1,
            "change_24h": 1.0,
            "change_7d": 2.0,
            "sparkline": [1.0, 2.0, 3.0, 2.0],
        }
    ],
}

with open(os.path.join(CACHE_DIR, "markets-eur-bitcoin.json"), "w") as handle:
    json.dump(ENTRY, handle)
