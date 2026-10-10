// Animations d'entrée prévues dès le départ (sinon tout reste visible) ; filet de sécurité si le script de fin ne passe pas.
(function () {
  var r = document.documentElement;
  if (matchMedia("(prefers-reduced-motion: reduce)").matches || !("IntersectionObserver" in window)) return;
  r.classList.add("anime");
  setTimeout(function () { if (!window.__f1xVu) r.classList.remove("anime"); }, 3000);
})();
