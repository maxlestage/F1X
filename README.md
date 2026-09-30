# F1X 🏎️

La Formule 1 dans ta poche — **uniquement de la F1** : prochain Grand Prix avec compte à rebours,
programme du week-end (à ton heure locale), résultats course / sprint / qualifs, classements pilotes
et écuries, fiche saison de chaque pilote.

| | |
|---|---|
| 📱 **App iOS native** | SwiftUI, dans [`ios/F1X.swiftpm`](ios/) |
| 🌐 **Site web mobile first** | Rust (axum + maud), dans [`web/`](web/) |
| ☁️ **Déploiement** | Heroku, pilotable 100 % depuis un téléphone |

Données : API publique [Jolpica F1](https://github.com/jolpica/jolpica-f1) (successeur d'Ergast), sans clé.

## 🚫 Zéro défilement horizontal

C'est une règle du projet, appliquée des deux côtés :

- **Web** : tout est empilé verticalement (pas de tableau, pas de carrousel), `overflow-x: hidden`,
  `min-width: 0` partout, texte long qui passe à la ligne (`overflow-wrap: anywhere`), grilles en
  `minmax(0, 1fr)`. Vérifié automatiquement sur toutes les pages à 320, 375 et 430 px de large.
- **iOS** : uniquement des `List` / `ScrollView(.vertical)`, aucune `ScrollView(.horizontal)` ni
  `TabView` en mode page ; les lignes utilisent `ViewThatFits` et passent sur 2 lignes si besoin.

## ☁️ Déployer le site sur Heroku depuis ton téléphone

### Option 1 — Bouton (le plus simple, repo public)

[![Deploy](https://www.herokucdn.com/deploy/button.svg)](https://heroku.com/deploy?template=https://github.com/maxlestage/f1x)

Touche le bouton depuis Safari → choisis un nom d'app → **Deploy app**. Heroku construit l'image
Docker (`heroku.yml`) et l'app est en ligne.

### Option 2 — Auto-déploiement à chaque merge (GitHub Actions)

À faire une seule fois, depuis le navigateur du téléphone :

1. **heroku.com** → *New* → *Create new app* (ex. `f1x-max`).
2. **heroku.com** → *Account settings* → *API Key* → *Reveal* et copie la clé.
3. **github.com/maxlestage/f1x** → *Settings* → *Secrets and variables* → *Actions* :
   - secret `HEROKU_API_KEY` = la clé copiée ;
   - onglet *Variables* : `HEROKU_APP_NAME` = le nom de l'app.

Ensuite, chaque merge sur `main` (depuis l'app GitHub) teste puis déploie automatiquement.
Tu peux aussi lancer un déploiement à la main : app GitHub → *Actions* → **Deploy Heroku** → *Run workflow*.
Le workflow passe lui-même l'app en stack `container`, aucun CLI n'est nécessaire.

## 🌐 Site web (Rust)

```sh
cd web
cargo run            # http://localhost:3000
```

| Route | Contenu |
|---|---|
| `/` | Prochain GP + compte à rebours, dernier podium, top 5 pilotes, top 3 écuries |
| `/calendrier` | Toute la saison |
| `/course/{manche}` | Programme, résultats course, sprint et qualifications |
| `/pilotes`, `/ecuries` | Classements |
| `/pilote/{id}` | Saison d'un pilote |
| `/healthz` | Health check |

Variables d'environnement : `PORT` (fourni par Heroku), `CACHE_TTL_SECS` (défaut 300).
Le serveur met en cache les réponses de l'API et sert la dernière copie connue si elle tombe.
Installable sur l'écran d'accueil (manifest web).

## 📱 App iOS

Voir [`ios/README.md`](ios/README.md).
