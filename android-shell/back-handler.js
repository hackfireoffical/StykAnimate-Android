// Android back-button handling for the Capacitor shell.
// Closes the top popover/dialog first; otherwise asks for a second press to exit.
(function () {
  var cap = window.Capacitor;
  if (!cap || !cap.isNativePlatform || !cap.isNativePlatform()) return;
  var App = cap.Plugins && cap.Plugins.App;
  if (!App) return;

  var lastBack = 0;

  function toast(msg) {
    var el = document.createElement('div');
    el.textContent = msg;
    el.style.cssText =
      'position:fixed;left:50%;bottom:88px;transform:translateX(-50%);' +
      'background:#222;color:#fff;padding:10px 16px;border-radius:20px;' +
      'font:14px sans-serif;z-index:2147483647;pointer-events:none';
    document.body.appendChild(el);
    setTimeout(function () { el.remove(); }, 1800);
  }

  function closeTopLayer() {
    try {
      var pop = document.querySelector(':popover-open');
      if (pop && pop.hidePopover) { pop.hidePopover(); return true; }
    } catch (e) { /* older WebView: no :popover-open */ }

    var dialogs = document.querySelectorAll('dialog[open]');
    if (dialogs.length) {
      var d = dialogs[dialogs.length - 1];
      var ev = new Event('cancel', { cancelable: true });
      d.dispatchEvent(ev);
      if (!ev.defaultPrevented && d.open) d.close();
      return true;
    }
    return false;
  }

  App.addListener('backButton', function () {
    if (closeTopLayer()) return;
    var now = Date.now();
    if (now - lastBack < 2000) {
      App.exitApp();
    } else {
      lastBack = now;
      toast('Press back again to exit');
    }
  });
})();
