// interactions.mjs: what counts as an interactive control on a built page, and which behaviour spec
// answers for it. Rule: project.every-link-and-control-is-tested.
//
// Discovery reads the HTML the build wrote, never a list: a control added to a template tomorrow is
// discovered tomorrow. An element is a control when a reader can act on it:
//   - a form control or a button: <button>, <input> (not hidden), <select>, <textarea>, <summary>
//   - anything an Alpine directive makes act: x-on:*, @*, x-model
//   - anything Flowbite binds: data-{collapse,dropdown,modal,drawer,tabs,accordion,tooltip,popover}-{toggle,target}
//   - an ARIA widget role: tab, switch, checkbox, radio, menuitem, slider, option
//   - the graph's bindings: data-graph-node, data-graph-filter, data-graph-reset
//   - a keyboard-reachable scroll region: tabindex="0" on an element that is not otherwise one
// A plain <a href> is not in this set: scripts/ci/link-check decides every link.
//
// Each spec under scripts/lib/interaction-specs/ exports { id, title, claims(el), exercise(ctx) }.
// claims() is a pure function of one element's tag and attributes, so the coverage check needs no
// browser; exercise() drives every claimed element of a page in one. An element no spec claims fails
// the coverage check; so does an element two specs claim, and a spec that claims nothing on the site.
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join, relative, sep } from 'node:path';
import { pathToFileURL } from 'node:url';


// Attributes of one start tag. Zola's output is well formed, so a quoted-value scan is exact here.
export function attributes(raw) {
  const out = {};
  const rx = /([^\s"'>\/=]+)(?:\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s"'>]+)))?/g;
  let m;
  while ((m = rx.exec(raw))) out[m[1]] = m[2] ?? m[3] ?? m[4] ?? '';
  return out;
}

// Self-contained on purpose: the probe hands this function's source to the browser, so the page's own
// DOM is classified by the same rule the static check applies to the HTML. It must not close over
// anything defined outside it.
export function isControl(tag, a) {
  const FLOWBITE = /^data-(collapse|dropdown|modal|drawer|tabs|accordion|tooltip|popover)-(toggle|target)$/;
  const ROLES = ['tab', 'switch', 'checkbox', 'radio', 'menuitem', 'slider', 'option'];
  if ('disabled' in a) return false;          // an element nobody can operate is not a control
  if (tag === 'button' || tag === 'select' || tag === 'textarea' || tag === 'summary') return true;
  if (tag === 'input') return (a.type || 'text').toLowerCase() !== 'hidden';
  for (const k of Object.keys(a)) {
    if (k.startsWith('x-on:') || k.startsWith('@') || k === 'x-model' || FLOWBITE.test(k)) return true;
    if (k === 'data-graph-node' || k === 'data-graph-filter' || k === 'data-graph-reset') return true;
  }
  if (a.role && ROLES.includes(a.role)) return true;
  if (a.tabindex === '0' && tag !== 'a') return true;
  return false;
}

// Every control of one page, in document order, each with an ordinal that locates it again in a browser.
export function discover(html) {
  const body = html.replace(/<script\b[\s\S]*?<\/script>/gi, (s) => ' '.repeat(s.length))
                   .replace(/<!--[\s\S]*?-->/g, (s) => ' '.repeat(s.length));
  const found = [];
  // <template> content is not in the document until a script renders it (Alpine's x-if), so a control
  // inside one is conditional: counted, and exercised by whichever spec makes its condition true
  // quote-aware: an attribute value may hold ">" (Alpine's `() => {...}` does), which a [^>]* scan would end the tag on
  const rx = /<(\/?)([a-zA-Z][a-zA-Z0-9-]*)((?:\s+[^\s"'>\/=]+(?:\s*=\s*(?:"[^"]*"|'[^']*'|[^\s"'>]+))?)*)\s*\/?>/g;
  let m, templates = 0;
  while ((m = rx.exec(body))) {
    const tag = m[2].toLowerCase();
    if (tag === 'template') { templates += m[1] ? -1 : 1; continue; }
    if (m[1]) continue;
    const a = attributes(m[3] || '');
    if (isControl(tag, a)) found.push({ tag, attrs: a, offset: m.index, conditional: templates > 0 });
  }
  return found;
}

// A short, stable description of an element for findings and the inventory: its tag and the
// attributes that make it a control, with values that vary per instance elided.
export function signature(el) {
  const keep = Object.entries(el.attrs)
    .filter(([k]) => /^(type|role|tabindex|x-model|x-on:|@|data-(collapse|dropdown|modal|drawer|tabs|accordion|tooltip|popover|graph))/.test(k))
    .map(([k, v]) => {
      if (/^data-graph-node$|^data-(collapse|dropdown)-toggle$/.test(k)) return `${k}=…`;
      const short = v.replace(/'[^']*'/g, "'…'").replace(/\d+/g, 'N').slice(0, 48);
      return v === '' ? k : `${k}="${short}"`;
    });
  return `<${el.tag}${keep.length ? ' ' + keep.join(' ') : ''}>`;
}

