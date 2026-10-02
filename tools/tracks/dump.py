#!/usr/bin/env python3
"""Enregistre le tracé de chaque circuit couru depuis 2023 dans web/server/tracks/{id}.json.

Les fichiers sont embarqués dans le serveur à la compilation : les circuits s'affichent même
quand OpenF1 refuse l'accès (pendant une séance en direct, l'API gratuite est fermée).

Usage : python3 tools/tracks/dump.py [URL du serveur F1X, défaut http://localhost:8080]
(à lancer hors séance en direct, serveur F1X démarré).
"""
import json
import os
import sys
import time
import urllib.request

BASE = (sys.argv[1] if len(sys.argv) > 1 else "http://localhost:8080").rstrip("/")
OUT = os.path.join(os.path.dirname(__file__), "..", "..", "web", "server", "tracks")


def get(url, tries=4):
    for k in range(tries):
        try:
            with urllib.request.urlopen(url, timeout=180) as r:
                return r.status, r.read()
        except urllib.error.HTTPError as e:
            if e.code in (404, 401):
                return e.code, e.read()
            time.sleep(5 * (k + 1))
        except Exception:
            time.sleep(5 * (k + 1))
    return 0, b""


circuits = set()
for year in range(2023, time.gmtime().tm_year + 1):
    status, body = get(f"{BASE}/api/f1/{year}/circuits.json?limit=100")
    if status == 200:
        for c in json.loads(body)["MRData"]["CircuitTable"]["Circuits"]:
            circuits.add(c["circuitId"])
os.makedirs(OUT, exist_ok=True)
ok = 0
for cid in sorted(circuits):
    status, body = get(f"{BASE}/api/track/{cid}")
    if status == 200:
        track = json.loads(body)
        # Arrondi : fichiers plus légers, précision largement suffisante.
        for p in track["points"]:
            for k in ("x", "y", "z", "t"):
                p[k] = round(p[k], 2)
        with open(os.path.join(OUT, f"{cid}.json"), "w") as f:
            json.dump(track, f, separators=(",", ":"))
        ok += 1
        print(f"{cid}: {len(track['points'])} points")
    else:
        print(f"{cid}: HTTP {status} {body[:80]!r}")
print(f"{ok}/{len(circuits)} circuits enregistrés dans {os.path.normpath(OUT)}")
