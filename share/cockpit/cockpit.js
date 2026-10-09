// The Cockpit's shell: the one Alpine component, and the shared helpers the page modules
// use. Loaded on every page; nothing here fetches anything until a person asks it to.
//
// Alpine is optional. If share/cockpit/vendor/alpine.csp.min.js is not there, the import
// fails, this module's registration never happens, and every page is exactly what the
// server rendered: complete HTML with working links and working forms. The palette and
// the theme toggle are the only things lost, and the theme still follows the system.
//
// Alpine's CSP build is what is loaded, so no expression in any attribute is ever
// evaluated as a string. Every `x-` attribute in the Rust names a property or a method
// defined here, which is why the page's content-security policy needs no `unsafe-eval`.

/** Where the Cockpit's own assets live. */
export const ASSETS = '/cockpit/assets/';

/** Ask this server for JSON. Same origin, no credentials, typed failure. */
export async function api(path, options) {
  const response = await fetch(path, {
    credentials: 'omit',
    ...options,
    headers: { Accept: 'application/json', ...(options && options.headers) },
  });
  const text = await response.text();
  let body = null;
  try {
    body = text ? JSON.parse(text) : null;
  } catch (e) {
    body = { error: { code: 'unreadable', message: text.slice(0, 400) } };
  }
  return { ok: response.ok, status: response.status, body };
}

/**
 * Load a vendored library that may not be there, once. Resolves with the global the
 * script defines, or rejects; a caller that rejects must leave its page working.
 */
const loading = new Map();
export function vendor(file, globalName) {
  if (globalName && window[globalName]) return Promise.resolve(window[globalName]);
  if (loading.has(file)) return loading.get(file);
  const pending = new Promise((resolve, reject) => {
    const script = document.createElement('script');
    script.src = ASSETS + 'vendor/' + file;
    script.addEventListener('load', () => {
      const value = globalName ? window[globalName] : true;
      if (value) resolve(value);
      else reject(new Error(file + ' loaded and defined no ' + globalName));
    });
    script.addEventListener('error', () =>
      reject(new Error(file + ' is not in share/cockpit/vendor/; run `just cockpit-assets`')),
    );
    document.head.appendChild(script);
  });
  loading.set(file, pending);
  return pending;
}

/**
 * Load a vendored ES module that may not be there. Same contract as `vendor`: the caller
 * must leave its page working when this rejects.
 */
export function vendorModule(file) {
  if (loading.has(file)) return loading.get(file);
  const pending = import(ASSETS + 'vendor/' + file).catch(() => {
    throw new Error(file + ' is not in share/cockpit/vendor/; run `just cockpit-assets`');
  });
  loading.set(file, pending);
  return pending;
}

/** Say, in the element, why an optional visual layer is not there. */
export function explainMissing(element, error) {
  element.textContent = '';
  const p = document.createElement('p');
  p.className = 'mj-note';
  p.style.padding = '1rem';
  p.textContent =
    'The visual layer is not available (' +
    error.message +
    '). Everything it would show is listed on this page as text.';
  element.appendChild(p);
}

/** Read the tokens the stylesheet defines, so a drawing uses the page's own palette. */
export function palette() {
  const style = getComputedStyle(document.documentElement);
  // The fallback is the page's own computed colour, never a literal. A hexadecimal here
  // would be a sixth copy of a decision share/design/tokens.yaml owns, and a copy that
  // only ever appears when the stylesheet failed to load is a copy nobody would notice
  // going stale. If the tokens are missing the drawing is monochrome, which is honest.
  const body = getComputedStyle(document.body);
  const read = (name, fallback) => (style.getPropertyValue(name) || fallback).trim();
  return {
    background: read('--mj-sunken', body.backgroundColor),
    surface: read('--mj-raised', body.backgroundColor),
    border: read('--mj-line', body.color),
    text: read('--mj-fg', body.color),
    muted: read('--mj-muted', body.color),
    accent: read('--mj-accent', body.color),
    dark: document.documentElement.classList.contains(THEME_CLASS),
  };
}

