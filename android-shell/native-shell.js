// Makes the Capacitor shell behave like an installed app instead of a web page.
//  1. No service worker / browser caches: the APK already bundles every file, and a
//     cached copy can shadow a newer APK.
//  2. Holds the native splash screen until the editor has booted, so the user never
//     watches a page load. It hides itself after a safety timeout either way.
(function () {
  var cap = window.Capacitor;
  if (!cap || !cap.isNativePlatform || !cap.isNativePlatform()) return;
  var noop = function () {};

  // 1. Browser-only machinery off inside the app.
  try {
    var sw = navigator.serviceWorker;
    if (sw) {
      sw.register = function () {
        return Promise.reject(new Error('Service worker is disabled in the Android app'));
      };
      sw.getRegistrations()
        .then(function (list) { list.forEach(function (r) { r.unregister(); }); })
        .catch(noop);
    }
    if (window.caches) {
      caches.keys()
        .then(function (keys) { keys.forEach(function (k) { caches.delete(k); }); })
        .catch(noop);
    }
  } catch (e) { /* ignore */ }

  // Native-feeling page behaviour.
  var style = document.createElement('style');
  style.textContent =
    'html,body{overscroll-behavior:none;-webkit-tap-highlight-color:transparent}';
  document.head.appendChild(style);

  // 2. Splash screen handover.
  var Splash = cap.Plugins && cap.Plugins.SplashScreen;
  if (!Splash) return;
  var hidden = false;
  function hide() {
    if (hidden) return;
    hidden = true;
    try { Splash.hide({ fadeOutDuration: 200 }); } catch (e) { /* ignore */ }
  }
  var started = Date.now();
  var timer = setInterval(function () {
    var booted = window.styk && window.styk.ui && window.styk.ui.toasts;
    if (booted || Date.now() - started > 15000) {
      clearInterval(timer);
      // Two frames so the first editor paint is on screen before the splash fades.
      requestAnimationFrame(function () { requestAnimationFrame(hide); });
    }
  }, 100);
})();
