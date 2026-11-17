// Theme toggle — the structure is Flowbite's (flowbite.com/docs/customize/dark-mode/); the
// contract is the design declaration's. The storage key and the class that means dark are
// written onto <html> by base.html from site/data/registry/design.json, generated from
// share/design/tokens.yaml, and read here; this file names neither. The pre-paint statement
// is the same declaration's, inlined by base.html; this only wires the button and icons.
(function () {
  var root = document.documentElement;
  var KEY = root.dataset.themeKey;
  var CLS = root.dataset.themeClass;
  var themeToggleDarkIcon = document.getElementById('theme-toggle-dark-icon');
  var themeToggleLightIcon = document.getElementById('theme-toggle-light-icon');
  var themeToggleBtn = document.getElementById('theme-toggle');
  if (!KEY || !CLS || !themeToggleBtn || !themeToggleDarkIcon || !themeToggleLightIcon) { return; }
  function stored() { try { return localStorage.getItem(KEY); } catch (e) { return null; } }
  function store(v) { try { localStorage.setItem(KEY, v); } catch (e) { /* the toggle still works for this page */ } }
  if (root.classList.contains(CLS)) {
    themeToggleLightIcon.classList.remove('hidden');
  } else {
    themeToggleDarkIcon.classList.remove('hidden');
  }
  themeToggleBtn.addEventListener('click', function () {
    themeToggleDarkIcon.classList.toggle('hidden');
    themeToggleLightIcon.classList.toggle('hidden');
    var dark = root.classList.toggle(CLS);
    store(dark ? 'dark' : 'light');
  });
  if (!stored()) {
    window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', function (event) {
      root.classList.toggle(CLS, event.matches);
      themeToggleDarkIcon.classList.toggle('hidden', event.matches);
      themeToggleLightIcon.classList.toggle('hidden', !event.matches);
    });
  }
})();
