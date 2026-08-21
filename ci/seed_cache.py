"""Seeds the cache for the CI performance measurement.

The startup budget has to be measured on a warm cache: that is the common
case, the one running at every Waybar tick.
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
