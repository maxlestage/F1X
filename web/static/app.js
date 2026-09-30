// F1X — heures locales et compte à rebours.
(function () {
  "use strict";
  var fmtDate = new Intl.DateTimeFormat("fr-FR", { weekday: "short", day: "numeric", month: "short" });
  var fmtTime = new Intl.DateTimeFormat("fr-FR", { hour: "2-digit", minute: "2-digit" });

  document.querySelectorAll("time[data-local]").forEach(function (el) {
    var d = new Date(el.getAttribute("datetime"));
    if (isNaN(d)) return;
    var text = fmtDate.format(d);
    if (el.dataset.local === "datetime") text += " · " + fmtTime.format(d);
    el.textContent = text;
  });

  document.querySelectorAll("[data-countdown]").forEach(function (box) {
    var target = new Date(box.dataset.countdown).getTime();
    var cells = {};
    box.querySelectorAll("[data-unit]").forEach(function (c) { cells[c.dataset.unit] = c; });
    function pad(n) { return n < 10 ? "0" + n : String(n); }
    function tick() {
      var s = Math.max(0, Math.floor((target - Date.now()) / 1000));
      cells.j.textContent = Math.floor(s / 86400);
      cells.h.textContent = pad(Math.floor((s % 86400) / 3600));
      cells.m.textContent = pad(Math.floor((s % 3600) / 60));
      cells.s.textContent = pad(s % 60);
      return s;
    }
    if (tick() > 0) {
      var timer = setInterval(function () { if (tick() === 0) clearInterval(timer); }, 1000);
    }
  });
})();
