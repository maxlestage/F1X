// Carte Apple Maps (MapKit JS) : jeton signé par le serveur ; rejet = repli OpenStreetMap.
window.f1xAppleMap = function (el, lat, lon, name) {
  return fetch("/api/mapkit-token").then(function (r) {
    if (!r.ok) throw new Error("mapkit");
    return r.text();
  }).then(function (tok) {
    return new Promise(function (resolve, reject) {
      function show() {
        if (!window.__f1xMapkit) {
          mapkit.init({ authorizationCallback: function (done) { done(tok); }, language: document.documentElement.lang || "fr" });
          window.__f1xMapkit = true;
        }
        var c = new mapkit.Coordinate(lat, lon);
        var map = new mapkit.Map(el, { mapType: mapkit.Map.MapTypes.Hybrid, showsCompass: mapkit.FeatureVisibility.Adaptive, colorScheme: mapkit.Map.ColorSchemes.Dark });
        map.region = new mapkit.CoordinateRegion(c, new mapkit.CoordinateSpan(0.022, 0.032));
        map.addAnnotation(new mapkit.MarkerAnnotation(c, { title: name, color: "#e10600", glyphText: "🏁" }));
        resolve();
      }
      if (window.mapkit && window.mapkit.Map) return show();
      var s = document.createElement("script");
      s.src = "https://cdn.apple-mapkit.com/mk/5.x.x/mapkit.js";
      s.crossOrigin = "anonymous";
      s.onload = show;
      s.onerror = reject;
      document.head.appendChild(s);
    });
  });
};
