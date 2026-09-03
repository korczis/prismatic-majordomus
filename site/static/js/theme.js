/* Theme preference. Runs before first paint, so there is no flash of the wrong theme.
   Flowbite's dark mode is class-based (.dark on <html>); this is the documented switcher
   and the only place the preference is written. */
(function () {
  'use strict';
  var KEY = 'majordomus-theme';
  function stored() {
    try { return window.localStorage.getItem(KEY); } catch (e) { return null; }
  }
  function apply(theme) {
    document.documentElement.classList.toggle('dark', theme === 'dark');
  }
  function preferred() {
    var s = stored();
    if (s === 'dark' || s === 'light') return s;
    return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
  }
  apply(preferred());
  document.addEventListener('DOMContentLoaded', function () {
    var buttons = document.querySelectorAll('[data-theme-toggle]');
    Array.prototype.forEach.call(buttons, function (b) {
      b.addEventListener('click', function () {
        var next = document.documentElement.classList.contains('dark') ? 'light' : 'dark';
        apply(next);
        try { window.localStorage.setItem(KEY, next); } catch (e) { /* private mode */ }
        document.dispatchEvent(new CustomEvent('majordomus:theme', { detail: next }));
      });
    });
  });
})();
