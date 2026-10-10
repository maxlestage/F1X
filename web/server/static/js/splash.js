// Animation de démarrage : une fois par session, touchée = passée, retirée à la fin.
(function () {
  var s = document.getElementById("splash");
  var seen = false;
  try { seen = sessionStorage.getItem("f1x-splash") === "1"; sessionStorage.setItem("f1x-splash", "1"); } catch (e) {}
  if (seen) { s.remove(); return; }
  var done = function () { if (s.parentNode) s.remove(); };
  s.addEventListener("click", done);
  // Compteur 0 → 100 % pendant que le signe se trace.
  var n = document.getElementById("porte-n"), t0 = performance.now();
  (function tick(t) {
    var k = Math.min(1, (t - t0) / 1350);
    n.textContent = Math.round(100 * (1 - Math.pow(1 - k, 3)));
    if (k < 1 && s.parentNode) requestAnimationFrame(tick);
  })(t0);
  setTimeout(done, 2100);
})();
