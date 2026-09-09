// The static half of UI conformance: what can be decided from the markup alone.
//
// The browser audit is the authority — it measures what a page actually does — but it needs
// a built site, a browser and a minute per fifty pages, so it cannot stand between a person
// and a commit. This half costs milliseconds, needs nothing, and holds the invariants whose
// violation is visible in the source: a scrolling box that only a pointer can reach.
//
// It scans *markup as written*, which here is Tera, awk and Rust string literals rather than
// parseable HTML, so the scanner works at the level those files agree on: an opening tag and
// its attributes. Anything it cannot read, it says so about rather than passing.

/** The utility classes and declarations that make a box scroll on the horizontal axis. */
export const SCROLLING = [
  /\boverflow-x-auto\b/,
  /\boverflow-x-scroll\b/,
  /\boverflow-auto\b/,
  /\boverflow-scroll\b/,
  /overflow-x\s*:\s*(auto|scroll)/,
];

/**
 * The element names the compiled theme makes scroll, read from the stylesheet it produced.
 *
 * A theme that makes `pre` scroll makes every `pre` on the site a scrolling region, and no
 * scan of the markup can know that — the declaration is in the CSS, one indirection away.
 * So the set is read from the CSS: any selector whose block scrolls horizontally and whose
 * last component is a bare element name contributes that name.
 *
 * ```
 * scrollingTags('.format :where(pre):not(x) { overflow-x: auto }')   // ['pre']
 * ```
 */
