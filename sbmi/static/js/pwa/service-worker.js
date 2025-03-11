/**
 * Progressive Web App functionality v1
 */

self.addEventListener("install", async (event) => {
    console.debug("[pwa-sw] installing...");
});

/**
 * Listen for the activate event, which is fired after installation
 * Activate is when the service worker actually takes over from the previous
 * version, which is a good time to clean up old caches.
 * Again we use waitUntil() to ensure we don't move on until the old caches are deleted.
 */
self.addEventListener("activate", async (event) => {
    console.debug("[pwa-sw] activating...");
});
/**
 * Listen for browser fetch events.
 * These fire any time the browser tries to load anything.
 * This isn't just fetch() calls; clicking a <a href> triggers it too.
 */
self.addEventListener('fetch', (event) => {
    const request = event.request;
  
    // Ignore non-GET requests
    if (request.method !== 'GET') {
      return;
    }
  
    // Handle navigation requests (e.g., SPA routes)
    if (request.mode === 'navigate') {
      event.respondWith(
        caches.match('/index.html').then((response) => {
          return response || fetch('/index.html');
        })
      );
      return;
    }
  
    // Handle other requests (e.g., static assets)
    event.respondWith(
      caches.match(request).then((response) => {
        return response || fetch(request);
      })
    );
  });
  