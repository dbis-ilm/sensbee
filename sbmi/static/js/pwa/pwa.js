// Register service worker if browser supports this feature
if ('serviceWorker' in navigator) {
  navigator.serviceWorker
    .register('/js/pwa/service-worker.js')
    .then(() => {
      console.debug('[pwa] service worker registered');
    })
    .catch(err => {
      console.error('[pwa] Service worker registration failed: ' + err);
    });
}