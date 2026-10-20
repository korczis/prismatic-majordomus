// The site's languages (ADR 0218): every page of the built site, in every language the site
// declares, derived from the English build and a translation memory — never authored twice.
//
// English is the only source. scripts/site-build renders it, and this pass reads every page it
// wrote, cuts it into segments (a run of inline content between block boundaries, the text of
// an attribute a reader meets: title, alt, aria-label, the page's description), and looks each
// one up in site/i18n/<lang>/memory/*.jsonl by the hash of its English. The page under
// /<lang>/ is the English page with each segment the memory knows replaced, its internal links
// pointed at their counterparts and its language declared. A segment the memory does not know
// stays English and is counted: the coverage is published at /<lang>/i18n.json and the missing
// segments are listed for scripts/site-translate, which asks a model for them and writes the
// memory. Nothing here calls a model; a build is a pure function of the English pages and the
// memory, so two builds of one tree write the same bytes.
//
// Inline markup is never shown to a translator as markup. An inline element becomes a numbered
// placeholder — <t1>…</t1>, or <t1/> for a void element and for code, which is never translated
// — so the memory is keyed by the words and their structure, not by an href that differs from
// page to page, and a translation is admitted only when it carries every placeholder exactly
// once and nests them as the English did.
//
//   node scripts/lib/site-i18n.mjs build <public> <i18n-dir> <base-url> <missing-out>

import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { dirname, join, relative } from 'node:path';

const VOID = new Set(['area', 'base', 'br', 'col', 'embed', 'hr', 'img', 'input', 'link', 'meta',
  'param', 'source', 'track', 'wbr']);
const RAW = new Set(['script', 'style', 'textarea', 'title']);
// Never translated, never entered: code and everything a machine reads. <data> is a value a
// page derives (a count, a ratio, a date): its words around it are translated, the value is not,
// and a value that changes does not change the key of the sentence it sits in.
const OPAQUE = new Set(['script', 'style', 'pre', 'code', 'kbd', 'samp', 'var', 'svg', 'math', 'data',
  'textarea', 'template', 'select', 'noscript', 'iframe', 'object', 'canvas', 'video', 'audio']);
const INLINE = new Set(['a', 'abbr', 'b', 'bdi', 'bdo', 'br', 'cite', 'code', 'data', 'del', 'dfn',
  'em', 'i', 'img', 'ins', 'kbd', 'mark', 'q', 's', 'samp', 'small', 'span', 'strong', 'sub',
  'sup', 'time', 'u', 'var', 'wbr', 'svg', 'label', 'button', 'abbr']);
const ATTRIBUTES = ['title', 'alt', 'aria-label', 'aria-description', 'placeholder'];
const META = new Set(['description', 'og:title', 'og:description', 'twitter:title',
  'twitter:description']);

// ---------------------------------------------------------------------------------------------
// The document, as positions in its own text. Nothing is ever re-serialised: an edit is a
// replacement of a range, so every byte this pass does not mean to change is the build's.

