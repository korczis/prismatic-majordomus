// Contrast: measured, and enforced on the palettes the build generates.
//
// The syntax-highlighting palettes are written by Zola from a named theme. A theme is chosen
// for how it looks, by somebody who was not measuring contrast, and the one this site had
// put eighteen of its nineteen colours below the WCAG AA threshold on its own background —
// 411 findings across 134 pages, all from one generated stylesheet.
//
// Choosing a different theme would move the number, not fix the class of defect: the next
// theme is chosen the same way. So the generated palette is *normalised* instead. Every
// colour keeps its hue and its saturation and gives up only as much lightness as the
// threshold demands, computed against the background the same stylesheet declares. A theme
// that already passes is left untouched, byte for byte.

/** The ratio WCAG 2.x asks of body-sized text (AA, 1.4.3). */
export const MINIMUM = 4.5;

const channel = (c) => {
  const s = c / 255;
  return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
};

/** Relative luminance of an [r, g, b] triple, per WCAG. */
export function luminance([r, g, b]) {
  return 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b);
}

/**
 * A CSS hex colour as `{ rgb, alpha }`; `null` when it is not one.
 *
 * ```
 * parseHex('#F80')       // { rgb: [255, 136, 0], alpha: 1 }
 * parseHex('#00000080')  // half-transparent black
 * ```
 */
export function parseHex(text) {
  const match = /^#([0-9a-fA-F]{3,8})$/.exec(text.trim());
  if (!match) return null;
  let digits = match[1];
  if (digits.length === 3 || digits.length === 4) digits = [...digits].map((c) => c + c).join('');
  if (digits.length !== 6 && digits.length !== 8) return null;
  const rgb = [0, 2, 4].map((i) => Number.parseInt(digits.slice(i, i + 2), 16));
  const alpha = digits.length === 8 ? Number.parseInt(digits.slice(6, 8), 16) / 255 : 1;
  return { rgb, alpha };
}

/** An [r, g, b] triple as `#rrggbb`. */
export function toHex(rgb) {
  return `#${rgb.map((c) => Math.round(Math.min(255, Math.max(0, c))).toString(16).padStart(2, '0')).join('')}`;
}

/** A colour composited over a background, so an alpha channel is measured rather than ignored. */
export function flatten({ rgb, alpha }, background) {
  return rgb.map((c, i) => c * alpha + background[i] * (1 - alpha));
}

/**
 * The contrast ratio between two colours, the first composited over the second.
 *
 * ```
 * Math.round(contrast('#000', '#fff'))   // 21
 * ```
 */
export function contrast(foreground, background) {
  const fg = typeof foreground === 'string' ? parseHex(foreground) : foreground;
  const bg = typeof background === 'string' ? parseHex(background) : background;
  if (!fg || !bg) return null;
  const [high, low] = [luminance(flatten(fg, bg.rgb)), luminance(bg.rgb)].sort((a, b) => b - a);
  return (high + 0.05) / (low + 0.05);
}

/** RGB to HSL and back, so lightness can move while hue and saturation stay. */
function toHsl([r, g, b]) {
  const [R, G, B] = [r / 255, g / 255, b / 255];
  const max = Math.max(R, G, B), min = Math.min(R, G, B), l = (max + min) / 2;
  if (max === min) return [0, 0, l];
  const d = max - min;
  const s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
  const h = max === R ? ((G - B) / d + (G < B ? 6 : 0)) : max === G ? (B - R) / d + 2 : (R - G) / d + 4;
  return [h / 6, s, l];
}
function fromHsl([h, s, l]) {
  if (s === 0) return [l * 255, l * 255, l * 255];
  const q = l < 0.5 ? l * (1 + s) : l + s - l * s;
  const p = 2 * l - q;
  const hue = (t) => {
    t = (t + 1) % 1;
    if (t < 1 / 6) return p + (q - p) * 6 * t;
    if (t < 1 / 2) return q;
    if (t < 2 / 3) return p + (q - p) * (2 / 3 - t) * 6;
    return p;
  };
  return [hue(h + 1 / 3), hue(h), hue(h - 1 / 3)].map((c) => c * 255);
}

