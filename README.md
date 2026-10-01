# F1X 🏎️

La Formule 1 dans ta poche — **uniquement de la F1**, de 1950 à aujourd'hui : prochain Grand Prix
avec compte à rebours, programme du week-end (à ton heure locale), résultats course / sprint / qualifs,
meilleurs tours, arrêts aux stands, analyse tour par tour, classements de toutes les saisons, carrières
des pilotes, palmarès des écuries et des circuits (avec leur **tracé GPS coloré selon la vitesse**) — et du
**temps réel en WebSocket** (direct et replays). **Français / English** : bouton FR/EN dans la barre du haut.

| | |
|---|---|
| 🏁 **Site de présentation** | `/presentation` — page vitrine FR/EN (HTML + CSS rendus par le serveur Rust, sans JavaScript) avec captures réelles et bouton « Voir l'app web » |
| 📱 **App iOS native** | SwiftUI, dans [`ios/F1X.swiftpm`](ios/) |
| 🌐 **Site web mobile first** | 100 % Rust : front **Yew 0.23** (WebAssembly) + serveur **axum**, dans [`web/`](web/) |
| ☁️ **Déploiement** | Heroku, pilotable 100 % depuis un téléphone |

Données : API publique [Jolpica F1](https://github.com/jolpica/jolpica-f1) (successeur d'Ergast), sans clé.

## 🚫 Zéro défilement horizontal

C'est une règle du projet, appliquée des deux côtés :

- **Web (Yew)** : tout est empilé verticalement (pas de tableau, pas de carrousel), `overflow-x: hidden`,
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

Le premier build prend quelques minutes (compilation Rust du serveur et du front WebAssembly), les suivants sont plus rapides grâce au cache.

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

## 🌐 Site web (100 % Rust)

```
web/
├── Cargo.toml        # workspace
├── frontend/         # Yew 0.23 + yew-router 0.20 → WebAssembly (application monopage)
├── protocol/         # messages WebSocket partagés front ↔ serveur
└── server/           # axum : proxy /api avec cache + sert le front
    └── build.rs      # compile frontend/ en wasm + wasm-bindgen, embarqué dans le binaire
```

- **Front** : tout en Rust avec [Yew](https://yew.rs) (composants, routeur, hooks, compte à rebours,
  heures locales via l'API `Intl` du navigateur). Le seul JavaScript est le petit chargeur généré par
  wasm-bindgen qui démarre le module WebAssembly.
- **Serveur** : axum sert le shell HTML, le `.wasm` (≈ 200 Ko gzip) et une API JSON `/api/*` qui met
  en cache l'API Jolpica (et sert la dernière copie connue si elle tombe).
- **Build** : un simple `cargo build` suffit — pas de trunk, pas de npm. `build.rs` installe la cible
  `wasm32-unknown-unknown` si besoin, compile le front et génère le glue avec
  `wasm-bindgen-cli-support` (même version que `wasm-bindgen`, épinglée dans le workspace).

```sh
cd web
cargo run            # http://localhost:3000
```

### Pages

| Route | Contenu | Endpoints Jolpica |
|---|---|---|
| `/` | Prochain GP + compte à rebours, dernier podium, top 5 pilotes, top 3 écuries | `current`, `current/last/results`, standings |
| `/saison/{année\|current}` | Calendrier + vainqueur de chaque GP, sélecteur de saison (1950 →) | `{saison}`, `{saison}/results/1`, `seasons` |
| `/saison/{s}/pilotes`, `/saison/{s}/ecuries` | Classements de n'importe quelle saison | `driverStandings`, `constructorStandings` |
| `/saison/{s}/course/{manche}` | Programme, course (places gagnées/perdues), sprint, qualifs, meilleurs tours, arrêts aux stands, tours en tête + position tour par tour (graphique), bilan des abandons | `results`, `sprint`, `qualifying`, `pitstops`, `laps`, `status` |
| `/pilote/{id}` | Carrière (départs, victoires, podiums, poles, meilleurs tours, saisons), écuries, saison par saison | `drivers/{id}/results` (toutes les pages), `driverStandings` |
| `/ecurie/{id}` | Palmarès, pilotes et résultats saison par saison | `constructors/{id}/…` (`results/1-3`, `grid/1`, `seasons`, `constructorStandings`) |
| `/circuit/{id}` | **Tracé** reconstitué depuis un vrai tour (GPS OpenF1) coloré par la vitesse, télémétrie au toucher (vitesse, rapport, gaz, freinage), stats du tour (longueur, V max/mini/moyenne, % à fond, % freinage) ; carte OpenStreetMap ; prochain GP ; record, victoires depuis la pole, rois du circuit, écuries, palmarès complet | `circuits/{id}/races`, `results/1`, `fastest/1/results` + OpenF1 `laps`, `location`, `car_data` |
| `/archives` (+ `/saisons`, `/pilotes`, `/ecuries`, `/circuits`) | Chiffres clés, causes d'abandon, listes complètes avec recherche | `seasons`, `races`, `drivers`, `constructors`, `circuits`, `status` |

Les anciennes adresses (`/calendrier`, `/course/{manche}`, `/pilotes`, `/ecuries`) restent valides.

### 🏎️ 3D et photos (gratuit, sans service externe payant)

- **Monoplaces en 3D** (pages écurie et pilote) : F1 stylisée générée par le code aux couleurs de
  l'écurie (aucun modèle officiel), rendue en **WebGL 2 depuis Rust** (`frontend/src/gl3d.rs`).
  Glisser horizontalement pour la faire tourner ; le défilement vertical de la page reste libre.
- **Circuits en relief** (fiche circuit, page GP) : le tracé GPS OpenF1 avec son **altitude réelle**
  (dénivelé exagéré ×4), coloré par la vitesse ; une voiture rejoue le meilleur tour à vitesse réelle,
  avec une **caméra embarquée**.
- **Race Center** : vue « Relief 3D » de la carte, les voitures roulent sur le circuit aux couleurs
  de leur écurie.
- **Photos et avatars** : images libres de **Wikimedia Commons** (crédit et licence en lien), via
  `/api/photo/{titre}` (cache serveur partagé, une requête Wikipédia par photo). Sinon, initiales
  sur la couleur de l'écurie.

### 🧭 Explorer (outils et jeux, sans coût supplémentaire)

| Page | Contenu |
|---|---|
| `/actus` | Titres de la presse F1 (flux RSS publics : Motorsport.com, Autosport, RaceFans…, FR ou EN) |
| `/archives/records` | Titres, victoires, podiums, poles, meilleurs tours, séries, âges, champions de chaque saison |
| `/comparer/{a}/{b}` | Deux pilotes côte à côte + face-à-face (course, coéquipiers, grille), export CSV |
| `/pronostics` | Pronostic du prochain GP (fermé au début des qualifs), noté automatiquement |
| `/fantasy` | 5 pilotes + 1 écurie, 100 M€, points réels (course + sprint) |
| `/quiz` | « Devine le pilote » d'après ses statistiques |
| `/lexique` | Lexique bilingue (drapeaux, pneus, stratégie, règlement 2026…) |

Pronostics, Fantasy, favoris (★), score du quiz et réglages d'alertes sont **enregistrés sur le
téléphone** (pas de compte, pas de base de données). La météo vient d'Open-Meteo, appelé directement
par le navigateur. Les champions 1950-2025 sont dans `server/static/champions.json` (le passé ne change
pas) ; seule la saison en cours est demandée à l'API.

### 🌍 Français / English

Toute l'interface existe dans les deux langues (dates au format local, statuts de course, noms de
sessions et de pays). Langue par défaut = celle du téléphone ; le choix FR/EN est mémorisé. Côté code,
les textes sont écrits en paires : `t("Calendrier", "Calendar")`, `tr!("Saison {s}", "Season {s}")`
(module `frontend/src/i18n.rs`).

### ⚡ Direct & replay en WebSocket (`/direct`)

Une connexion WebSocket (`/ws`) relie le front Yew au serveur axum ; le protocole est un crate partagé
(`web/protocol`), donc typé des deux côtés. Chaque seconde le serveur pousse une photo complète de la
course : classement, écarts (au leader et à la voiture de devant), dernier/meilleur tour, pneus (gomme +
âge), arrêts et passages aux stands, drapeaux / voiture de sécurité, météo, messages de la direction de
course. Le nombre de personnes connectées est diffusé en temps réel.

- **Race Center** : vues Classement (écarts étiquetés, secteurs violet/vert/jaune, pneus, légende),
  Carte (voitures placées sur le tracé), Stratégie (relais de pneus, « et s'il s'arrêtait maintenant ? »),
  Chronologie (dépassements, arrêts, meilleurs tours, drapeaux, pénalités, abandons) + alertes réglables.
- **Replay (gratuit)** : rejoue n'importe quelle session depuis 2023 (essais, qualifs, sprint, course) comme
  en direct, de ×1 à ×60, avec pause et ±2 min. Données [OpenF1](https://openf1.org) (historique gratuit).
- **Direct** : pendant une session, le serveur interroge OpenF1 toutes les ~4 s et diffuse à tous les
  connectés. Le temps réel d'OpenF1 est **réservé aux abonnés** : renseigne `OPENF1_USERNAME` et
  `OPENF1_PASSWORD` (Heroku → *Settings* → *Config Vars*). Sans eux, la page propose le replay.
- Reconnexion automatique côté navigateur (reprend le suivi en cours), signe de vie toutes les 25 s
  (Heroku ferme les WebSockets inactives après 55 s), débit OpenF1 limité côté serveur (30 req/min en
  gratuit), données de session partagées entre spectateurs (chargées une seule fois).

### API du serveur

| Route | Rôle |
|---|---|
| `/api/f1/{chemin}.json?limit=&offset=` | N'importe quel endpoint Jolpica (chemin validé) |
| `/api/photo/{titre}` | Photo libre (Wikimedia Commons) d'un article Wikipédia `{src, credit}`, 404 sinon |
| `/api/all/{chemin}.json` | Toutes les pages d'un endpoint, fusionnées côté serveur |
| `/api/live/sessions/{année}` | Sessions rejouables (OpenF1) |
| `/api/track/{circuit}` | Tracé GPS + télémétrie du meilleur tour de la dernière course (depuis 2023), mis en cache |
| `/ws` | WebSocket direct / replay (messages JSON, voir `web/protocol`) |
| `/healthz` | Health check |

**Quota Jolpica** (500 requêtes/heure, 4/s) : le serveur espace ses appels, met en cache
(saisons passées 7 jours, carrières/listes 1 h, saison en cours 5 min), sert la dernière copie connue
en cas de panne et coupe les appels pendant la durée demandée après un `429`. Les pages sont conçues
pour limiter les requêtes (ex. la carrière d'un pilote est calculée à partir d'un seul jeu de résultats ;
l'analyse tour par tour ne se charge qu'à la demande).

Variables d'environnement : `PORT` (fourni par Heroku), `CACHE_TTL_SECS` (saison en cours, défaut 300),
`OPENF1_USERNAME` / `OPENF1_PASSWORD` (optionnels, direct OpenF1), `CACHE_FILE` (optionnel : sauvegarde le cache à l'arrêt et le recharge au démarrage, pratique en local).
Installable sur l'écran d'accueil (manifest web).

## 📱 App iOS

Voir [`ios/README.md`](ios/README.md).
