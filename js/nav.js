/* The navbar's two behaviours, wired by the data attributes partials/navbar.html carries:
   `data-collapse-toggle="<id>"` opens and closes the mobile menu it names, and
   `data-dropdown-toggle="<id>"` opens and closes the menu it names beneath its toggle. Each
   sets `aria-expanded` and toggles the target's `hidden` class, so the page's CSS and the
   probes read the same state. One dropdown is open at a time; a click elsewhere or Escape
   closes it.

   This replaces Flowbite's JavaScript bundle (134 KB) for the only two things the site used it
   for. Flowbite's styles stay: they are CSS, compiled into app.css. Copied to static/js/nav.js
   by scripts/site-build and loaded, deferred, by the base layout. */
(function () {
  'use strict';

  function toggle(button, target, open) {
    var show = open === undefined ? target.classList.contains('hidden') : open;
    target.classList.toggle('hidden', !show);
    button.setAttribute('aria-expanded', show ? 'true' : 'false');
    return show;
  }

  function init() {
    document.querySelectorAll('[data-collapse-toggle]').forEach(function (button) {
      var target = document.getElementById(button.getAttribute('data-collapse-toggle'));
      if (!target) { return; }
      button.setAttribute('aria-expanded', target.classList.contains('hidden') ? 'false' : 'true');
      button.addEventListener('click', function () { toggle(button, target); });
    });

    var dropdowns = [];
    document.querySelectorAll('[data-dropdown-toggle]').forEach(function (button) {
      var menu = document.getElementById(button.getAttribute('data-dropdown-toggle'));
      if (!menu) { return; }
      dropdowns.push([button, menu]);
      button.setAttribute('aria-expanded', 'false');
      button.addEventListener('click', function (event) {
        event.stopPropagation();
        var opening = menu.classList.contains('hidden');
        dropdowns.forEach(function (d) { if (d[1] !== menu) { toggle(d[0], d[1], false); } });
        toggle(button, menu, opening);
      });
    });
    function closeAll() { dropdowns.forEach(function (d) { toggle(d[0], d[1], false); }); }
    document.addEventListener('click', function (event) {
      dropdowns.forEach(function (d) {
        if (!d[1].classList.contains('hidden') && !d[1].contains(event.target)) { toggle(d[0], d[1], false); }
      });
    });
    document.addEventListener('keydown', function (event) {
      if (event.key === 'Escape') { closeAll(); }
    });
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init, { once: true });
  } else {
    init();
  }
})();
