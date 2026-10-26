/* The drawer's behaviour, on /ui and /cockpit (I3214): the shell is
   apps/majordomus-cli/src/ui/drawer.rs and its look share/design/drawer.css, where the drawer
   opens and shows with no script at all. This adds what a script must: the drawer closes on
   Escape, on its backdrop and on a choice of entry, and the focus goes back to the toggle that
   opened it, so a keyboard reader is never left on a panel that has gone.

   Two toggles can open it, never both on screen at once: the drawer's own checkbox (its button
   shows from md to lg, and below md with scripts off), and below md the site bar's menu
   (`navbar-main`), which the site's nav.js opens and closes and whose Escape it already
   handles. Closing the bar's menu here goes through its own button, so nav.js keeps
   `aria-expanded` true to what is shown.

   Copied byte for byte to share/ui/drawer.js and share/cockpit/drawer.js by
   scripts/cockpit-assets, and loaded, deferred, with the bar's scripts. */
(function () {
  'use strict';

  function init() {
    var box = document.getElementById('mj-drawer');
    if (!box) { return; }
    var barToggle = document.querySelector('nav[aria-label="Main"] [data-collapse-toggle="navbar-main"]');
    var barMenu = document.getElementById('navbar-main');
    // the bar's menu counts only where its toggle is on screen (below md)
    function barOpen() {
      return !!barToggle && !!barMenu && barToggle.offsetParent !== null
        && !barMenu.classList.contains('hidden');
    }
    function close(focus) {
      if (box.checked) {
        box.checked = false;
        if (focus) { box.focus(); }
      }
      if (barOpen()) {
        barToggle.click();
        if (focus) { barToggle.focus(); }
      }
    }
    // the bar's menu has nav.js for Escape; the checkbox has this
    document.addEventListener('keydown', function (event) {
      if (event.key === 'Escape' && box.checked) { close(true); }
    });
    document.querySelectorAll('.mj-drawer-scrim').forEach(function (scrim) {
      scrim.addEventListener('click', function (event) {
        event.preventDefault();
        close(true);
      });
    });
    document.querySelectorAll('.mj-drawer').forEach(function (panel) {
      panel.addEventListener('click', function (event) {
        if (event.target.closest && event.target.closest('a[href]')) { close(false); }
      });
    });
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init, { once: true });
  } else {
    init();
  }
})();