export function scrollingTags(css) {
  const found = new Set();
  for (const rule of css.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
    if (!/overflow-x\s*:\s*(auto|scroll)/.test(rule[2])) continue;
    for (const selector of rule[1].split(',')) {
      // `.format :where(pre):not(:where([class~=not-format] *))` and `.[&_pre] pre` are the
      // same statement — this element scrolls — written by two generators. Unwrap the
      // negations and the `:where()` groups, then read the element the selector ends on.
      const plain = selector
        .replace(/:not\((?:[^()]|\([^()]*\))*\)/g, ' ')
        .replace(/:where\(([^()]*)\)/g, '$1')
        .replace(/::?[a-zA-Z-]+(\([^()]*\))?/g, ' ')
        .trim();
      const last = plain.split(/\s+/).pop() ?? '';
      const name = last.replace(/[[.#>+~].*$/, '');
      if (/^[a-zA-Z][a-zA-Z0-9-]*$/.test(name)) found.add(name.toLowerCase());
    }
  }
  return [...found].sort();
}

/** Tags a browser already puts in the tab order, whatever we do to them. */
const FOCUSABLE = new Set(['a', 'button', 'input', 'select', 'textarea', 'summary', 'iframe']);

/**
 * Does this opening tag declare that *it* scrolls horizontally?
 *
 * Tailwind's arbitrary variants read the other way: `[&_pre]:overflow-x-auto` on an article
 * says the article's `pre` descendants scroll, not the article. Treating it as the element's
 * own overflow puts a tabindex on a box that never scrolls and, worse, reports a conforming
 * template as a failure — so those forms are removed before the question is asked. What they
 * do declare is picked up from the compiled stylesheet by `scrollingTags`, which is where a
 * descendant rule belongs.
 *
 * ```
 * scrolls('class="overflow-x-auto"')            // true
 * scrolls('class="[&_pre]:overflow-x-auto"')    // false — the pre scrolls, not this
 * ```
 */
export function scrolls(tag) {
  const own = tag.replace(/\[&[^\]]*\]:[\w-]+/g, ' ');
  return SCROLLING.some((pattern) => pattern.test(own));
}

/**
 * Every opening tag in `text`, with the line it starts on.
 *
 * Deliberately not an HTML parser: the inputs are templates and string literals, and a
 * parser would refuse them all. A tag is `<name ...>` up to the first `>` that is not inside
 * a quoted attribute value, which is the whole of what this check needs to read.
 */
export function tags(text) {
  const out = [];
  const opening = /<([a-zA-Z][a-zA-Z0-9-]*)((?:[^>"']|"[^"]*"|'[^']*')*)>/g;
  for (const match of text.matchAll(opening)) {
    out.push({
      name: match[1].toLowerCase(),
      attributes: match[2],
      whole: match[0],
      line: text.slice(0, match.index).split('\n').length,
      index: match.index,
      end: match.index + match[0].length,
    });
  }
  return out;
}


/** Elements that never contain anything, so a nesting walk must not wait for their close. */
const VOID = new Set([
  'area', 'base', 'br', 'col', 'embed', 'hr', 'img', 'input',
  'link', 'meta', 'param', 'source', 'track', 'wbr',
]);

/**
 * The class tokens of an opening tag that apply unconditionally.
 *
 * A Tailwind variant is a condition — `focus:absolute` positions the element while it has
 * focus, `sm:absolute` above a breakpoint, `[&_pre]:absolute` its descendants. None of them
 * describes the element's resting state, and a check that read them would fail the skip
 * link every page carries.
 *
 * ```
 * plainClasses('class="sr-only focus:not-sr-only focus:absolute"')   // ['sr-only']
 * ```
 */
export function plainClasses(attributes) {
  const match = /\bclass\s*=\s*("([^"]*)"|'([^']*)')/.exec(attributes);
  const value = match ? (match[2] ?? match[3] ?? '') : '';
  return value.split(/\s+/).filter((token) => token && !token.includes(':'));
}

/** Does this tag take itself out of flow, so that a containing block decides where it sits? */
const outOfFlow = (attributes) =>
  plainClasses(attributes).some((c) => c === 'sr-only' || c === 'absolute' || c === 'fixed')
  || /position\s*:\s*(absolute|fixed)/.test(attributes);

/** Is this tag a containing block for the absolutely positioned elements inside it? */
const containingBlock = (attributes) =>
  plainClasses(attributes).some((c) => c === 'relative' || c === 'absolute' || c === 'fixed' || c === 'sticky')
  || /position\s*:\s*(relative|absolute|fixed|sticky)/.test(attributes);

/**
 * Where one opening tag's element ends: the offsets of the text it contains.
 *
 * Not a parser and not trying to be. It counts one tag name in and out, which is the only
 * nesting this check has to get right. An unclosed tag yields the rest of the file, which is
 * the safe direction — the check then looks at more markup, not less.
 */
function bounds(text, tag) {
  if (VOID.has(tag.name) || /\/>\s*$/.test(tag.whole)) return { start: tag.end, end: tag.end };
  const open = new RegExp(`<${tag.name}(?=[\\s/>])`, 'gi');
  const close = new RegExp(`</${tag.name}\\s*>`, 'gi');
  let depth = 1, at = tag.end;
  while (at < text.length) {
    open.lastIndex = at; close.lastIndex = at;
    const o = open.exec(text), c = close.exec(text);
    if (!c) break;
    if (o && o.index < c.index) { depth += 1; at = o.index + 1; continue; }
    depth -= 1;
    if (depth === 0) return { start: tag.end, end: c.index };
    at = c.index + 1;
  }
  return { start: tag.end, end: text.length };
}

/**
 * Out-of-flow descendants of a scrolling box that no containing block inside it holds.
 *
 * An absolutely positioned element is placed against its nearest positioned ancestor, and
 * against the initial containing block when it has none. Put one inside a horizontally
 * scrolling box that is not itself positioned, and it leaves: its static position is inside
 * the scrolled content — past the viewport, on a narrow screen — but it is laid out against
 * the page, so the page grows sideways to reach it. A `<span class="sr-only">` in a wide
 * table's cell scrolls the whole document to reveal a one-pixel box nobody can see, and the
 * overflow it causes names no visible element, which is how it survives a review.
 *
 * The remedy is one class: a scroll container is a containing block for what it positions.
 *
 * A subtree under a containing block of its own is skipped rather than searched: whatever it
 * positions, it positions inside itself, and inside is where the scroller wants it.
 *
 * ```
 * escapes('<div class="overflow-x-auto"><span class="sr-only">x</span></div>').length   // 1
 * escapes('<div class="relative overflow-x-auto"><span class="sr-only">x</span></div>') // []
 * ```
 */
export function escapes(text) {
  const out = [];
  for (const box of tags(text)) {
    if (!scrolls(box.attributes)) continue;
    if (containingBlock(box.attributes)) continue;
    const { start, end } = bounds(text, box);
    const region = text.slice(start, end);
    let skipTo = 0;
    for (const tag of tags(region)) {
      if (tag.index < skipTo) continue;
      // Out of flow first: `absolute` makes an element a containing block for what is inside
      // it and a fugitive from what is outside it, and it is the second that this is about.
      if (!outOfFlow(tag.attributes)) {
        if (containingBlock(tag.attributes)) skipTo = bounds(region, tag).end;
        continue;
      }
      out.push({
        line: text.slice(0, start + tag.index).split('\n').length,
        rule: 'ui.positioned-escapes-scroller',
        detail: `<${tag.name}> is positioned out of flow inside a scrolling <${box.name}> that is not a containing block`,
        remedy: 'add `relative` to the scrolling box, so what it positions stays inside it',
        excerpt: tag.whole.replace(/\s+/g, ' ').slice(0, 120),
      });
      break;
    }
  }
  return out;
}

/**
 * The offences in one file's text: a scrolling box a keyboard cannot reach.
 *
 * ```
 * scan('<div class="overflow-x-auto">')            // one offence
 * scan('<div class="overflow-x-auto" tabindex="0">')  // none
 * ```
 */
export function scan(text, { scrollingTagNames = [] } = {}) {
  const offences = [];
  // A document that carries its own stylesheet answers for itself: what scrolls in it is
  // what its own CSS says scrolls, not what some other theme does to a page elsewhere.
  const own = [...text.matchAll(/<style[^>]*>([\s\S]*?)<\/style>/gi)].map((m) => m[1]).join('\n');
  const byTheme = new Set(own ? scrollingTags(own) : scrollingTagNames);
  for (const tag of tags(text)) {
    if (!scrolls(tag.attributes) && !byTheme.has(tag.name)) continue;
    if (FOCUSABLE.has(tag.name)) continue;
    if (/\btabindex\s*=/.test(tag.attributes)) continue;
    offences.push({
      line: tag.line,
      rule: 'ui.scrollable-region-focusable',
      detail: byTheme.has(tag.name)
        ? `<${tag.name}> is made to scroll horizontally by the theme and is not in the tab order`
        : `<${tag.name}> scrolls horizontally and is not in the tab order`,
      remedy: 'add tabindex="0", so a keyboard can scroll it (WCAG 2.1.1)',
      excerpt: tag.whole.replace(/\s+/g, ' ').slice(0, 120),
    });
  }
  // a CSS rule that makes a class scroll is the same defect one indirection away: the class
  // is named here so that the tag check above can be trusted not to be the whole story —
  // unless the elements carrying it are in this same document, where the check can see them
  // and has already judged them.
  for (const match of text.matchAll(/\.([a-zA-Z][\w-]*)\s*\{[^}]*overflow-x\s*:\s*(auto|scroll)/g)) {
    // A document that declares the class and carries it can be judged here, and is: the
    // warning stands only when the elements are somewhere this scan cannot see. A stylesheet
    // whose consumers are in another language is that case; a self-contained page with no
    // element carrying the class at all is not — there is nothing there to be wrong.
    const carriers = tags(text).filter((t) => new RegExp(`\\b${match[1]}\\b`).test(t.attributes));
    const selfContained = /<style[^>]*>/i.test(text);
    if (selfContained && carriers.every((t) => /\btabindex\s*=/.test(t.attributes))) continue;
    if (carriers.length && carriers.every((t) => /\btabindex\s*=/.test(t.attributes))) continue;
    offences.push({
      line: text.slice(0, match.index).split('\n').length,
      rule: 'ui.scrollable-class-declared',
      detail: `the class .${match[1]} scrolls horizontally`,
      remedy: `every element carrying .${match[1]} needs tabindex="0"; the check cannot see them from here`,
      excerpt: match[0].replace(/\s+/g, ' ').slice(0, 120),
      severity: 'warning',
    });
  }
  offences.push(...escapes(text));
  return offences;
}

/**
 * Put every scrolling box of a rendered document into the tab order.
 *
 * This is normalisation of *derived* output, not an edit to a source: the elements it
 * reaches are the ones a third party emitted — the syntax highlighter's `<pre>`, the
 * typography plugin's `<table>` — which no configuration of ours can give an attribute to.
 * Markup we write is held to the same invariant at its source, by `scan`, which is why this
 * pass finds so little on a page we control and everything on a page we do not.
 *
 * Returns the text and how many tags it changed, so the build can say what it did.
 */
export function normalise(text, { scrollingTagNames = [] } = {}) {
  const byTheme = new Set(scrollingTagNames);
  let changed = 0;
  const out = text.replace(
    /<([a-zA-Z][a-zA-Z0-9-]*)((?:[^>"']|"[^"]*"|'[^']*')*)>/g,
    (whole, name, attributes) => {
      const lower = name.toLowerCase();
      if (FOCUSABLE.has(lower)) return whole;
      if (!byTheme.has(lower) && !scrolls(attributes)) return whole;
      if (/\btabindex\s*=/.test(attributes)) return whole;
      changed += 1;
      return `<${name}${attributes} tabindex="0">`;
    },
  );
  return { text: out, changed };
}

/**
 * Give a rendered task-list checkbox the name it was born without.
 *
 * A markdown task list becomes `<li><input disabled type="checkbox"> the item text`. The
 * text is the item's, not the control's, so an accessibility engine sees a form control
 * with no accessible name — correctly, because a screen reader announces "checkbox" and
 * nothing else. The state it carries is real, so hiding it would lose information; naming
 * it from the text that follows is what a person reading the page perceives.
 *
 * Only that shape is touched: a checkbox that opens a list item, whose name can only come
 * from the text beside it. A checkbox inside a `<label>` already has a name, and rewriting
 * it would be a change to markup that was already correct.
 */
export function nameTaskCheckboxes(text) {
  let changed = 0;
  // the attributes are matched quote-aware, never as `[^>]*`: an event handler containing
  // an arrow function has a `>` inside its value, and a greedy-stop at the first `>` cuts
  // the tag in half and writes the rest of it out as text
  const pattern = /<li(?:(?:[^>"']|"[^"]*"|'[^']*')*)>\s*<input((?:[^>"']|"[^"]*"|'[^']*')*)\/?>([^<]*)/g;
  const out = text.replace(pattern, (whole, raw, following) => {
    const attributes = raw.replace(/\/\s*$/, '');
    if (!/\btype\s*=\s*"checkbox"/.test(attributes)) return whole;
    if (/\baria-label\s*=|\baria-labelledby\s*=|\bid\s*=/.test(attributes)) return whole;
    const name = following.replace(/\s+/g, ' ').trim().slice(0, 80);
    if (!name) return whole;
    changed += 1;
    const done = /\bchecked\b/.test(attributes) ? 'done' : 'not done';
    const label = escapeAttribute(`${name} — ${done}`);
    return whole.replace(`<input${raw}`, `<input${attributes} aria-label="${label}"`);
  });
  return { text: out, changed };
}

/** Escape a value for a double-quoted HTML attribute. */
function escapeAttribute(value) {
  return value.replace(/&/g, '&amp;').replace(/"/g, '&quot;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}

/**
 * The text a document renders, with every tag removed quote-aware.
 *
 * Used to hold the build's normalisations to being *normalisations*: they add attributes and
 * nothing else, so the text a reader sees must come out identical. A rewrite that cut a tag
 * in half — an event handler containing `=>` is enough — spills the rest of the tag into the
 * page as text, and that shows up here as a difference and nowhere else until somebody looks
 * at the page.
 */
export function textOf(html) {
  return html.replace(/<(?:[^>"']|"[^"]*"|'[^']*')*>/g, '').replace(/\s+/g, ' ').trim();
}

/**
 * Apply every markup normalisation to a rendered page, refusing any that changes what the
 * page says.
 *
 * Returns the text, the counts, and nothing else: a normalisation that is not lossless is a
 * thrown error, because writing it out and finding it in an audit an hour later is how a
 * build corrupts 480 pages quietly.
 */
export function normaliseRendered(html, { scrollingTagNames = [] } = {}) {
  const before = textOf(html);
  const scrolling = normalise(html, { scrollingTagNames });
  const boxes = nameTaskCheckboxes(scrolling.text);
  const after = textOf(boxes.text);
  if (after !== before) {
    const at = [...before].findIndex((c, i) => c !== after[i]);
    throw new Error(
      `the accessibility normalisation changed what the page says, near "${before.slice(Math.max(0, at - 40), at + 40)}"`,
    );
  }
  return { text: boxes.text, scrolling: scrolling.changed, named: boxes.changed };
}
