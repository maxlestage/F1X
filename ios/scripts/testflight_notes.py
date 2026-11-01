#!/usr/bin/env python3
"""Remplit « À tester » d'un build TestFlight avec la liste des commits livrés.

Le build vient d'être envoyé : on attend qu'App Store Connect le connaisse, puis on écrit
les notes (titre de la fusion, commit, et un point par commit depuis le build précédent)
dans chacune de ses localisations bêta.

Variables : KEY_ID, ISSUER_ID, KEY_PATH (clé .p8), BUNDLE_ID, BUILD_NUMBER, NOTES_FILE.
"""
import json
import os
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

import jwt  # PyJWT (+ cryptography pour ES256)

API = "https://api.appstoreconnect.apple.com/v1"
LIMIT = 4000  # longueur maximale acceptée par Apple pour « whatsNew »


def token():
    now = int(time.time())
    key = open(os.environ["KEY_PATH"]).read()
    return jwt.encode(
        {"iss": os.environ["ISSUER_ID"], "iat": now, "exp": now + 600, "aud": "appstoreconnect-v1"},
        key,
        algorithm="ES256",
        headers={"kid": os.environ["KEY_ID"], "typ": "JWT"},
    )


def call(method, path, body=None):
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(f"{API}{path}", data=data, method=method, headers={
        "Authorization": f"Bearer {token()}", "Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=60) as r:
        raw = r.read()
        return json.loads(raw) if raw else None


def find_build(app_id, number, wait=1500):
    """Attend (jusqu'à 25 min) que le build envoyé apparaisse sur App Store Connect."""
    query = urllib.parse.urlencode({"filter[app]": app_id, "filter[version]": number, "limit": 1})
    deadline = time.time() + wait
    while time.time() < deadline:
        data = call("GET", f"/builds?{query}")["data"]
        if data:
            return data[0]["id"]
        time.sleep(30)
    return None


def main():
    notes = open(os.environ["NOTES_FILE"], encoding="utf-8").read().strip()
    if len(notes) > LIMIT:
        notes = notes[: LIMIT - 1].rsplit("\n", 1)[0] + "\n…"
    print(notes)

    bundle = urllib.parse.quote(os.environ["BUNDLE_ID"])
    apps = call("GET", f"/apps?filter[bundleId]={bundle}&limit=1")["data"]
    if not apps:
        print(f"::warning::App {os.environ['BUNDLE_ID']} introuvable : notes TestFlight non écrites.")
        return
    build = find_build(apps[0]["id"], os.environ["BUILD_NUMBER"])
    if not build:
        print("::warning::Build pas encore visible sur App Store Connect : notes TestFlight non écrites.")
        return

    for attempt in range(10):
        try:
            locs = call("GET", f"/builds/{build}/betaBuildLocalizations")["data"]
            if locs:
                for loc in locs:
                    call("PATCH", f"/betaBuildLocalizations/{loc['id']}", {"data": {
                        "type": "betaBuildLocalizations", "id": loc["id"],
                        "attributes": {"whatsNew": notes}}})
            else:
                call("POST", "/betaBuildLocalizations", {"data": {
                    "type": "betaBuildLocalizations",
                    "attributes": {"locale": "fr-FR", "whatsNew": notes},
                    "relationships": {"build": {"data": {"type": "builds", "id": build}}}}})
            print(f"::notice::Notes « À tester » écrites sur le build {os.environ['BUILD_NUMBER']}.")
            return
        except urllib.error.HTTPError as e:
            # Build encore en traitement : Apple refuse parfois l'écriture, on réessaie.
            print(f"essai {attempt + 1} : HTTP {e.code} {e.read()[:300]!r}")
            time.sleep(30)
    print("::warning::Notes TestFlight non écrites après plusieurs essais.")


if __name__ == "__main__":
    try:
        main()
    except Exception as e:  # les notes ne doivent jamais faire échouer la livraison
        print(f"::warning::Notes TestFlight non écrites : {e}")
        sys.exit(0)
