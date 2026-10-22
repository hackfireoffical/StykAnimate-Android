// Android back-button handling for the Capacitor shell.
// Back steps out one level at a time: popover/dialog -> a session's Done -> a half-drawn
// polygon/chain -> Draw/Build back to Animate -> the editor back to Home -> (Home) press twice to exit.
(function () {
  var cap = window.Capacitor;
  if (!cap || !cap.isNativePlatform || !cap.isNativePlatform()) return;

  function plugin(name) {
    try {
      return (cap.Plugins && cap.Plugins[name]) || (cap.registerPlugin && cap.registerPlugin(name)) || null;
    } catch (e) { return null; }
  }

  function toast(msg) {
    if (window.__nativeToast) return window.__nativeToast(msg);
    var el = document.createElement('div');
    el.textContent = msg;
    el.style.cssText =
      'position:fixed;left:50%;bottom:96px;transform:translateX(-50%);' +
      'background:#222;color:#fff;padding:10px 16px;border-radius:20px;' +
      'font:14px sans-serif;z-index:2147483647;pointer-events:none';
    document.body.appendChild(el);
    setTimeout(function () { el.remove(); }, 1800);
  }

  var App = plugin('App');
  if (!App || !App.addListener) {
    toast('Back button handler unavailable');
    return;
  }

  function shown(el) { return !!el && !el.hidden && el.offsetParent !== null; }

  function press(selector) {
    var el = document.querySelector(selector);
    if (shown(el)) { el.click(); return true; }
    return false;
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

  function stepBack() {
    if (closeTopLayer()) return true;
    // A half-finished polygon or chain.
    if (press('#polygon-controls [data-action="cancel-polygon"]')) return true;
    if (press('#chain-controls #chain-cancel')) return true;
    // A session (Build, movieclip, camera): Done keeps the work.
    if (press('.shell-top .band-done')) return true;
    var done = document.querySelectorAll('.shell-top .band-done');
    for (var i = 0; i < done.length; i++) {
      if (shown(done[i])) { done[i].click(); return true; }
    }
    // Draw / Build back to Animate.
    var ws = document.querySelector('.workspace[data-mode]');
    if (ws && ws.getAttribute('data-mode') !== 'animate') {
      var animate = document.querySelector('.workspace-switch [data-workspace-mode="animate"]');
      if (animate) { animate.click(); return true; }
    }
    // The editor back to Home.
    var home = document.getElementById('home');
    var homeButton = document.querySelector('[data-action="home"]');
    if (home && home.hidden && homeButton) { homeButton.click(); return true; }
    return false;
  }

  var lastBack = 0;
  App.addListener('backButton', function () {
    if (stepBack()) return;
    var now = Date.now();
    if (now - lastBack < 2000) {
      App.exitApp();
    } else {
      lastBack = now;
      toast('Press back again to exit');
    }
  });
})();
