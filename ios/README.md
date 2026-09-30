# F1X — app iOS native (SwiftUI)

Projet au format **App Swift Playgrounds** (`F1X.swiftpm`) : il s'ouvre tel quel

- sur **iPad** dans l'app *Swift Playgrounds* (lancer, modifier, et même publier sur TestFlight / App Store),
- sur **Mac** dans *Xcode* (double-clic sur `F1X.swiftpm`).

iOS 17 minimum. Aucune dépendance externe.

## Écrans

- **Accueil** : prochain Grand Prix, compte à rebours en direct, programme du week-end à l'heure locale,
  podium de la dernière course, top 5 pilotes et top 3 écuries.
- **Calendrier** : toute la saison (manches passées grisées, prochaine mise en avant, week-ends sprint).
- **Détail GP** : programme, résultats course / sprint / qualifications, meilleur tour.
- **Pilotes** / **Écuries** : classements du championnat, avec couleurs d'écurie.
- **Fiche pilote** : position, points, victoires et résultat de chaque manche.

Tirer vers le bas pour rafraîchir. Les données viennent de l'API Jolpica F1 (cache 5 min,
dernière copie servie hors ligne).

## Pas de défilement horizontal

Uniquement des listes verticales. Les lignes « nom + heure » utilisent `ViewThatFits` pour passer sur
deux lignes plutôt que déborder ; les cases (compte à rebours, podium, stats) se partagent la largeur
de l'écran à parts égales et réduisent leur texte si besoin.

## Structure

```
F1X.swiftpm/
├── Package.swift          # produit .iOSApplication (bundle com.maxlestage.f1x)
├── App/F1XApp.swift       # point d'entrée + barre d'onglets
├── Data/                  # modèles + client API (actor, cache)
└── Views/                 # écrans SwiftUI et composants
```

Pour publier : dans Xcode / Swift Playgrounds, renseigne ton *Team* Apple Developer (le champ
`teamIdentifier` du `Package.swift` est vide par défaut).
