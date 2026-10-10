(function () {
  var c = document.querySelector(".curseur");
  if (!c || !matchMedia("(hover: hover) and (pointer: fine)").matches) return;
  var x = -100, y = -100, cx = x, cy = y, slow = !matchMedia("(prefers-reduced-motion: reduce)").matches;
  addEventListener("mousemove", function (e) {
    x = e.clientX; y = e.clientY;
    c.classList.toggle("actif", !!(e.target.closest && e.target.closest("a, button, select, summary, [role=button], input[type=checkbox]")));
  }, { passive: true });
  (function loop() {
    cx += (x - cx) * (slow ? .22 : 1); cy += (y - cy) * (slow ? .22 : 1);
    c.style.transform = "translate(" + cx + "px," + cy + "px)";
    requestAnimationFrame(loop);
  })();
  // Boutons aimantés : suivent légèrement le pointeur.
  document.addEventListener("pointermove", function (e) {
    var b = e.target.closest && e.target.closest(".btn, .site-link, .lang-switch, .pill");
    if (window.__aimant && window.__aimant !== b) { window.__aimant.style.transform = ""; }
    window.__aimant = b;
    if (!b) return;
    var r = b.getBoundingClientRect();
    b.style.transform = "translate(" + (e.clientX - r.left - r.width / 2) * .22 + "px," + (e.clientY - r.top - r.height / 2) * .3 + "px)";
  }, { passive: true });
})();
// Compte à rebours : le chiffre qui change bascule.
(function () {
  if (matchMedia("(prefers-reduced-motion: reduce)").matches) return;
  new MutationObserver(function (list) {
    list.forEach(function (m) {
      var el = m.target.nodeType === 3 ? m.target.parentElement : m.target;
      var n = el && el.closest && el.closest(".cd-num");
      if (!n) return;
      n.classList.remove("tic"); void n.offsetWidth; n.classList.add("tic");
    });
  }).observe(document.body, { subtree: true, characterData: true, childList: true });
})();
// Moteur d'animations : tout ce que l'app (active) affiche entre en scène en cascade quand il arrive à
// l'écran, les chiffres comptent, l'écran répond au toucher. Uniquement des attributs data-*
// et des variables CSS : rien que l'app ne gère elle-même. Sans ce script, tout reste visible.
(function () {
  var root = document.documentElement;
  var reduit = matchMedia("(prefers-reduced-motion: reduce)").matches;
  // Barre du haut plus dense dès qu'on défile ; barre de progression si le CSS ne sait pas la faire seul.
  var progres = !(window.CSS && CSS.supports && CSS.supports("animation-timeline: scroll()"));
  var prevu = false;
  function defile() {
    prevu = false;
    var y = scrollY, h = root.scrollHeight - innerHeight;
    root.toggleAttribute("data-defile", y > 24);
    if (progres) root.style.setProperty("--defile", h > 0 ? Math.min(1, y / h).toFixed(4) : 0);
  }
  addEventListener("scroll", function () { if (!prevu) { prevu = true; requestAnimationFrame(defile); } }, { passive: true });
  if (reduit || !("IntersectionObserver" in window) || !("MutationObserver" in window)) return;
  root.classList.add("anime");

  // 1. Révélation en cascade : chaque bloc et chaque ligne de liste apparaît en entrant à l'écran.
  var CIBLES = [
    ".page > :not(.hero):not(h1):not(.toasts):not(.banner)",
    ".rows > li", ".race-list > li", ".news > li", ".glossary > .card", ".timeline > li", ".wx-days > li",
    ".strategy > li", ".cmp > li", ".quiz-options > *", ".year-pills > *", ".chips > *", ".race-data-grid > *",
    ".suggestions > li", ".legend-list > li", ".map-actions > *", ".app-foot > *"
  ].join(",");
  var vus = new IntersectionObserver(function (entrees) {
    var rang = 0;
    entrees.forEach(function (e) {
      if (!e.isIntersecting) return;
      vus.unobserve(e.target);
      e.target.style.setProperty("--d", Math.min(rang++, 10) * 55 + "ms");
      e.target.setAttribute("data-rv", "1");
    });
  }, { rootMargin: "0px 0px -6% 0px" });

  // 2. Chiffres qui comptent depuis 0 (points, statistiques) quand ils deviennent visibles.
  function compte(el) {
    var n = el.firstChild;
    if (!n || n.nodeType !== 3) return;
    var fin = n.nodeValue, m = /^(\D*?)(\d[\d\s  ]*\d|\d)(\D*)$/.exec(fin);
    if (!m) return;
    var v = parseInt(m[2].replace(/\D/g, ""), 10);
    // Pas les années (« 2008 ») ni les tout petits nombres.
    if (!(v > 2) || (/^(19|20)\d\d$/.test(m[2]) && !m[1].trim() && !m[3].trim())) return;
    var groupe = /\D/.test(m[2]), lang = root.lang || "fr", t0 = performance.now(), dernier = fin;
    (function pas(t) {
      if (n.nodeValue !== dernier) return; // l'app a mis le chiffre à jour entre-temps : on le laisse.
      var k = Math.min(1, (t - t0) / 1100), x = Math.round(v * (1 - Math.pow(1 - k, 3)));
      dernier = k < 1 ? m[1] + (groupe ? x.toLocaleString(lang) : x) + m[3] : fin;
      n.nodeValue = dernier;
      if (k < 1) requestAnimationFrame(pas);
    })(t0);
  }
  var chiffres = new IntersectionObserver(function (entrees) {
    entrees.forEach(function (e) {
      if (!e.isIntersecting) return;
      chiffres.unobserve(e.target);
      compte(e.target);
    });
  }, { threshold: .5 });

  function scrute(noeud) {
    if (noeud.nodeType !== 1) return;
    var liste = [].slice.call(noeud.querySelectorAll(CIBLES));
    if (noeud.matches(CIBLES)) liste.unshift(noeud);
    liste.forEach(function (el) {
      if (el.hasAttribute("data-rv")) return;
      el.setAttribute("data-rv", "0");
      vus.observe(el);
    });
    var nb = [].slice.call(noeud.querySelectorAll(".stats dd, .pts"));
    if (noeud.matches(".stats dd, .pts")) nb.push(noeud);
    nb.forEach(function (el) { if (!el.__compte) { el.__compte = 1; chiffres.observe(el); } });
  }
  new MutationObserver(function (liste) {
    liste.forEach(function (m) {
      m.addedNodes.forEach(scrute);
      // Valeur arrivée après coup (« – » pendant le chargement) : elle compte à son tour.
      if (m.type === "characterData" && /^[–-]$/.test((m.oldValue || "").trim())) {
        var p = m.target.parentElement;
        if (p && p.matches(".stats dd, .pts")) compte(p);
      }
    });
  }).observe(document.body, { childList: true, subtree: true, characterData: true, characterDataOldValue: true });

  // 3. Onde rouge à l'endroit touché, sur tout ce qui se touche.
  document.addEventListener("pointerdown", function (e) {
    if (e.button > 0 || !e.target.closest) return;
    if (!e.target.closest("a, button, summary, select, label, [role=button], .row, .race-item")) return;
    var o = document.createElement("span");
    o.className = "onde";
    o.style.left = e.clientX + "px";
    o.style.top = e.clientY + "px";
    document.body.appendChild(o);
    setTimeout(function () { o.remove(); }, 700);
  }, { passive: true });

  // 4. Projecteur : une lueur suit la souris sur les cartes (ordinateur).
  if (matchMedia("(hover: hover) and (pointer: fine)").matches) {
    document.addEventListener("pointermove", function (e) {
      var c = e.target.closest && e.target.closest(".card, .race-item, .news-item, .rows-card");
      if (!c) return;
      var r = c.getBoundingClientRect();
      c.style.setProperty("--mx", Math.round(e.clientX - r.left) + "px");
      c.style.setProperty("--my", Math.round(e.clientY - r.top) + "px");
    }, { passive: true });
  }

  // 5. Photos : se dévoilent (net + zoom) au moment où elles arrivent.
  document.addEventListener("load", function (e) {
    if (e.target.tagName === "IMG") e.target.setAttribute("data-charge", "");
  }, true);
})();
