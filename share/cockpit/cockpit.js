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
    background: read('--mj-bg-sunken', body.backgroundColor),
    surface: read('--mj-bg-raised', body.backgroundColor),
    border: read('--mj-border', body.color),
    text: read('--mj-text', body.color),
    muted: read('--mj-text-muted', body.color),
    accent: read('--color-accent-500', body.color),
    dark: document.documentElement.classList.contains('dark'),
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

const THEME_KEY = 'mj-theme';

function storedTheme() {
  try {
    return localStorage.getItem(THEME_KEY);
  } catch (e) {
    return null;
  }
}

function storeTheme(value) {
  try {
    localStorage.setItem(THEME_KEY, value);
  } catch (e) {
    /* a browser that refuses storage still gets the toggle, for this page */
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

    init() {
      window.addEventListener('keydown', (event) => {
        const combo = (event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'k';
        if (combo) {
          event.preventDefault();
          this.openPalette();
        }
      });
    },

    toggleTheme() {
      const dark = document.documentElement.classList.toggle('dark');
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
      document.documentElement.classList.toggle('dark', event.matches);
    });
}

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
