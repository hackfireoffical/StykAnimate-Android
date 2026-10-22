// Makes the Capacitor shell behave like an installed app instead of a web page.
//  1. No service worker / browser caches: the APK already bundles every file.
//  2. Holds the native splash screen until the editor has booted.
//  3. Saves and exports: a WebView ignores <a download> on blob: URLs, so Save and Export
//     silently did nothing. Those downloads are caught here, written to the app cache in
//     chunks, and handed to the Android share sheet (Save to Files / Drive / send).
// Styling lives in native-shell.css (the page's CSP does not allow every injected style).
(function () {
  var cap = window.Capacitor;
  if (!cap || !cap.isNativePlatform || !cap.isNativePlatform()) return;
  var noop = function () {};

  function plugin(name) {
    try {
      return (cap.Plugins && cap.Plugins[name]) || (cap.registerPlugin && cap.registerPlugin(name)) || null;
    } catch (e) { return null; }
  }

  function toast(msg) {
    var el = document.createElement('div');
    el.textContent = msg;
    el.style.cssText =
      'position:fixed;left:50%;bottom:96px;transform:translateX(-50%);max-width:88vw;' +
      'background:#222;color:#fff;padding:10px 16px;border-radius:20px;text-align:center;' +
      'font:14px sans-serif;z-index:2147483647;pointer-events:none';
    document.body.appendChild(el);
    setTimeout(function () { el.remove(); }, 3500);
  }
  window.__nativeToast = toast;

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

  // 2. Splash screen handover.
  var Splash = plugin('SplashScreen');
  if (Splash) {
    var hidden = false;
    var hide = function () {
      if (hidden) return;
      hidden = true;
      try { Splash.hide({ fadeOutDuration: 200 }); } catch (e) { /* ignore */ }
    };
    var started = Date.now();
    var timer = setInterval(function () {
      var booted = window.styk && window.styk.ui && window.styk.ui.toasts;
      if (booted || Date.now() - started > 15000) {
        clearInterval(timer);
        requestAnimationFrame(function () { requestAnimationFrame(hide); });
      }
    }, 100);
  }

  // 3. Downloads -> cache file -> share sheet.
  var Fs = plugin('Filesystem');
  var Share = plugin('Share');
  if (Fs && Fs.rmdir) Fs.rmdir({ path: 'exports', directory: 'CACHE', recursive: true }).catch(noop);

  // The page's CSP blocks fetch() on blob: URLs, so remember each Blob when its URL is made.
  var blobs = new Map();
  var origCreate = URL.createObjectURL.bind(URL);
  URL.createObjectURL = function (obj) {
    var url = origCreate(obj);
    if (obj instanceof Blob) blobs.set(url, obj);
    return url;
  };
  var origRevoke = URL.revokeObjectURL.bind(URL);
  URL.revokeObjectURL = function (url) {
    setTimeout(function () { blobs.delete(url); }, 60000);
    return origRevoke(url);
  };

  function toBase64(blob) {
    return new Promise(function (resolve, reject) {
      var reader = new FileReader();
      reader.onload = function () { resolve(String(reader.result).split(',')[1] || ''); };
      reader.onerror = function () { reject(reader.error || new Error('Could not read file')); };
      reader.readAsDataURL(blob);
    });
  }

  var saving = false;
  async function saveBlob(blob, name) {
    if (!Fs || !Share) { toast('Saving is not available in this build'); return; }
    if (saving) { toast('Another file is still being prepared'); return; }
    saving = true;
    var safe = String(name || 'download').replace(/[\\/:*?"<>|\u0000-\u001f]+/g, '_') || 'download';
    var path = 'exports/' + Date.now() + '/' + safe;
    try {
      toast('Preparing ' + safe + '\u2026');
      var CHUNK = 1572864; // divisible by 3, so each chunk's base64 has no padding
      var uri = null;
      for (var off = 0; off === 0 || off < blob.size; off += CHUNK) {
        var b64 = await toBase64(blob.slice(off, off + CHUNK));
        if (off === 0) {
          var res = await Fs.writeFile({ path: path, data: b64, directory: 'CACHE', recursive: true });
          uri = res.uri;
        } else {
          await Fs.appendFile({ path: path, data: b64, directory: 'CACHE' });
        }
      }
      await Share.share({ title: safe, url: uri, dialogTitle: 'Save ' + safe });
    } catch (e) {
      var msg = String((e && e.message) || e);
      if (!/cancel/i.test(msg)) toast('Could not save ' + safe + ': ' + msg);
    } finally {
      saving = false;
    }
  }

  function intercept(anchor) {
    if (!anchor || !anchor.hasAttribute || !anchor.hasAttribute('download')) return false;
    var blob = blobs.get(anchor.href);
    if (!blob) return false;
    saveBlob(blob, anchor.getAttribute('download'));
    return true;
  }

  var origClick = HTMLAnchorElement.prototype.click;
  HTMLAnchorElement.prototype.click = function () {
    if (intercept(this)) return;
    return origClick.apply(this, arguments);
  };
  HTMLAnchorElement.prototype.dispatchEvent = function (event) {
    if (event && event.type === 'click' && intercept(this)) return true;
    return EventTarget.prototype.dispatchEvent.apply(this, arguments);
  };
  document.addEventListener('click', function (event) {
    var a = event.target && event.target.closest && event.target.closest('a[download]');
    if (a && intercept(a)) { event.preventDefault(); event.stopPropagation(); }
  }, true);
})();