/**
 * The nearest colour to `foreground` that reaches `minimum` against `background`, found by
 * moving lightness only.
 *
 * Hue and saturation are the theme's identity and are never touched; lightness is what the
 * threshold is about. A colour that already passes is returned unchanged, so a conforming
 * theme is normalised to itself.
 */
export function raise(foreground, background, minimum = MINIMUM) {
  const fg = parseHex(foreground);
  const bg = parseHex(background);
  if (!fg || !bg) return foreground;
  if (contrast(fg, bg) >= minimum) return foreground;

  // an alpha channel caps how dark a colour can get; the threshold outranks the transparency
  const flat = flatten(fg, bg.rgb);
  const [h, s] = toHsl(flat);
  const towardsDark = luminance(bg.rgb) > 0.18;
  let low = towardsDark ? 0 : toHsl(flat)[2];
  let high = towardsDark ? toHsl(flat)[2] : 1;
  let best = towardsDark ? '#000000' : '#ffffff';
  for (let i = 0; i < 24; i += 1) {
    const mid = (low + high) / 2;
    // the candidate is measured *after* it is rounded to eight bits per channel, because the
    // rounded value is the one that ships: a search over continuous colour converges on
    // 4.5000 and writes a hex that reads 4.49
    const candidate = toHex(fromHsl([h, s, mid]));
    if (contrast(candidate, bg) >= minimum) {
      best = candidate;
      if (towardsDark) low = mid; else high = mid;
    } else if (towardsDark) high = mid; else low = mid;
  }
  return best;
}

/**
 * The background a generated highlight stylesheet declares for its code block.
 *
 * Read from the stylesheet rather than passed in: the palette and the surface it sits on are
 * written by one generator, and a measurement against some other background would be a
 * measurement of nothing.
 */
export function backgroundOf(css) {
  return css.match(/background-color:\s*(#[0-9a-fA-F]{3,8})/)?.[1] ?? null;
}

/**
 * A generated palette is one stylesheet per colour scheme, and the dark one is scoped under
 * `.dark` after Zola writes it. Neither shape changes what a normalisation has to do, so the
 * selector is read loosely — everything up to the brace — and used only for reporting.
 */

/** Every `color:` declaration of a stylesheet, measured against its own background. */
export function measure(css) {
  const background = backgroundOf(css);
  if (!background) return { background: null, colours: [] };
  const colours = [];
  for (const match of css.matchAll(/([.#][\w-]+)\s*\{[^}]*?(?<!-)\bcolor:\s*(#[0-9a-fA-F]{3,8})/g)) {
    colours.push({
      selector: match[1],
      color: match[2],
      ratio: contrast(match[2], background),
    });
  }
  return { background, colours };
}

/**
 * Raise every foreground of a generated highlight stylesheet to the threshold.
 *
 * Returns the stylesheet and how many colours moved, so the build can say what it did and a
 * theme that already conforms can be recognised by the zero.
 */
export function enforce(css, minimum = MINIMUM) {
  const background = backgroundOf(css);
  if (!background) return { css, changed: 0, background: null };
  let changed = 0;
  // `background-color:` ends in `color:` too, and the palette's own surface is the reference
  // rather than a value to move: the lookbehind is what keeps this from raising the ground
  // it measures against, which is a failure that gets worse every time it runs.
  const out = css.replace(/((?<!-)\bcolor:\s*)(#[0-9a-fA-F]{3,8})/g, (whole, prefix, colour) => {
    const raised = raise(colour, background, minimum);
    if (raised.toLowerCase() === colour.toLowerCase()) return whole;
    changed += 1;
    return `${prefix}${raised}`;
  });
  return { css: out, changed, background };
}