export function parse(html) {
  const root = { type: 'el', name: '#root', attrs: [], children: [], parent: null };
  const stack = [root];
  let i = 0;
  const n = html.length;
  const text = (start, end) => {
    if (end > start) stack[stack.length - 1].children.push({ type: 'text', start, end });
  };
  while (i < n) {
    const lt = html.indexOf('<', i);
    if (lt < 0) { text(i, n); break; }
    text(i, lt);
    if (html.startsWith('<!--', lt)) {
      const end = html.indexOf('-->', lt + 4);
      i = end < 0 ? n : end + 3;
      continue;
    }
    if (html[lt + 1] === '!' || html[lt + 1] === '?') {
      const end = html.indexOf('>', lt);
      i = end < 0 ? n : end + 1;
      continue;
    }
    const close = html[lt + 1] === '/';
    const m = /^[a-zA-Z][\w:-]*/.exec(html.slice(lt + (close ? 2 : 1), lt + 80));
    if (!m) { text(lt, lt + 1); i = lt + 1; continue; }
    const name = m[0].toLowerCase();
    let j = lt + (close ? 2 : 1) + m[0].length;
    const attrs = [];
    const attr = /\s*([^\s=/>"']+)(?:\s*=\s*("([^"]*)"|'([^']*)'|([^\s>"']+)))?|\s*\/(?!>)/y;
    for (;;) {
      attr.lastIndex = j;
      if (/^\s*\/?>/.test(html.slice(j, j + 64))) break;
      const a = attr.exec(html);
      if (!a || a[0].length === 0) break;
      if (a[1]) {
        const quoted = a[3] !== undefined ? a[3] : a[4];
        const value = quoted !== undefined ? quoted : a[5];
        let vstart = -1;
        if (value !== undefined) {
          const off = a[0].lastIndexOf(a[2]);
          vstart = j + off + (quoted !== undefined ? 1 : 0);
        }
        attrs.push({ name: a[1].toLowerCase(), value: value ?? '', vstart,
          vend: vstart < 0 ? -1 : vstart + (value ?? '').length });
      }
      j = attr.lastIndex;
    }
    const gt = html.indexOf('>', j);
    const end = gt < 0 ? n : gt + 1;
    if (close) {
      for (let k = stack.length - 1; k > 0; k -= 1) {
        if (stack[k].name === name) {
          stack[k].closeStart = lt;
          stack[k].closeEnd = end;
          stack.length = k;
          break;
        }
      }
      i = end;
      continue;
    }
    const el = { type: 'el', name, start: lt, openEnd: end, attrs, children: [],
      parent: stack[stack.length - 1] };
    stack[stack.length - 1].children.push(el);
    const selfClosing = html[end - 2] === '/';
    if (VOID.has(name) || selfClosing) {
      el.closeStart = end; el.closeEnd = end;
      i = end;
      continue;
    }
    if (RAW.has(name)) {
      const closeAt = html.toLowerCase().indexOf(`</${name}`, end);
      const stop = closeAt < 0 ? n : closeAt;
      if (stop > end) el.children.push({ type: 'text', start: end, end: stop });
      const gt2 = closeAt < 0 ? n : html.indexOf('>', closeAt);
      el.closeStart = stop;
      el.closeEnd = gt2 < 0 ? n : gt2 + 1;
      i = el.closeEnd;
      continue;
    }
    stack.push(el);
    i = end;
  }
  for (let k = stack.length - 1; k > 0; k -= 1) {
    stack[k].closeStart = n; stack[k].closeEnd = n;
  }
  return root;
}

function attrOf(el, name) {
  return el.attrs.find((a) => a.name === name);
}

function opaque(el) {
  if (OPAQUE.has(el.name)) return true;
  if (attrOf(el, 'translate')?.value === 'no') return true;
  if (attrOf(el, 'data-i18n')?.value === 'off') return true;
  const cls = attrOf(el, 'class')?.value ?? '';
  return /\bnotranslate\b/.test(cls);
}

function hasBlock(el) {
  if (el.block !== undefined) return el.block;
  let block = false;
  for (const c of el.children) {
    if (c.type !== 'el') continue;
    if (opaque(c) && INLINE.has(c.name)) continue;
    if (!INLINE.has(c.name) || hasBlock(c)) { block = true; break; }
  }
  el.block = block;
  return block;
}

function inlineish(node) {
  if (node.type === 'text') return true;
  return INLINE.has(node.name) && !hasBlock(node);
}

// ---------------------------------------------------------------------------------------------
// Segments.

const decode = (s) => s.replace(/&(#x?[0-9a-f]+|[a-z]+);/gi, (m, e) => {
  const named = { amp: '&', lt: '<', gt: '>', quot: '"', apos: "'", nbsp: '\u00a0' };
  if (e[0] === '#') {
    const cp = e[1] === 'x' || e[1] === 'X' ? parseInt(e.slice(2), 16) : parseInt(e.slice(1), 10);
    return Number.isFinite(cp) ? String.fromCodePoint(cp) : m;
  }
  return named[e.toLowerCase()] ?? m;
});

