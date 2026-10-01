// Service worker F1X : démarrage instantané et consultation hors ligne.
// Les empreintes {{APP}} / {{CSS}} changent à chaque version : un nouveau déploiement
// installe un nouveau cache et supprime l'ancien.
const VERSION = "{{APP}}-{{CSS}}";
const SHELL = "f1x-shell-" + VERSION;
const DATA = "f1x-data";
const PRECACHE = [
  "/",
  "/static/app.css?v={{CSS}}",
  "/pkg/{{APP}}/f1x_frontend.js",
  "/pkg/{{APP}}/f1x_frontend_bg.wasm",
  "/manifest.webmanifest",
  "/static/icon.svg",
  "/static/img/icon-192.png",
  "/static/img/icon-512.png",
  "/favicon.ico",
];
const MAX_DATA = 300;

self.addEventListener("install", (event) => {
  event.waitUntil(caches.open(SHELL).then((c) => c.addAll(PRECACHE)).then(() => self.skipWaiting()));
});

self.addEventListener("activate", (event) => {
  event.waitUntil(
    caches.keys()
      .then((keys) => Promise.all(keys.filter((k) => k.startsWith("f1x-shell-") && k !== SHELL).map((k) => caches.delete(k))))
      .then(() => self.clients.claim()),
  );
});

async function trim(name, max) {
  const cache = await caches.open(name);
  const keys = await cache.keys();
  for (let i = 0; i < keys.length - max; i++) await cache.delete(keys[i]);
}

// Réseau d'abord, copie locale si hors ligne.
async function networkFirst(request, cacheName, fallbackUrl) {
  try {
    const response = await fetch(request);
    if (response.ok) {
      const cache = await caches.open(cacheName);
      cache.put(fallbackUrl || request, response.clone());
      if (cacheName === DATA) trim(DATA, MAX_DATA);
    }
    return response;
  } catch (err) {
    const cached = await caches.match(fallbackUrl || request);
    if (cached) return cached;
    throw err;
  }
}

// Cache d'abord (fichiers dont l'adresse change avec le contenu).
async function cacheFirst(request, cacheName) {
  const cached = await caches.match(request);
  if (cached) return cached;
  const response = await fetch(request);
  if (response.ok) {
    const cache = await caches.open(cacheName);
    cache.put(request, response.clone());
  }
  return response;
}

self.addEventListener("fetch", (event) => {
  const request = event.request;
  if (request.method !== "GET") return;
  const url = new URL(request.url);

  if (url.origin === self.location.origin) {
    // Pages de l'app (monopage) : toujours la dernière version, le shell en secours hors ligne.
    if (request.mode === "navigate") {
      if (url.pathname.startsWith("/presentation") || ["/mentions-legales", "/confidentialite", "/credits"].includes(url.pathname)) {
        event.respondWith(networkFirst(request, DATA));
      } else {
        event.respondWith(networkFirst(request, SHELL, "/"));
      }
      return;
    }
    if (url.pathname.startsWith("/pkg/") || url.pathname.startsWith("/static/")) {
      event.respondWith(cacheFirst(request, SHELL));
      return;
    }
    if (url.pathname.startsWith("/api/")) {
      event.respondWith(networkFirst(request, DATA));
      return;
    }
  }
  // Ressources externes (photos Commons, carte, météo) : laissées au cache du navigateur.
});
