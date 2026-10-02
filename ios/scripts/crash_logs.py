#!/usr/bin/env python3
"""Affiche les derniers rapports de plantage TestFlight (API App Store Connect).

Quand un testeur touche « Partager » après un plantage, Apple enregistre le rapport ; ce script
les récupère (betaFeedbackCrashSubmissions + crashLog) pour diagnostiquer sans Xcode.
Variables : KEY_ID, ISSUER_ID, KEY_PATH. Bundle : com.maxlestage.f1x.
"""
import json
import os
import time
import urllib.error
import urllib.request

import jwt

API = "https://api.appstoreconnect.apple.com/v1"
BUNDLE = os.environ.get("BUNDLE_ID", "com.maxlestage.f1x")


def token():
    now = int(time.time())
    return jwt.encode(
        {"iss": os.environ["ISSUER_ID"], "iat": now, "exp": now + 600, "aud": "appstoreconnect-v1"},
        open(os.environ["KEY_PATH"]).read(),
        algorithm="ES256",
        headers={"kid": os.environ["KEY_ID"], "typ": "JWT"},
    )


def get(url):
    req = urllib.request.Request(url if url.startswith("http") else API + url, headers={"Authorization": f"Bearer {token()}"})
    try:
        with urllib.request.urlopen(req, timeout=60) as r:
            return json.loads(r.read())
    except urllib.error.HTTPError as e:
        print(f"HTTP {e.code} sur {url} : {e.read()[:500]!r}")
        return None


apps = get(f"/apps?filter[bundleId]={BUNDLE}")
if not apps or not apps["data"]:
    raise SystemExit("App introuvable")
app_id = apps["data"][0]["id"]
subs = get(f"/apps/{app_id}/betaFeedbackCrashSubmissions?limit=5&sort=-createdDate")
if not subs or not subs.get("data"):
    print("Aucun rapport de plantage partagé pour l'instant (toucher « Partager » après un plantage).")
    raise SystemExit(0)
for sub in subs["data"]:
    a = sub["attributes"]
    print("=" * 80)
    print(f"Plantage {sub['id']} · {a.get('createdDate')} · build {a.get('buildBundleId', '')} · "
          f"{a.get('deviceModel')} iOS {a.get('osVersion')} · commentaire : {a.get('comment')!r}")
    log = get(f"/betaFeedbackCrashSubmissions/{sub['id']}/crashLog")
    text = ((log or {}).get("data") or {}).get("attributes", {}).get("logText", "")
    lines = text.splitlines()
    # En-tête, exception et pile du fil qui a planté (les lignes utiles du rapport).
    keep, crashed = [], False
    for ln in lines:
        if any(k in ln for k in ("Exception Type", "Exception Codes", "Termination Reason", "Crashed Thread",
                                 "Application Specific Information", "Fatal error", "fatal error", "Version:", "OS Version")):
            keep.append(ln)
        if "Crashed:" in ln:
            crashed = True
        if crashed:
            keep.append(ln)
            if ln.strip() == "" and len(keep) > 10:
                crashed = False
    print("\n".join(keep[:160]) if keep else "\n".join(lines[:200]))