/** Does this reader want motion? */
export function motionAllowed() {
  return !window.matchMedia('(prefers-reduced-motion: reduce)').matches;
}

/**
 * Run `frame` on every animation frame while `element` is on screen and the tab is
 * visible, and stop otherwise. Returns a function that stops it for good. This is the one
 * place a repeating draw is started, so no page can leave one running in a hidden tab.
 */
export function whileVisible(element, frame) {
  let running = false;
  let handle = 0;
  const tick = (time) => {
    if (!running) return;
    frame(time);
    handle = requestAnimationFrame(tick);
  };
  const start = () => {
    if (running) return;
    running = true;
    handle = requestAnimationFrame(tick);
  };
  const stop = () => {
    running = false;
    cancelAnimationFrame(handle);
  };
  const decide = (visible) => {
    if (visible && document.visibilityState === 'visible') start();
    else stop();
  };
  const observer = new IntersectionObserver(
    (entries) => decide(entries.some((e) => e.isIntersecting)),
    { threshold: 0 },
  );
  observer.observe(element);
  const onVisibility = () => decide(element.getBoundingClientRect().height > 0);
  document.addEventListener('visibilitychange', onVisibility);
  return () => {
    stop();
    observer.disconnect();
    document.removeEventListener('visibilitychange', onVisibility);
  };
}

// --------------------------------------------------------------------- the theme

// The theme contract is the design declaration's, not this file's: the shell writes the
// storage key and the class onto <body> from the compiled declaration, and this reads them
// there. A page without them (a distribution whose compiled declaration is invalid) gets a
// toggle that works for the page and stores nothing.
const THEME_KEY = (document.body && document.body.dataset.themeKey) || null;
const THEME_CLASS = (document.body && document.body.dataset.themeClass) || 'dark';

function storedTheme() {
  if (!THEME_KEY) return null;
  try {
    return localStorage.getItem(THEME_KEY);
  } catch (e) {
    return null;
  }
}

function storeTheme(value) {
  if (!THEME_KEY) return;
  try {
    localStorage.setItem(THEME_KEY, value);
  } catch (e) {
    /* a browser that refuses storage still gets the toggle, for this page */
  }
}

// The width at which the sidebar sits beside the page rather than over it: Tailwind's `lg`,
// the breakpoint the stylesheet's `lg:block` and `max-lg:` rules are written against.
const DESKTOP = window.matchMedia('(min-width: 64rem)');

// What the open drawer made inert, so closing it gives back exactly that and never clears
// an `inert` something else set.
let madeInert = [];

/**
 * Put the sidebar into its drawer state or take it out. While open it is a modal dialog:
 * named, the rest of the page inert (unreachable by pointer, keyboard and assistive
 * technology alike), the page behind it not scrolling, the trigger saying it is expanded.
 * Closed, every one of those is undone, which is what keeps a closed drawer from leaving
 * an invisible layer or a locked page behind.
 */
function setDrawer(nav, trigger, open) {
  // written here, synchronously, and not bound reactively: the page is in its final state
  // when the tap's handler returns, so a second tap, a test or a screen reader never sees
  // the drawer half open
  nav.toggleAttribute('data-open', open);
  document.documentElement.classList.toggle('mj-nav-locked', open);
  if (trigger) trigger.setAttribute('aria-expanded', open ? 'true' : 'false');
  if (open) {
    nav.setAttribute('role', 'dialog');
    nav.setAttribute('aria-modal', 'true');
    // every sibling on the way up to <body>, except the backdrop that closes the drawer
    for (let node = nav; node && node !== document.body; node = node.parentElement) {
      for (const sibling of node.parentElement.children) {
        if (sibling === node || sibling.inert) continue;
        if (sibling.classList.contains('mj-nav-backdrop')) continue;
        if (sibling.tagName === 'SCRIPT') continue;
        sibling.inert = true;
        madeInert.push(sibling);
      }
    }
  } else {
    nav.removeAttribute('role');
    nav.removeAttribute('aria-modal');
    for (const node of madeInert) node.inert = false;
    madeInert = [];
  }
}

