(function () {
  var root = document.documentElement, reduit = matchMedia("(prefers-reduced-motion: reduce)").matches;
  var souris = matchMedia("(hover: hover) and (pointer: fine)").matches;
  // Barre du haut plus dense au défilement ; barre de progression si le CSS ne sait pas la lier au défilement.
  var progres = !(window.CSS && CSS.supports && CSS.supports("animation-timeline: scroll()")), prevu = false;
  addEventListener("scroll", function () {
    if (prevu) return;
    prevu = true;
    requestAnimationFrame(function () {
      prevu = false;
      var y = scrollY, h = root.scrollHeight - innerHeight;
      root.toggleAttribute("data-defile", y > 24);
      if (progres) root.style.setProperty("--defile", h > 0 ? Math.min(1, y / h).toFixed(4) : 0);
    });
  }, { passive: true });
  // Blocs qui entrent en scène en arrivant à l'écran (en cascade s'ils arrivent ensemble).
  window.__f1xVu = 1;
  if (root.classList.contains("anime")) {
    var vus = new IntersectionObserver(function (es) {
      var k = 0;
      es.forEach(function (e) {
        if (!e.isIntersecting) return;
        vus.unobserve(e.target);
        e.target.style.setProperty("--d", Math.min(k++, 6) * 90 + "ms");
        e.target.classList.add("vu");
      });
    }, { rootMargin: "0px 0px -8% 0px" });
    document.querySelectorAll(".numbers li, .feature, .band, .final, .doc > *, .foot-grid > *, .foot-bottom, .foot-legal").forEach(function (el) { vus.observe(el); });
  }
  if (souris) {
    // Curseur anneau qui grossit sur ce qui se clique.
    var c = document.querySelector(".curseur"), x = -100, y = -100, cx = x, cy = y;
    addEventListener("mousemove", function (e) {
      x = e.clientX; y = e.clientY;
      c.classList.toggle("actif", !!(e.target.closest && e.target.closest("a, button")));
    }, { passive: true });
    (function boucle() {
      cx += (x - cx) * (reduit ? 1 : .22); cy += (y - cy) * (reduit ? 1 : .22);
      c.style.transform = "translate(" + cx + "px," + cy + "px)";
      requestAnimationFrame(boucle);
    })();
    if (!reduit) {
      // Projecteur sur les cartes et téléphone de l'accueil qui s'incline vers la souris.
      document.addEventListener("pointermove", function (e) {
        var b = e.target.closest && e.target.closest(".mini, .numbers li, .steps li");
        if (!b) return;
        var r = b.getBoundingClientRect();
        b.style.setProperty("--mx", Math.round(e.clientX - r.left) + "px");
        b.style.setProperty("--my", Math.round(e.clientY - r.top) + "px");
      }, { passive: true });
      var tel = document.querySelector(".hero-shot");
      if (tel) addEventListener("mousemove", function (e) {
        var kx = e.clientX / innerWidth - .5, ky = e.clientY / innerHeight - .5;
        tel.style.setProperty("--ry", (kx * 14).toFixed(2) + "deg");
        tel.style.setProperty("--rx", (-ky * 10).toFixed(2) + "deg");
      }, { passive: true });
    }
  }
  // Chiffres clés : comptent depuis 0 quand ils arrivent à l'écran (valeur finale déjà affichée sans script).
  if (!matchMedia("(prefers-reduced-motion: reduce)").matches && "IntersectionObserver" in window) {
    var io = new IntersectionObserver(function (es) {
      es.forEach(function (e) {
        if (!e.isIntersecting) return;
        io.unobserve(e.target);
        var el = e.target, final = el.textContent, n = parseInt(final.replace(/\D/g, ""), 10);
        if (!n) return;
        var t0 = performance.now();
        (function tick(t) {
          var k = Math.min(1, (t - t0) / 1400), v = Math.round(n * (1 - Math.pow(1 - k, 3)));
          el.textContent = k < 1 ? final.replace(/[\d\s\u202f]+/, v.toLocaleString(document.documentElement.lang) + (/\s$/.test(final) ? " " : "")) : final;
          if (k < 1) requestAnimationFrame(tick);
        })(t0);
      });
    }, { threshold: .6 });
    document.querySelectorAll(".numbers strong").forEach(function (el) { io.observe(el); });
  }
  // Boutons aimantés : suivent légèrement le pointeur (souris uniquement).
  if (matchMedia("(hover: hover) and (pointer: fine)").matches) {
    document.querySelectorAll(".btn, .lang").forEach(function (b) {
      b.addEventListener("pointermove", function (e) {
        var r = b.getBoundingClientRect();
        b.style.transform = "translate(" + (e.clientX - r.left - r.width / 2) * .25 + "px," + (e.clientY - r.top - r.height / 2) * .35 + "px)";
      });
      b.addEventListener("pointerleave", function () { b.style.transform = ""; });
    });
  }
})();
