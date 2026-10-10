# F1X — consignes pour Claude

## Livraison : fusion et TestFlight automatiques

Le propriétaire du dépôt veut que chaque modification terminée soit livrée sans qu'on lui
demande à chaque fois :

1. Travailler sur la branche de la session, ouvrir une PR vers `master`.
2. Vérifier avant de fusionner :
   - site : `cargo clippy` (front en `wasm32-unknown-unknown` et serveur) et `cargo test` dans `web/` ;
   - app iOS : le workflow « Compilation iOS » (`.github/workflows/ios-build.yml`) doit être vert ;
     ses captures de chaque page sont publiées sur la branche `captures-ios`, à regarder.
3. Si tout est vert, **fusionner la PR dans `master` sans redemander** : la fusion déploie le site
   sur Heroku et envoie l'app sur TestFlight.
4. Si une vérification échoue, corriger d'abord ; ne jamais fusionner du rouge.

## Rappels du projet

- Interface en français et en anglais (`L("fr", "en")` côté iOS, `t(...)` / `tr!` côté site).
- Aucun défilement horizontal, sur le site comme dans l'app.
- « Réduire les animations » doit toujours être respecté.