export async function loadSpecs(dir) {
  const specs = [];
  for (const f of readdirSync(dir).filter((n) => n.endsWith('.mjs')).sort()) {
    const mod = await import(pathToFileURL(join(dir, f)).href);
    const spec = mod.default;
    if (!spec || typeof spec.claims !== 'function' || typeof spec.exercise !== 'function' || !spec.id) {
      throw new Error(`${f}: a spec exports default { id, title, claims(el), exercise(ctx) }`);
    }
    specs.push({ ...spec, file: f });
  }
  return specs;
}

// Every built page: route -> path of its index.html. Redirect stubs carry no controls of their own.
export function pages(pub) {
  const out = [];
  const walk = (d) => {
    for (const n of readdirSync(d)) {
      const p = join(d, n);
      if (statSync(p).isDirectory()) walk(p);
      else if (n === 'index.html') {
        const rel = relative(pub, d).split(sep).join('/');
        out.push({ route: rel ? `/${rel}/` : '/', file: p });
      }
    }
  };
  walk(pub);
  return out.sort((x, y) => (x.route < y.route ? -1 : x.route > y.route ? 1 : 0));
}

// The claim of every control on every page: { route, el, specs: [ids] }.
export function classify(pub, specs) {
  const rows = [];
  for (const { route, file } of pages(pub)) {
    const html = readFileSync(file, 'utf8');
    if (/http-equiv="refresh"/.test(html)) continue;
    for (const el of discover(html)) {
      rows.push({ route, el, specs: specs.filter((s) => s.claims(el)).map((s) => s.id) });
    }
  }
  return rows;
}

// In the browser: mark every control the live document holds with data-mj-control="<n>" and return what
// each one is, so node can apply the same claims() to the page a reader actually gets.
export function markControls(isControlSource) {
  // eslint-disable-next-line no-new-func
  const isControl = new Function(`return (${isControlSource})`)();
  const out = [];
  let n = 0;
  for (const el of document.querySelectorAll('*')) {
    const a = {};
    for (const attr of el.attributes) a[attr.name] = attr.value;
    if (!isControl(el.tagName.toLowerCase(), a)) continue;
    el.setAttribute('data-mj-control', String(n));
    out.push({ n, tag: el.tagName.toLowerCase(), attrs: a });
    n++;
  }
  return out;
}

// Every Alpine expression a page carries compiles as JavaScript. A template that interpolates text into a JavaScript
// string breaks on the first apostrophe in that text, and Alpine then drops the whole directive: on /doctrines/ and
// /use-cases/ that silently turned a filter off. Checked from the markup, so the class is refused where the site is
// built rather than found by a browser at the end of a probe.
// Directives whose value is not JavaScript are skipped: x-for (`item in items`), and x-ref, x-cloak, x-transition,
// x-teleport, x-id, x-ignore, which take a name or nothing.
const NOT_JS = /^x-(for|ref|cloak|transition(:[a-z-]+)?(\.[a-z0-9-]+)*|teleport|id|ignore)$/;
function unescapeAttr(v) {
  return v.replace(/&quot;/g, '"').replace(/&#x27;|&#39;/g, "'").replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&amp;/g, '&');
}
export function brokenExpressions(html) {
  const body = html.replace(/<script\b[\s\S]*?<\/script>/gi, '').replace(/<!--[\s\S]*?-->/g, '');
  const out = [];
  const tagRx = /<([a-zA-Z][a-zA-Z0-9-]*)((?:\s+[^\s"'>\/=]+(?:\s*=\s*(?:"[^"]*"|'[^']*'|[^\s"'>]+))?)*)\s*\/?>/g;
  let m;
  while ((m = tagRx.exec(body))) {
    const attrs = attributes(m[2] || '');
    for (const [name, raw] of Object.entries(attrs)) {
      const alpine = name.startsWith('x-') || name.startsWith('@') || (name.startsWith(':') && name.length > 1);
      if (!alpine || NOT_JS.test(name) || raw === '') continue;
      const expr = unescapeAttr(raw);
      let ok = false;
      for (const body of [`return (${expr});`, `${expr};`]) {
        try { new Function(body); ok = true; break; } catch { /* the other form */ }
      }
      if (!ok) {
        let reason = 'does not compile';
        try { new Function(`${expr};`); } catch (e) { reason = e.message; }
        out.push({ tag: m[1].toLowerCase(), name, expr, reason });
      }
    }
  }
  return out;
}
