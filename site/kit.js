/* The kit's behaviour: the three things a component of site/templates/kit/ does that markup
   cannot — copy a command, switch tabs, mark the section being read — wired by the data
   attributes the components emit. Copied to static/js/kit.js by scripts/site-build and
   loaded by the base layout.

   Every behaviour is an enhancement of something already readable. A copy button is
   rendered `hidden` and revealed here; a tab strip is rendered `hidden` with every panel
   shown, and this hides the panels and shows the strip; a table of contents is a list of
   links before it is a position indicator. With no script, nothing is missing that the
   reader needed. State is written to attributes (`aria-selected`, `aria-current`,
   `data-copied`), which is what share/design/kit.css styles. */
(function () {
  'use strict';

  var COPIED_FOR_MS = 1600;

  function copyButtons(root) {
    root.querySelectorAll('button[data-copy]').forEach(function (button) {
      var target = document.getElementById(button.getAttribute('data-copy'));
      if (!target || !navigator.clipboard) {
        return;
      }
      var label = button.getAttribute('aria-label') || 'Copy';
      button.hidden = false;
      button.addEventListener('click', function () {
        navigator.clipboard.writeText(target.innerText.trim()).then(function () {
          button.setAttribute('data-copied', 'true');
          button.setAttribute('aria-label', 'Copied');
          setTimeout(function () {
            button.removeAttribute('data-copied');
            button.setAttribute('aria-label', label);
          }, COPIED_FOR_MS);
        });
      });
    });
  }

  /* The ARIA tabs pattern with automatic activation: arrows move and select, Home and End
     go to the ends, and only the selected tab is in the tab order. */
  function tabStrips(root) {
    root.querySelectorAll('[data-tabs]').forEach(function (container) {
      var list = container.querySelector('[role="tablist"]');
      if (!list) {
        return;
      }
      var tabs = Array.prototype.slice.call(list.querySelectorAll('[role="tab"]'));
      var panels = tabs.map(function (tab) {
        return document.getElementById(tab.getAttribute('aria-controls'));
      });
      if (panels.some(function (p) { return !p; })) {
        return;
      }
      function select(index, focus) {
        tabs.forEach(function (tab, i) {
          var on = i === index;
          tab.setAttribute('aria-selected', on ? 'true' : 'false');
          tab.tabIndex = on ? 0 : -1;
          panels[i].hidden = !on;
        });
        if (focus) {
          tabs[index].focus();
        }
      }
      tabs.forEach(function (tab, i) {
        tab.addEventListener('click', function () { select(i, false); });
        tab.addEventListener('keydown', function (event) {
          var next = null;
          if (event.key === 'ArrowRight') { next = (i + 1) % tabs.length; }
          if (event.key === 'ArrowLeft') { next = (i - 1 + tabs.length) % tabs.length; }
          if (event.key === 'Home') { next = 0; }
          if (event.key === 'End') { next = tabs.length - 1; }
          if (next !== null) {
            event.preventDefault();
            select(next, true);
          }
        });
      });
      var initial = parseInt(container.getAttribute('data-selected') || '0', 10);
      container.setAttribute('data-tabs-live', '');
      list.hidden = false;
      select(initial >= 0 && initial < tabs.length ? initial : 0, false);
    });
  }

  /* The section being read is the last heading above the upper third of the viewport. */
  function tablesOfContents(root) {
    root.querySelectorAll('[data-toc]').forEach(function (nav) {
      var links = Array.prototype.slice.call(nav.querySelectorAll('a[href^="#"]'));
      var targets = links.map(function (a) {
        return document.getElementById(decodeURIComponent(a.getAttribute('href').slice(1)));
      });
      if (!links.length || targets.some(function (t) { return !t; })) {
        return;
      }
      var ticking = false;
      function mark() {
        ticking = false;
        var line = window.innerHeight / 3;
        var current = 0;
        targets.forEach(function (t, i) {
          if (t.getBoundingClientRect().top <= line) {
            current = i;
          }
        });
        links.forEach(function (a, i) {
          if (i === current) {
            a.setAttribute('aria-current', 'true');
          } else {
            a.removeAttribute('aria-current');
          }
        });
      }
      window.addEventListener('scroll', function () {
        if (!ticking) {
          ticking = true;
          window.requestAnimationFrame(mark);
        }
      }, { passive: true });
      mark();
    });
  }

  function init() {
    copyButtons(document);
    tabStrips(document);
    tablesOfContents(document);
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init, { once: true });
  } else {
    init();
  }
})();
