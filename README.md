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

Heroku ne reconnaît pas Rust tout seul : il faut **une fois** lui indiquer le buildpack Rust
(`RustConfig` et `Procfile` à la racine du repo font le reste).

### Option 1 — Dashboard Heroku + GitHub (tout depuis Safari)

1. **dashboard.heroku.com** → ton app → **Settings** → **Buildpacks** → **Add buildpack**
   → colle `emk/rust` → **Save changes**.
2. Onglet **Deploy** → *Deployment method* **GitHub** → repo `maxlestage/f1x`
   → branche **master** → **Enable Automatic Deploys** (et/ou **Deploy Branch**).

Le premier build prend quelques minutes (compilation Rust), les suivants sont plus rapides grâce au cache.

### Option 2 — Bouton (repo public)

[![Deploy](https://www.herokucdn.com/deploy/button.svg)](https://heroku.com/deploy?template=https://github.com/maxlestage/f1x)

Le buildpack est déjà configuré dans `app.json`.

### Option 3 — GitHub Actions

Dans **github.com/maxlestage/f1x** → *Settings* → *Secrets and variables* → *Actions* :
secret `HEROKU_API_KEY` (heroku.com → *Account settings* → *API Key*) et variable `HEROKU_APP_NAME`.
Chaque merge sur `master` teste puis déploie (le workflow configure lui-même le buildpack) ;
lancement manuel possible depuis l'app GitHub : *Actions* → **Deploy Heroku** → *Run workflow*.
N'active pas en même temps l'option 1 (Automatic Deploys), sinon chaque merge déploie deux fois.

Le `Dockerfile` reste disponible pour un déploiement conteneur ailleurs.

**Stack :** `app.json` cible `heroku-26` (Ubuntu 26.04, la plus récente). Une app déjà créée reste sur
`heroku-24` (toujours supportée) : pour la passer en 26, dashboard → **Settings** → *Stack* → **Upgrade**,
puis redéploie.

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