/**
 * Keep Tab inside the open drawer: past the last control it returns to the first, and
 * before the first to the last. The rest of the page is inert already; this only stops
 * focus leaving for the browser's own chrome mid-list.
 */
function wrapFocus(event, container) {
  if (!container) return;
  const focusable = [...container.querySelectorAll('a[href], button, summary, [tabindex]')]
    .filter((el) => !el.closest('[inert]') && el.getClientRects().length > 0);
  if (!focusable.length) return;
  const first = focusable[0];
  const last = focusable[focusable.length - 1];
  if (event.shiftKey && document.activeElement === first) {
    event.preventDefault();
    last.focus();
  } else if (!event.shiftKey && document.activeElement === last) {
    event.preventDefault();
    first.focus();
  }
}

/**
 * The one Alpine component. Everything the markup names is a property or a method here,
 * because the CSP build evaluates nothing.
 */
export function component() {
  return {
    // flat, deliberately: Alpine's CSP build resolves a property name and treats
    // `palette.open` as an expression it will not evaluate, which leaves x-show inert and
    // the palette's backdrop covering the page. The browser probe holds this shut.
    paletteOpen: false,
    paletteQuery: '',
    // the sidebar as a drawer, below the width where it sits beside the page; this is the
    // one place its state lives, and every way of closing it goes through closeNav
    navOpen: false,

    init() {
      window.addEventListener('keydown', (event) => {
        const combo = (event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'k';
        if (combo) {
          event.preventDefault();
          this.closeNav(false);
          this.openPalette();
          return;
        }
        if (!this.navOpen) return;
        if (event.key === 'Escape') {
          event.preventDefault();
          this.closeNav(true);
        } else if (event.key === 'Tab') {
          wrapFocus(event, this.$refs.nav);
        }
      });
      // the sections reached by their fragment are opened by `:target` — through the trigger
      // before the script ran, or later by a link, a typed address or history. The component
      // takes each of them over, so there is one state and not two: a drawer the stylesheet
      // shows and the component does not know is one Escape cannot close.
      // the stylesheet's `:target` fallback is for a page this component never started on;
      // once it runs, it owns the drawer, and `:target` (which browsers do not re-evaluate
      // on history.replaceState) must not keep a closed drawer on screen
      document.documentElement.setAttribute('data-mj-nav', 'owned');
      const takeOver = () => {
        if (!this.$refs.nav || location.hash !== '#' + this.$refs.nav.id) return;
        history.replaceState(null, '', location.pathname + location.search);
        this.openNav();
      };
      takeOver();
      window.addEventListener('hashchange', takeOver);
      // widening past the drawer's width puts the sidebar beside the page, where it is not
      // modal; a drawer left open there would keep the page inert and locked
      DESKTOP.addEventListener('change', (event) => {
        if (event.matches) this.closeNav(false);
      });
      // following an entry leaves the page; the drawer closes first, so the page the back
      // button restores from the cache is not one with the drawer still over it
      if (this.$refs.nav) {
        this.$refs.nav.addEventListener('click', (event) => {
          if (event.target.closest('a.mj-nav-link')) this.closeNav(false);
        });
      }
      window.addEventListener('pageshow', (event) => {
        if (event.persisted) this.closeNav(false);
      });
    },

    openNav() {
      if (this.navOpen || DESKTOP.matches || !this.$refs.nav) return;
      this.navOpen = true;
      setDrawer(this.$refs.nav, this.$refs.navOpen, true);
      if (this.$refs.navClose) this.$refs.navClose.focus();
    },

    // `restoreFocus` is false when the page is leaving or the drawer stops being one: the
    // trigger is then hidden or about to be gone, and focus belongs elsewhere
    closeNav(restoreFocus) {
      if (!this.navOpen) return;
      this.navOpen = false;
      setDrawer(this.$refs.nav, this.$refs.navOpen, false);
      if (restoreFocus !== false && this.$refs.navOpen) this.$refs.navOpen.focus();
    },

    toggleTheme() {
      const dark = document.documentElement.classList.toggle(THEME_CLASS);
      storeTheme(dark ? 'dark' : 'light');
    },

    openPalette() {
      this.paletteOpen = true;
      this.$nextTick(() => {
        if (this.$refs.paletteInput) this.$refs.paletteInput.focus();
      });
      window.dispatchEvent(new CustomEvent('mj:palette-open'));
    },

    closePalette() {
      this.paletteOpen = false;
    },

    paletteFilter() {
      window.dispatchEvent(
        new CustomEvent('mj:palette-query', { detail: this.paletteQuery }),
      );
    },

    paletteNext() {
      window.dispatchEvent(new CustomEvent('mj:palette-move', { detail: 1 }));
    },

    palettePrevious() {
      window.dispatchEvent(new CustomEvent('mj:palette-move', { detail: -1 }));
    },

    paletteChoose() {
      window.dispatchEvent(new CustomEvent('mj:palette-choose'));
    },
  };
}

// The theme the bootstrap script already applied is not re-applied here; this only keeps
// a reader who never chose in step with their system.
if (!storedTheme()) {
  window
    .matchMedia('(prefers-color-scheme: dark)')
    .addEventListener('change', (event) => {
      document.documentElement.classList.toggle(THEME_CLASS, event.matches);
    });
}

// --------------------------------------------------------------- the design check
//
// The stylesheet this page loaded carries the fingerprint of the declaration it was
// projected from (`--mj-design`); the executable that rendered the page carries the
// fingerprint of the declaration it was built with (`data-design` on <body>). The two are
// the same file in the repository and can still differ on a machine — a stylesheet compiled
// before the declaration moved, an executable built before it was regenerated — and a
// mismatch is invisible to every check that reads files. So the page says so, once, at the
// top, and the Design page shows the verdict as a badge.
export function designCheck() {
  const built = document.body && document.body.dataset.design;
  const served = getComputedStyle(document.documentElement)
    .getPropertyValue('--mj-design')
    .trim()
    .replace(/^"|"$/g, '');
  const badge = document.getElementById('mj-design-check');
  const same = built && served && built === served;
  if (badge) {
    badge.className = 'mj-badge mj-badge--' + (same ? 'ok' : served ? 'stale' : 'missing');
    badge.textContent = same
      ? 'stylesheet and executable agree'
      : served
        ? 'stylesheet ' + served + ', executable ' + built
        : 'the stylesheet carries no design fingerprint';
  }
  if (built && served && !same) {
    const main = document.getElementById('main');
    if (main) {
      const alert = document.createElement('div');
      alert.className = 'mj-alert mj-alert--stale';
      alert.setAttribute('role', 'status');
      alert.textContent =
        'The stylesheet this page loaded was projected from design ' +
        served +
        '; this executable was built with ' +
        built +
        '. One of them is stale: run scripts/cockpit-assets after `majordomus generate design`, rebuild, and restart the server.';
      main.prepend(alert);
    }
  }
}
designCheck();

// the ES module build: it exports Alpine and starts nothing, so the component is
// registered before the first element is initialised. The CDN build starts itself on a
// microtask, which is earlier than this module runs, and every attribute then names a
// variable that does not exist yet.
vendorModule('alpine.csp.min.js')
  .then((module) => {
    const Alpine = module.default || module.Alpine;
    if (!Alpine) throw new Error('alpine.csp.min.js exported no Alpine');
    window.Alpine = Alpine;
    Alpine.data('cockpit', component);
    Alpine.start();
  })
  .catch((error) => {
    // the pages are complete without it; say so once, quietly, for whoever is looking
    console.info('cockpit: %s — pages remain fully functional without it', error.message);
  });