/** Whether a piece of text is words a reader reads, rather than a name a machine does. */
export function translatable(plain) {
  const t = plain.replace(/\s+/g, ' ').trim();
  if (!/[A-Za-z]{2}/.test(t)) return false;
  if (!/\s/.test(t) && /[._:/@#=\\]|\d|[a-z][A-Z]/.test(t)) return false;
  if (/^(v?\d[\w.+-]*|[A-Z]\d+|https?:\/\/\S+)$/.test(t)) return false;
  return true;
}

export const keyOf = (source) =>
  createHash('sha256').update(source.replace(/\s+/g, ' ').trim()).digest('hex').slice(0, 20);

// One run of inline content: its range, the source a translator sees, and the raw tags each
// placeholder stands for.
function runSegment(html, nodes) {
  let first = 0;
  let last = nodes.length - 1;
  const blank = (nd) => nd.type === 'text' && html.slice(nd.start, nd.end).trim() === '';
  while (first <= last && blank(nodes[first])) first += 1;
  while (last >= first && blank(nodes[last])) last -= 1;
  if (first > last) return null;
  // A run that is one element wrapping everything is translated inside that element: the
  // wrapper is structure, not words, and the same words in another wrapper are one segment.
  const only = nodes[first];
  if (first === last && only.type === 'el' && !opaque(only) && !VOID.has(only.name)
    && only.children.length) {
    return runSegment(html, only.children);
  }
  const parts = [];
  const tags = [];
  let plain = '';
  const firstText = nodes[first];
  const start = firstText.type === 'text'
    ? firstText.start + (html.slice(firstText.start, firstText.end).length
      - html.slice(firstText.start, firstText.end).trimStart().length)
    : firstText.start;
  const lastNode = nodes[last];
  const end = lastNode.type === 'text'
    ? lastNode.start + html.slice(lastNode.start, lastNode.end).trimEnd().length
    : lastNode.closeEnd;
  const walk = (nd, clipStart, clipEnd) => {
    if (nd.type === 'text') {
      const s = Math.max(nd.start, clipStart);
      const e = Math.min(nd.end, clipEnd);
      if (e > s) {
        const t = html.slice(s, e);
        parts.push(t);
        plain += decode(t);
      }
      return;
    }
    const id = tags.length + 1;
    if (opaque(nd) || VOID.has(nd.name) || nd.closeStart === nd.openEnd && nd.children.length === 0
      && VOID.has(nd.name)) {
      tags.push({ id, whole: html.slice(nd.start, nd.closeEnd) });
      parts.push(`<t${id}/>`);
      plain += ' x ';
      return;
    }
    const tag = { id, open: [nd.start, nd.openEnd], close: html.slice(nd.closeStart, nd.closeEnd) };
    tags.push(tag);
    parts.push(`<t${id}>`);
    for (const c of nd.children) walk(c, clipStart, clipEnd);
    parts.push(`</t${id}>`);
  };
  for (let k = first; k <= last; k += 1) walk(nodes[k], start, end);
  const source = parts.join('').replace(/\s+/g, ' ').trim();
  if (!translatable(plain)) return null;
  return { kind: 'html', start, end, source, tags, key: keyOf(source) };
}

/** Every segment of a page, in document order, with the ranges they replace. */
export function segments(html, root = parse(html)) {
  const out = [];
  const visit = (el) => {
    if (el.type !== 'el') return;
    if (el.name !== '#root') {
      for (const a of el.attrs) {
        const metaName = el.name === 'meta' && a.name === 'content'
          ? (attrOf(el, 'name')?.value ?? attrOf(el, 'property')?.value ?? '') : '';
        if ((ATTRIBUTES.includes(a.name) || META.has(metaName)) && a.vstart >= 0) {
          const plain = decode(a.value);
          if (translatable(plain)) {
            const source = plain.replace(/\s+/g, ' ').trim();
            out.push({ kind: 'attr', start: a.vstart, end: a.vend, source, key: keyOf(source) });
          }
        }
      }
    }
    if (el.name === 'title') {
      const t = el.children[0];
      if (t) {
        const plain = decode(html.slice(t.start, t.end));
        if (translatable(plain)) {
          const source = plain.replace(/\s+/g, ' ').trim();
          out.push({ kind: 'text', start: t.start, end: t.end, source, key: keyOf(source) });
        }
      }
      return;
    }
    if (el.name !== '#root' && opaque(el)) return;
    let run = [];
    const flush = () => {
      if (run.length) {
        // controls side by side with only whitespace between them — a row of buttons, a list of
        // links — are separate words, each translated on its own, never one sentence
        const elements = run.filter((nd) => nd.type === 'el');
        const loose = elements.length > 1 && run.every((nd) => nd.type === 'el'
          || html.slice(nd.start, nd.end).trim() === '');
        for (const part of loose ? elements.map((nd) => [nd]) : [run]) {
          const s = runSegment(html, part);
          if (s) out.push(s);
        }
        // attributes of the inline elements inside the run are still a reader's words
        for (const nd of run) if (nd.type === 'el') visitAttrsOnly(nd);
      }
      run = [];
    };
    const visitAttrsOnly = (nd) => {
      if (nd.type !== 'el') return;
      for (const a of nd.attrs) {
        if (ATTRIBUTES.includes(a.name) && a.vstart >= 0) {
          const plain = decode(a.value);
          if (translatable(plain)) {
            const source = plain.replace(/\s+/g, ' ').trim();
            out.push({ kind: 'attr', start: a.vstart, end: a.vend, source, key: keyOf(source),
              inner: true });
          }
        }
      }
      if (!opaque(nd)) for (const c of nd.children) visitAttrsOnly(c);
    };
    const blockish = el.name === '#root' || !inlineish(el);
    if (!blockish) return;
    for (const c of el.children) {
      if (inlineish(c) && !(c.type === 'el' && c.name === 'br' && run.length === 0)) {
        run.push(c);
      } else {
        flush();
        visit(c);
      }
    }
    flush();
  };
  visit(root);
  return out;
}

// ---------------------------------------------------------------------------------------------
// Translations: admitted only with the structure of their source.

const PLACEHOLDER = /<(\/?)t(\d+)(\/?)>/g;

/** Whether a translation carries every placeholder of its source, once, nested the same way. */
export function admissible(source, translation) {
  if (typeof translation !== 'string' || translation.trim() === '') return false;
  const shape = (s) => {
    const seen = [];
    const stack = [];
    for (const m of s.matchAll(PLACEHOLDER)) {
      const [, close, id, selfClose] = m;
      if (selfClose) { seen.push(`v${id}`); continue; }
      if (close) {
        if (stack.pop() !== id) return null;
        seen.push(`c${id}`);
      } else {
        stack.push(id);
        seen.push(`o${id}`);
      }
    }
    if (stack.length) return null;
    return seen.sort().join(',');
  };
  const a = shape(source);
  return a !== null && a === shape(translation);
}

const escapeText = (s) => s.replace(/&(?!#?[0-9a-z]+;)/gi, '&amp;').replace(/</g, '&lt;')
  .replace(/>/g, '&gt;');
const escapeAttr = (s) => s.replace(/&/g, '&amp;').replace(/"/g, '&quot;').replace(/</g, '&lt;');

function restore(html, seg, translation, rewriteTag) {
  if (seg.kind === 'attr') return escapeAttr(translation);
  if (seg.kind === 'text') return escapeText(translation);
  const byId = new Map(seg.tags.map((t) => [String(t.id), t]));
  let out = '';
  let last = 0;
  for (const m of translation.matchAll(PLACEHOLDER)) {
    out += escapeText(translation.slice(last, m.index));
    const [, close, id, selfClose] = m;
    const t = byId.get(id);
    if (selfClose) out += t.whole;
    else if (close) out += t.close;
    else out += rewriteTag(html.slice(t.open[0], t.open[1]));
    last = m.index + m[0].length;
  }
  out += escapeText(translation.slice(last));
  return out;
}

// ---------------------------------------------------------------------------------------------
// The memory: one JSON object per line, sharded by the first two characters of the key, sorted,
// so a change to it reads as the segments it changed.

export function loadMemory(dir) {
  const memory = new Map();
  if (!existsSync(dir)) return memory;
  for (const f of readdirSync(dir).filter((x) => x.endsWith('.jsonl')).sort()) {
    for (const line of readFileSync(join(dir, f), 'utf8').split('\n')) {
      if (!line.trim()) continue;
      const e = JSON.parse(line);
      memory.set(e.k, e);
    }
  }
  return memory;
}

export function saveMemory(dir, memory) {
  mkdirSync(dir, { recursive: true });
  const shards = new Map();
  for (const e of [...memory.values()].sort((a, b) => (a.k < b.k ? -1 : 1))) {
    const s = e.k.slice(0, 2);
    if (!shards.has(s)) shards.set(s, []);
    const { k, en, ...rest } = e;
    shards.get(s).push(JSON.stringify({ k, en, ...rest }));
  }
  for (const [s, lines] of shards) writeFileSync(join(dir, `${s}.jsonl`), `${lines.join('\n')}\n`);
}

// ---------------------------------------------------------------------------------------------
// One page, in one language.

function pagesOf(dir, skip) {
  const pages = [];
  (function walk(d) {
    for (const entry of readdirSync(d, { withFileTypes: true })) {
      const p = join(d, entry.name);
      if (entry.isDirectory()) {
        if (!skip.has(relative(dir, p))) walk(p);
      } else if (entry.name.endsWith('.html')) pages.push(p);
    }
  })(dir);
  return pages.sort();
}

/** The English route a page is served at, from its file. */
const routeOf = (rel) => `/${rel.replace(/(^|\/)index\.html$/, '$1')}`;

export function localise(html, { lang, memory, base, routes, route, englishRoute }) {
  const root = parse(html);
  const segs = segments(html, root);
  const prefix = `/${lang}`;
  const counterpart = (url) => {
    let path = url;
    let origin = '';
    if (base && url.startsWith(base)) { origin = base; path = url.slice(base.length) || '/'; }
    else if (!url.startsWith('/') || url.startsWith('//')) return url;
    const m = /^([^?#]*)(.*)$/.exec(path);
    const p = m[1] || '/';
    if (p.startsWith(`${prefix}/`) || p === prefix) return url;
    const dir = p.endsWith('/') ? p : null;
    if (dir === null || !routes.has(dir)) return url;
    return `${origin}${prefix}${p}${m[2]}`;
  };
  const rewriteTag = (raw) => {
    // the switch and the alternates name each language's copy already
    if (/\bdata-i18n-keep\b|\shreflang=/.test(raw)) return raw;
    return raw.replace(/(\s(?:href|content)\s*=\s*")([^"]*)(")/gi, (all, a, url, c) => {
      if (/\scontent\s*=/i.test(a) && !/property\s*=\s*"og:url"/i.test(raw)) return all;
      return `${a}${counterpart(url)}${c}`;
    });
  };
  const edits = [];
  let total = 0;
  let done = 0;
  const missing = [];
  const covered = [];
  for (const s of segs) {
    total += 1;
    const e = memory.get(s.key);
    const t = e && e[lang];
    if (t && admissible(s.source, t)) {
      done += 1;
      edits.push({ start: s.start, end: s.end, text: restore(html, s, t, rewriteTag), seg: s });
      covered.push([s.start, s.end]);
    } else {
      missing.push(s);
      if (s.kind === 'html') covered.push([s.start, s.end, s]);
    }
  }
  // Every tag outside a translated segment is rewritten in place; a missing html segment keeps
  // its words but its links still lead to this language.
  const inTranslated = (pos) => edits.some((e) => e.seg.kind === 'html' && pos >= e.start
    && pos < e.end);
  const tagEdits = [];
  (function walk(el) {
    for (const c of el.children) {
      if (c.type !== 'el') continue;
      if (!inTranslated(c.start) && (c.name === 'a' || c.name === 'link' || c.name === 'meta'
        || c.name === 'form')) {
        const raw = html.slice(c.start, c.openEnd);
        const next = rewriteTag(raw);
        if (next !== raw) tagEdits.push({ start: c.start, end: c.openEnd, text: next, tag: true });
      }
      walk(c);
    }
  })(root);
  const all = [...edits.filter((e) => !e.seg.inner || !inTranslated(e.start)), ...tagEdits]
    .sort((a, b) => b.start - a.start);
  // An attribute edit inside a tag that is itself rewritten: apply the attribute edit to the
  // rewritten tag text instead of the document.
  let out = html;
  const tagsByStart = new Map(tagEdits.map((t) => [t.start, t]));
  void tagsByStart;
  for (const e of all) {
    if (!e.tag) {
      const host = tagEdits.find((t) => e.start >= t.start && e.end <= t.end);
      if (host) {
        const rel = e.start - host.start;
        const raw = html.slice(host.start, host.end);
        const relEnd = e.end - host.start;
        // the tag's own rewrite shifts only href/content values, never the value edited here
        host.text = rewriteTag(raw.slice(0, rel) + e.text + raw.slice(relEnd));
        e.skip = true;
      }
    }
  }
  for (const e of all) {
    if (e.skip) continue;
    out = out.slice(0, e.start) + e.text + out.slice(e.end);
  }
  out = out.replace(/<html([^>]*?)\slang="[^"]*"/, `<html$1 lang="${lang}"`);
  out = switchLanguage(out, { lang, base, route, englishRoute, prefix });
  return { html: out, total, done, missing };
}

/** A page written in another language at an English address names only itself, in its own
 *  language, unless it already names an authored counterpart. */
function authoredAlternates(html, base, englishRoute, pageLang) {
  if (/<link rel="alternate" hreflang=/.test(html)) return html;
  return html.replace('</head>',
    `<link rel="alternate" hreflang="${pageLang}" href="${base}${englishRoute}">\n</head>`);
}

/** The language switch and the alternates, set for a page in `lang` ('en' for the source). */
export function switchLanguage(html, { lang, base, englishRoute, prefix, langs = ['en', 'cs'] }) {
  const url = (l) => `${base}${l === 'en' ? '' : `/${l}`}${englishRoute}`;
  let out = html.replace(/<a([^>]*?)\sdata-i18n-lang="([a-z-]+)"([^>]*)>/g, (all, a, l, b) => {
    let tag = `<a${a} data-i18n-lang="${l}"${b}>`;
    tag = tag.replace(/\shref="[^"]*"/, ` href="${url(l)}"`);
    tag = tag.replace(/\saria-current="[^"]*"/, '');
    if (l === lang && /\sdata-i18n-current/.test(tag)) tag = tag.replace(/>$/, ' aria-current="true">');
    return tag;
  });
  out = out.replace(/(<span[^>]*\sdata-i18n-current-label[^>]*>)[^<]*(<\/span>)/g, `$1${lang}$2`);
  if (!/<link rel="alternate" hreflang=/.test(out)) {
    const links = [...langs.map((l) => `<link rel="alternate" hreflang="${l}" href="${url(l)}">`),
      `<link rel="alternate" hreflang="x-default" href="${url('en')}">`].join('\n');
    out = out.replace('</head>', `${links}\n</head>`);
  }
  if (lang !== 'en') {
    out = out.replace(/(<[^>]*\sdata-i18n-notice)([^>]*?)\shidden(?=[\s>])/, '$1$2');
    // a page with an authored counterpart in this language names that page canonical: the
    // copy derived from the English is its duplicate, not a second original
    const authored = new RegExp(`<link rel="alternate" hreflang="${lang}" href="([^"]+)">`).exec(out);
    const canonical = authored ? authored[1] : url(lang);
    out = out.replace(/<link rel="canonical" href="[^"]*">/, `<link rel="canonical" href="${canonical}">`);
  }
  return out;
}

// ---------------------------------------------------------------------------------------------
// The pass scripts/site-build runs.

export function build({ publicDir, i18nDir, base, missingOut, exclude = [] }) {
  const config = JSON.parse(readFileSync(join(i18nDir, 'languages.json'), 'utf8'));
  const skip = new Set([...config.languages.filter((l) => l !== config.source),
    ...exclude.map((m) => m.replace(/^\/|\/$/g, ''))]);
  const pages = pagesOf(publicDir, skip);
  const routes = new Set(pages.filter((p) => p.endsWith('index.html'))
    .map((p) => routeOf(relative(publicDir, p))));
  const report = { schema: 'site-i18n/v1', source: config.source, languages: {} };
  const missingAll = {};
  for (const lang of config.languages) {
    if (lang === config.source) continue;
    const memory = loadMemory(join(i18nDir, lang, 'memory'));
    let total = 0;
    let done = 0;
    const missing = new Map();
    for (const page of pages) {
      const rel = relative(publicDir, page);
      const html = readFileSync(page, 'utf8');
      const englishRoute = routeOf(rel);
      const pageLang = /<html[^>]*\slang="([^"]+)"/.exec(html)?.[1] ?? 'en';
      let result;
      if (html.includes('<title>Redirect</title>')) {
        // a Zola alias: no words, only a target; the copy leads to the target's copy in this
        // language and keeps the marker the site's checks recognise a redirect by
        const to = (url) => {
          const path = url.slice(base.length) || '/';
          return routes.has(path) ? `${base}/${lang}${path}` : url;
        };
        const escaped = base.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
        result = { html: html.replace(new RegExp(`${escaped}/[^"'\\s<>]*`, 'g'), to),
          total: 0, done: 0, missing: [] };
      } else if (pageLang !== config.source) {
        // a page already written in another language is carried as it is, its links localised,
        // and names the original canonical: the copy is a duplicate, never a second original
        const original = authoredAlternates(html, base, englishRoute, pageLang);
        result = localise(original, { lang, memory: new Map(), base, routes, route: englishRoute,
          englishRoute });
        result.html = result.html.replace(/<html([^>]*?)\slang="[^"]*"/, `<html$1 lang="${pageLang}"`)
          .replace(/<link rel="canonical" href="[^"]*">/, `<link rel="canonical" href="${base}${englishRoute}">`);
        result.total = 0; result.done = 0; result.missing = [];
      } else {
        result = localise(html, { lang, memory, base, routes, route: englishRoute, englishRoute });
      }
      total += result.total;
      done += result.done;
      for (const s of result.missing) {
        const m = missing.get(s.key) ?? { k: s.key, en: s.source, pages: 0, routes: [] };
        m.pages += 1;
        if (m.routes.length < 8 && !m.routes.includes(englishRoute)) m.routes.push(englishRoute);
        missing.set(s.key, m);
      }
      const target = join(publicDir, lang, rel);
      mkdirSync(dirname(target), { recursive: true });
      writeFileSync(target, result.html);
    }
    report.languages[lang] = { segments: total, translated: done, pages: pages.length,
      unique_missing: missing.size,
      coverage: total ? Math.floor((done / total) * 10000) / 100 : 100 };
    missingAll[lang] = [...missing.values()].sort((a, b) => b.pages - a.pages
      || (a.k < b.k ? -1 : 1));
  }
  // the source pages carry the switch and the alternates too
  for (const page of pages) {
    const rel = relative(publicDir, page);
    const read = readFileSync(page, 'utf8');
    const pageLang = /<html[^>]*\slang="([^"]+)"/.exec(read)?.[1] ?? config.source;
    const html = pageLang === config.source ? read
      : authoredAlternates(read, base, routeOf(rel), pageLang);
    const next = switchLanguage(html, { lang: config.source, base, englishRoute: routeOf(rel),
      prefix: '', langs: config.languages });
    if (next !== html) writeFileSync(page, next);
  }
  for (const lang of Object.keys(report.languages)) {
    writeFileSync(join(publicDir, lang, 'i18n.json'), `${JSON.stringify(report, null, 2)}\n`);
  }
  sitemap({ publicDir, base, routes, languages: config.languages, source: config.source });
  if (missingOut) {
    mkdirSync(dirname(missingOut), { recursive: true });
    writeFileSync(missingOut, `${JSON.stringify(missingAll)}\n`);
  }
  return report;
}

/** Every indexable page in every language, in the sitemap the build wrote: each English entry is
 *  followed by its copy in every other language, and each names all of them as hreflang
 *  alternates, the form search engines read a multilingual site in. The English sitemap decides
 *  which pages are indexable (templates/sitemap.xml, held by scripts/ci/homepage-check); a copy
 *  is listed exactly when its original is. */
export function sitemap({ publicDir, base, routes, languages, source }) {
  const file = join(publicDir, 'sitemap.xml');
  if (!existsSync(file)) return;
  const xml = readFileSync(file, 'utf8');
  if (xml.includes('xmlns:xhtml=')) return;
  const entry = /<url><loc>([^<]+)<\/loc>((?:<lastmod>[^<]*<\/lastmod>)?)<\/url>/g;
  const urlOf = (l, path) => `${base}${l === source ? '' : `/${l}`}${path}`;
  const body = xml.replace(entry, (all, loc, lastmod) => {
    const path = loc.startsWith(base) ? loc.slice(base.length) || '/' : null;
    if (!path || !routes.has(path)) return all;
    const alternates = [...languages.map((l) => `<xhtml:link rel="alternate" hreflang="${l}" href="${urlOf(l, path)}"/>`),
      `<xhtml:link rel="alternate" hreflang="x-default" href="${urlOf(source, path)}"/>`].join('');
    return languages.map((l) => `<url><loc>${urlOf(l, path)}</loc>${lastmod}${alternates}</url>`)
      .join('\n  ');
  }).replace('<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">',
    '<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml">');
  writeFileSync(file, body);
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const [cmd, publicDir, i18nDir, base, missingOut, exclude] = process.argv.slice(2);
  if (cmd !== 'build' || !publicDir || !i18nDir) {
    process.stderr.write('usage: site-i18n.mjs build <public> <i18n-dir> <base-url> [missing-out]\n');
    process.exit(2);
  }
  const report = build({ publicDir, i18nDir, base: (base || '').replace(/\/$/, ''), missingOut,
    exclude: (exclude || '').split(',').filter(Boolean) });
  for (const [lang, r] of Object.entries(report.languages)) {
    console.log(`  /${lang}/: ${r.pages} page(s), ${r.translated} of ${r.segments} segment(s) translated (${r.coverage}%), ${r.unique_missing} distinct segment(s) missing`);
  }
}
