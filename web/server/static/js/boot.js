(function () {
  var started = false;
  window.__f1xCar = "/static/car.bin?v={{CAR}}";
  window.__f1xStarted = function () { started = true; };
  function fail(reason, force) {
    if (started && !force) return;
    var fr = !/^en/i.test(navigator.language || "");
    var app = document.getElementById("app");
    if (!app) return;
    app.innerHTML =
      '<div class="boot"><div class="boot-error">' +
      '<p><strong>' + (fr ? "F1X n'a pas pu démarrer." : "F1X could not start.") + '</strong></p>' +
      '<p>' + (fr ? "Recharge la page (la dernière version sera téléchargée)." : "Reload the page to get the latest version.") + '</p>' +
      '<button class="btn" onclick="location.reload()">' + (fr ? "Recharger" : "Reload") + '</button>' +
      '<small>' + String(reason).replace(/[<>&]/g, "") + '</small></div></div>';
  }
  window.__f1xFail = fail;
  import("/pkg/{{APP}}/f1x_frontend.js")
    .then(function (m) { return m.default({ module_or_path: "/pkg/{{APP}}/f1x_frontend_bg.wasm" }); })
    .catch(fail);
  setTimeout(function () { fail("timeout"); }, 25000);

  // Application installable : invite d'installation différée (Chrome, Edge, Android)…
  addEventListener("beforeinstallprompt", function (e) {
    e.preventDefault();
    window.__f1xDeferred = e;
    dispatchEvent(new Event("f1x-installable"));
  });
  addEventListener("appinstalled", function () { window.__f1xDeferred = null; });
  // … et service worker : démarrage instantané et consultation hors ligne.
  if ("serviceWorker" in navigator) {
    addEventListener("load", function () { navigator.serviceWorker.register("/sw.js").catch(function () {}); });
  }
})();
