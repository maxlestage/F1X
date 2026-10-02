#!/usr/bin/env python3
"""Libère des certificats de développement Apple avant une compilation CI.

Chaque compilation sur un Mac GitHub neuf crée un certificat « Apple Development » (la clé
privée n'est pas conservée) ; le compte atteint vite la limite et Xcode refuse alors de
signer (« Your account has reached the maximum number of certificates »).
On révoque donc les certificats de développement créés par l'API (compilations précédentes),
puis, si la limite est encore atteinte, les plus anciens. Les certificats de distribution ne
sont jamais touchés.

Variables : KEY_ID, ISSUER_ID, KEY_PATH (clé .p8 App Store Connect).
"""
import json
import os
import time
import urllib.request

import jwt  # PyJWT (+ cryptography pour ES256)

API = "https://api.appstoreconnect.apple.com/v1"
DEV_TYPES = {"DEVELOPMENT", "IOS_DEVELOPMENT"}
KEEP = 1  # nombre maximal de certificats de développement conservés


def token():
    now = int(time.time())
    key = open(os.environ["KEY_PATH"]).read()
    return jwt.encode(
        {"iss": os.environ["ISSUER_ID"], "iat": now, "exp": now + 600, "aud": "appstoreconnect-v1"},
        key,
        algorithm="ES256",
        headers={"kid": os.environ["KEY_ID"], "typ": "JWT"},
    )


def call(method, url):
    req = urllib.request.Request(url, method=method, headers={"Authorization": f"Bearer {token()}"})
    with urllib.request.urlopen(req, timeout=60) as r:
        body = r.read()
        return json.loads(body) if body else None


def main():
    data = call("GET", f"{API}/certificates?limit=200")["data"]
    dev = [c for c in data if c["attributes"].get("certificateType") in DEV_TYPES]
    dev.sort(key=lambda c: c["attributes"].get("expirationDate") or "")
    for c in dev:
        a = c["attributes"]
        print(f"certificat {a.get('certificateType')} « {a.get('name') or a.get('displayName')} » expire {a.get('expirationDate')}")
    api_made = [c for c in dev if "api" in (c["attributes"].get("name") or "").lower()
                or "api" in (c["attributes"].get("displayName") or "").lower()]
    to_revoke = list(api_made)
    rest = [c for c in dev if c not in to_revoke]
    while len(rest) > KEEP:
        to_revoke.append(rest.pop(0))  # le plus ancien d'abord
    for c in to_revoke:
        call("DELETE", f"{API}/certificates/{c['id']}")
        print(f"révoqué : {c['attributes'].get('name') or c['id']}")
    print(f"{len(to_revoke)} certificat(s) de développement révoqué(s), {len(dev) - len(to_revoke)} conservé(s)")


if __name__ == "__main__":
    main()
