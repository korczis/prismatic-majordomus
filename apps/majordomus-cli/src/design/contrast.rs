//! Contrast: whether a colour the declaration pairs with a ground can actually be read on
//! it.
//!
//! ADR 0036 made every colour of every first-party surface one declaration and proved that
//! the projections carry it. Nothing proved the *choices*. The declaration pairs
//! foregrounds with grounds — reading text on the page, a status word on its own tint, the
//! label of a primary action on its fill — in a light theme and a dark one, and until this
//! module every one of those pairs was chosen by eye. `scripts/site-build` already raises a
//! third party's syntax-highlighting palette to the threshold with
//! `scripts/lib/ui-contrast.mjs`; the repository's own palette was the unmeasured one.
//!
//! # What is measured, and where the pairs come from
//!
//! Not a list. A list of pairs goes stale the day a role is added, which is the defect this
//! repository exists to prevent. The pair set is derived, from two things that already
//! exist:
//!
//! - **the declaration**, which resolves every role and every status colour to a literal
//!   per theme (`DesignSystem::resolve`), and which files each status's text, ground and
//!   border together — so a status's parts are known to belong to one another; and
//! - **the primitives that consume it** — `share/design/primitives.css`,
//!   `share/design/base.css` and `share/cockpit/src/cockpit.css`, the stylesheets a person
//!   writes over the tokens. A rule that sets `color:` and `background:` together states a
//!   pair outright. A rule that sets only a colour states a foreground that lands on
//!   whatever ground a container gave it, so it is measured against every ground a
//!   container sets — a ground declared by a rule that carries no text of its own.
//!
//! The site's templates are not read, and do not need to be: the site writes Flowbite's
//! vocabulary, and the declaration's `alias` makes every one of those names a synonym of
//! the same roles (ADR 0036 §3). Measuring the roles measures the site.
//!
//! `--mj-status-fg`, `--mj-status-bg` and `--mj-status-line` are the indirection the
//! generated status sheet sets per state word, all three at once, from one status. So a
//! reference to one of them expands to one binding per status, and two bindings from
//! different statuses are never paired: a badge measures emerald on emerald and rose on
//! rose, never emerald on rose. What stands after the comma in `var(--mj-status-fg,
//! var(--mj-sunken))` is the word that is filed under no status, and pairs with the other
//! fallbacks of the same rule.
//!
//! # The threshold
//!
//! WCAG 2.1 AA, and the same number the repository already holds:
//! [`MINIMUM_TEXT`] is `scripts/lib/ui-contrast.mjs`'s `MINIMUM`, which cites 1.4.3.
//! Nothing here can know that a given pair is only ever rendered at large-text size, so
//! every text pair is held to the stricter 4.5, never to the 3:1 that 1.4.3 allows large
//! text.
//!
//! A border or an outline is measured too, at [`MINIMUM_NON_TEXT`] (1.4.11), and reported
//! — but it is not enforced. The design states, in the primitive that draws them, that
//! "the word is always shown, so nothing is carried by colour alone": a badge's rule and a
//! table's rule repeat a distinction the text beside them already carries, which is what
//! 1.4.11 exempts as decoration. The ratio is measured and shown so that a person can see
//! it; the number this module fails on is the text one.
//!
//! A value composed at run time — `color-mix()`, a translucent overlay — is not measured:
//! what it resolves to depends on what is under it, and a ratio computed against a guess
//! would be a measurement of nothing. Such a declaration is skipped and counted.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{DesignSystem, PREFIX};

/// The standard every threshold here comes from.
pub const STANDARD: &str = "WCAG 2.1 AA";

/// The ratio WCAG 2.1 asks of body-sized text (AA, 1.4.3). The same constant
/// `scripts/lib/ui-contrast.mjs` exports as `MINIMUM` and enforces on the generated
/// highlighting palettes; one threshold, stated in one place per language.
pub const MINIMUM_TEXT: f64 = 4.5;

/// The ratio WCAG 2.1 asks of a non-text element that carries meaning (AA, 1.4.11), and of
/// large text (1.4.3). Measured and reported; see the module documentation for why a
/// border of this design is not held to it.
pub const MINIMUM_NON_TEXT: f64 = 3.0;

/// The stylesheets that consume the declaration, relative to the distribution's share
/// directory. Every colour a person writes over the tokens is in one of these; the
/// generated sheets declare values and pair nothing.
pub const CONSUMERS: &[&str] = &[
    "design/primitives.css",
    "design/base.css",
    "cockpit/src/cockpit.css",
];

// --------------------------------------------------------------------- colour arithmetic

/// A colour literal as eight bits per channel — the value that ships, so a ratio is
/// computed on what a browser will actually paint.
///
/// `#rgb`, `#rrggbb` and `oklch(L% C H)` are understood, which is every form the
/// declaration uses. A value with an alpha channel is refused rather than guessed at: what
/// it resolves to depends on what is beneath it.
fn srgb(literal: &str) -> Option<[f64; 3]> {
    let text = literal.trim();
    if let Some(digits) = text.strip_prefix('#') {
        let expanded: String = match digits.len() {
            3 => digits.chars().flat_map(|c| [c, c]).collect(),
            6 => digits.to_string(),
            _ => return None,
        };
        let mut out = [0.0; 3];
        for (i, slot) in out.iter_mut().enumerate() {
            *slot = f64::from(u8::from_str_radix(expanded.get(i * 2..i * 2 + 2)?, 16).ok()?);
        }
        return Some(out);
    }
    let inner = text.strip_prefix("oklch(")?.strip_suffix(')')?;
    if inner.contains('/') {
        return None; // an alpha channel: not a colour on its own
    }
    let parts: Vec<&str> = inner.split_whitespace().collect();
    let [lightness, chroma, hue] = parts.as_slice() else {
        return None;
    };
    let l = lightness.strip_suffix('%')?.parse::<f64>().ok()? / 100.0;
    let c = chroma.parse::<f64>().ok()?;
    let h = hue.parse::<f64>().ok()?.to_radians();
    Some(oklab_to_srgb(l, c * h.cos(), c * h.sin()))
}

/// Oklab to eight-bit sRGB: the inverse of the transform Oklch is defined by, then the
/// sRGB transfer function. The matrices are Björn Ottosson's, as CSS Color 4 states them.
fn oklab_to_srgb(l: f64, a: f64, b: f64) -> [f64; 3] {
    let (l_, m_, s_) = (
        l + 0.396_337_777_4 * a + 0.215_803_757_3 * b,
        l - 0.105_561_345_8 * a - 0.063_854_172_8 * b,
        l - 0.089_484_177_5 * a - 1.291_485_548_0 * b,
    );
    let (lc, mc, sc) = (l_ * l_ * l_, m_ * m_ * m_, s_ * s_ * s_);
    let linear = [
        4.076_741_662_1 * lc - 3.307_711_591_3 * mc + 0.230_969_929_2 * sc,
        -1.268_438_004_6 * lc + 2.609_757_401_1 * mc - 0.341_319_396_5 * sc,
        -0.004_196_086_3 * lc - 0.703_418_614_7 * mc + 1.707_614_701_0 * sc,
    ];
    let mut out = [0.0; 3];
    for (slot, value) in out.iter_mut().zip(linear) {
        let encoded = if value <= 0.003_130_8 {
            12.92 * value
        } else {
            1.055 * value.max(0.0).powf(1.0 / 2.4) - 0.055
        };
        *slot = (encoded * 255.0).round().clamp(0.0, 255.0);
    }
    out
}

/// Relative luminance of an eight-bit sRGB triple, per WCAG.
fn luminance(rgb: [f64; 3]) -> f64 {
    let channel = |c: f64| {
        let s = c / 255.0;
        if s <= 0.039_28 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(rgb[0]) + 0.7152 * channel(rgb[1]) + 0.0722 * channel(rgb[2])
}

/// The contrast ratio between two colour literals, to two decimal places — the precision
/// the report states and the precision the threshold is compared at, so that what a reader
/// sees is what was judged.
///
/// `None` when either literal is not a form this module measures.
pub fn ratio(foreground: &str, ground: &str) -> Option<f64> {
    let (fg, bg) = (luminance(srgb(foreground)?), luminance(srgb(ground)?));
    let (high, low) = if fg > bg { (fg, bg) } else { (bg, fg) };
    Some((((high + 0.05) / (low + 0.05)) * 100.0).round() / 100.0)
}

// ------------------------------------------------------------------------ what is derived

/// What a colour is doing where it was found.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Carries {
    /// Text: held to [`MINIMUM_TEXT`].
    Text,
    /// A border or an outline: measured against [`MINIMUM_NON_TEXT`], reported, not enforced.
    NonText,
}

/// One measured pair: a foreground on a ground, in one theme.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Measured {
    /// The foreground token, as the declaration names it (`muted`, `ok`).
    pub foreground: String,
    /// The palette entry it resolves to in this theme.
    pub foreground_entry: String,
    /// The literal that entry holds.
    pub foreground_value: String,
    /// The ground token.
    pub ground: String,
    /// The palette entry the ground resolves to in this theme.
    pub ground_entry: String,
    /// The literal that entry holds.
    pub ground_value: String,
    /// `light` or `dark`.
    pub theme: String,
    /// Text, or a border.
    pub carries: Carries,
    /// The measured ratio, to two decimal places.
    pub ratio: f64,
    /// What the standard asks of this pair.
    pub required: f64,
    /// Whether it reaches it.
    pub passes: bool,
    /// Whether falling short is a finding. False for a border; see the module documentation.
    pub enforced: bool,
    /// Where the pair was read: the stylesheet and the selector that states it.
    pub seen: String,
}

/// Every pair the declaration and its consumers state, measured.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ContrastReport {
    /// The fingerprint of the declaration measured.
    pub fingerprint: String,
    /// The standard the thresholds come from.
    pub standard: String,
    /// The ratio asked of text.
    pub minimum_text: f64,
    /// The ratio asked of a non-text element that carries meaning.
    pub minimum_non_text: f64,
    /// The stylesheets read, and whether each was found.
    pub sources: Vec<Source>,
    /// How many pairs were derived and measured.
    pub measured: usize,
    /// Every pair, in a stable order: theme, then what it carries, then the two tokens.
    pub pairs: Vec<Measured>,
    /// Declarations skipped because their value is composed at run time.
    pub composed: usize,
    /// One line per enforced pair that falls short, naming role, ground, theme, measured
    /// ratio and required ratio.
    pub findings: Vec<String>,
    /// Whether every enforced pair reaches its threshold.
    pub readable: bool,
}

/// One stylesheet the pair set was derived from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Source {
    /// Where it was read from, as the caller named it.
    pub path: String,
    /// How many rules of it state a colour.
    pub rules: usize,
}

// ------------------------------------------------------------------------- the derivation

/// Which half of a pair a declaration's property makes its value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Slot {
    Text,
    Ground,
    Line,
}

fn slot(property: &str) -> Option<Slot> {
    let property = property.trim().to_ascii_lowercase();
    match property.as_str() {
        "color" => Some(Slot::Text),
        "background" | "background-color" => Some(Slot::Ground),
        other if other.starts_with("border") || other.starts_with("outline") => Some(Slot::Line),
        _ => None,
    }
}

/// Which status a binding belongs to. Two bindings are paired only when they can be on the
/// screen together: the same status, the same fallback, or one of them filed under none.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Variant {
    Any,
    Status(String),
    Fallback,
}

impl Variant {
    fn compatible(&self, other: &Variant) -> bool {
        matches!(self, Variant::Any) || matches!(other, Variant::Any) || self == other
    }
}

/// One token a declaration puts in one slot, with the status it belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Binding {
    variant: Variant,
    token: String,
}

/// A declaration block of a stylesheet: its selector and the declarations it carries.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Rule {
    selector: String,
    declarations: Vec<(String, String)>,
}

/// The rules of a stylesheet, innermost first. Nesting is followed so that a rule inside
/// `@layer components {}` is a rule; a block that declares nothing is not.
fn rules(css: &str) -> Vec<Rule> {
    let text = strip_comments(css);
    let mut out = Vec::new();
    let mut stack: Vec<Rule> = Vec::new();
    let mut buffer = String::new();
    let flush = |rule: &mut Rule, buffer: &str| {
        if let Some((property, value)) = buffer.split_once(':') {
            if !property.trim().is_empty() && !value.trim().is_empty() {
                rule.declarations
                    .push((property.trim().to_string(), value.trim().to_string()));
            }
        }
    };
    for ch in text.chars() {
        match ch {
            '{' => {
                stack.push(Rule {
                    selector: buffer.split_whitespace().collect::<Vec<_>>().join(" "),
                    declarations: Vec::new(),
                });
                buffer.clear();
            }
            '}' => {
                if let Some(mut rule) = stack.pop() {
                    flush(&mut rule, &buffer);
                    if !rule.declarations.is_empty() {
                        out.push(rule);
                    }
                }
                buffer.clear();
            }
            ';' => {
                if let Some(rule) = stack.last_mut() {
                    flush(rule, &buffer);
                }
                buffer.clear();
            }
            _ => buffer.push(ch),
        }
    }
    out
}

fn strip_comments(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut rest = css;
    while let Some(start) = rest.find("/*") {
        out.push_str(&rest[..start]);
        match rest[start + 2..].find("*/") {
            Some(end) => rest = &rest[start + 2 + end + 2..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

/// Every `--mj-` token a value names, in the order it names them: the first is what a
/// browser uses when it is declared, the rest are what stands after the comma.
fn references(value: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = value;
    while let Some(at) = rest.find(PREFIX) {
        let after = &rest[at + PREFIX.len()..];
        let end = after
            .find(|c: char| !c.is_ascii_lowercase() && !c.is_ascii_digit() && c != '-')
            .unwrap_or(after.len());
        if end > 0 {
            out.push(after[..end].trim_end_matches('-').to_string());
        }
        rest = &after[end..];
    }
    out
}

/// The part of a status a `--mj-status-*` indirection stands for, and the suffix that
/// names it on a status: `ok`, `ok-bg`, `ok-line`.
fn indirection(token: &str) -> Option<&'static str> {
    match token {
        "status-fg" => Some(""),
        "status-bg" => Some("-bg"),
        "status-line" => Some("-line"),
        _ => None,
    }
}

fn bindings(design: &DesignSystem, value: &str) -> Vec<Binding> {
    let mut out = Vec::new();
    let mut first = true;
    let mut primary_is_status = false;
    for token in references(value) {
        if let Some(suffix) = indirection(&token) {
            for (role, _) in design.status.roles.iter() {
                out.push(Binding {
                    variant: Variant::Status(role.to_string()),
                    token: format!("{role}{suffix}"),
                });
            }
            primary_is_status = primary_is_status || first;
            first = false;
            continue;
        }
        let variant = if first || !primary_is_status {
            Variant::Any
        } else {
            Variant::Fallback
        };
        out.push(Binding { variant, token });
        first = false;
    }
    out
}

/// What one rule of a stylesheet puts in each slot, and where it was read.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Coloured {
    source: String,
    selector: String,
    slots: Vec<(Slot, Vec<Binding>)>,
}

/// A pair as derived, before it is measured in either theme.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Derived {
    carries_text: bool,
    foreground: String,
    ground: String,
}

/// Derive every pair the stylesheets state over this declaration.
///
/// `sources` is each stylesheet as `(name, text)`; the name is only ever reported.
fn derive(
    design: &DesignSystem,
    sources: &[(String, String)],
) -> (BTreeMap<Derived, String>, Vec<Source>, usize) {
    let mut composed = 0usize;
    let mut parsed: Vec<(String, Vec<Rule>)> = Vec::new();
    let mut described = Vec::new();
    for (name, text) in sources {
        let rules = rules(text);
        let colouring = rules
            .iter()
            .filter(|r| {
                r.declarations
                    .iter()
                    .any(|(p, v)| slot(p).is_some() && v.contains(PREFIX))
            })
            .count();
        described.push(Source {
            path: name.clone(),
            rules: colouring,
        });
        parsed.push((name.clone(), rules));
    }

    // What each rule puts in each slot, once, so that the ambient grounds can be collected
    // before any pair is made.
    let mut coloured: Vec<Coloured> = Vec::new();
    for (name, rules) in &parsed {
        for rule in rules {
            let mut found: Vec<(Slot, Vec<Binding>)> = Vec::new();
            for (property, value) in &rule.declarations {
                let Some(slot) = slot(property) else { continue };
                if !value.contains(PREFIX) {
                    continue;
                }
                if value.contains("color-mix(") {
                    composed += 1;
                    continue;
                }
                let bound: Vec<Binding> = bindings(design, value)
                    .into_iter()
                    .filter(|b| colour(design, &b.token, false).is_some())
                    .collect();
                if !bound.is_empty() {
                    found.push((slot, bound));
                }
            }
            if !found.is_empty() {
                coloured.push(Coloured {
                    source: name.clone(),
                    selector: rule.selector.clone(),
                    slots: found,
                });
            }
        }
    }

    // What a stylesheet ever draws a border or an outline with is an edge. One rule paints
    // a hairline divider with `background: var(--mj-line)`, and a hairline is not a ground
    // a reader reads on: what is read sits inside the box, not on its edge. So a token
    // used as a line anywhere is not collected as a ground below — derived from how the
    // consumers use it, so a role that stops being a border stops being excluded.
    let edges: BTreeSet<String> = coloured
        .iter()
        .flat_map(|rule| rule.slots.iter())
        .filter(|(s, _)| *s == Slot::Line)
        .flat_map(|(_, bound)| bound.iter().map(|b| b.token.clone()))
        .collect();

    // A ground a container sets — a rule that carries no text of its own — is a ground any
    // floating foreground can land on.
    let mut ambient: BTreeSet<String> = BTreeSet::new();
    for rule in &coloured {
        let has_text = rule.slots.iter().any(|(s, _)| *s == Slot::Text);
        if has_text {
            continue;
        }
        for (s, bound) in &rule.slots {
            if *s != Slot::Ground {
                continue;
            }
            for binding in bound {
                if matches!(binding.variant, Variant::Any | Variant::Fallback)
                    && !edges.contains(&binding.token)
                {
                    ambient.insert(binding.token.clone());
                }
            }
        }
    }

    let mut pairs: BTreeMap<Derived, String> = BTreeMap::new();
    for rule in &coloured {
        let (name, selector) = (&rule.source, &rule.selector);
        let grounds: Vec<&Binding> = rule
            .slots
            .iter()
            .filter(|(s, _)| *s == Slot::Ground)
            .flat_map(|(_, b)| b.iter())
            .collect();
        for (s, bound) in &rule.slots {
            let carries_text = match s {
                Slot::Text => true,
                Slot::Line => false,
                Slot::Ground => continue,
            };
            for binding in bound {
                let mut landed = false;
                for ground in grounds
                    .iter()
                    .filter(|g| g.variant.compatible(&binding.variant))
                {
                    landed = true;
                    pairs
                        .entry(Derived {
                            carries_text,
                            foreground: binding.token.clone(),
                            ground: ground.token.clone(),
                        })
                        .or_insert_with(|| format!("{name} {selector}"));
                }
                if landed {
                    continue;
                }
                // nothing under it here: it lands on whatever a container gave it
                for ground in &ambient {
                    pairs
                        .entry(Derived {
                            carries_text,
                            foreground: binding.token.clone(),
                            ground: ground.clone(),
                        })
                        .or_insert_with(|| {
                            format!("{name} {selector}, on every ground a container sets")
                        });
                }
            }
        }
    }
    (pairs, described, composed)
}

/// What a colour token resolves to in one theme: the palette entry it ends at and the
/// literal that entry holds. `None` for a token that is not a colour.
///
/// A status may name a role (`neutral` is the page's own text on the page's own well) and
/// a role names a palette entry, so the references are followed until one of them is an
/// entry — what a person would have to edit to move the colour.
fn colour(design: &DesignSystem, token: &str, dark: bool) -> Option<(String, String)> {
    let mut reference = if let Some(status) = design.status_ref(token) {
        let role = design.status.roles.get(status.role)?;
        let pair = match status.part {
            "bg" => &role.bg,
            "line" => &role.line,
            _ => &role.fg,
        };
        (if dark { &pair.dark } else { &pair.light }).clone()
    } else {
        let role = design.roles.get(token)?;
        (if dark { &role.dark } else { &role.light }).clone()
    };
    // a role may name a role; follow it, and refuse a cycle rather than spin
    for _ in 0..design.roles.len() + 1 {
        if let Some(literal) = design.palette_value(&reference) {
            return Some((reference.clone(), literal.to_string()));
        }
        let role = design.roles.get(&reference)?;
        reference = (if dark { &role.dark } else { &role.light }).clone();
    }
    None
}

/// Measure every pair the declaration and the stylesheets that consume it state.
///
/// `sources` is each consuming stylesheet as `(name, text)`; [`CONSUMERS`] names the ones
/// the distribution ships, and the name is reported rather than read.
pub fn measure(design: &DesignSystem, sources: &[(String, String)]) -> ContrastReport {
    let (derived, described, composed) = derive(design, sources);
    let mut pairs: Vec<Measured> = Vec::new();
    for (dark, theme) in [(false, "light"), (true, "dark")] {
        for (pair, seen) in &derived {
            let (Some(fg), Some(bg)) = (
                colour(design, &pair.foreground, dark),
                colour(design, &pair.ground, dark),
            ) else {
                continue;
            };
            let Some(ratio) = ratio(&fg.1, &bg.1) else {
                continue;
            };
            let required = if pair.carries_text {
                MINIMUM_TEXT
            } else {
                MINIMUM_NON_TEXT
            };
            pairs.push(Measured {
                foreground: pair.foreground.clone(),
                foreground_entry: fg.0,
                foreground_value: fg.1,
                ground: pair.ground.clone(),
                ground_entry: bg.0,
                ground_value: bg.1,
                theme: theme.to_string(),
                carries: if pair.carries_text {
                    Carries::Text
                } else {
                    Carries::NonText
                },
                ratio,
                required,
                passes: ratio >= required,
                enforced: pair.carries_text,
                seen: seen.clone(),
            });
        }
    }
    pairs.sort_by(|a, b| {
        (&a.theme, a.carries, &a.foreground, &a.ground).cmp(&(
            &b.theme,
            b.carries,
            &b.foreground,
            &b.ground,
        ))
    });
    let findings: Vec<String> = pairs
        .iter()
        .filter(|p| p.enforced && !p.passes)
        .map(|p| {
            format!(
                "{fg} ({fge} {fgv}) on {bg} ({bge} {bgv}) in the {theme} theme measures {ratio:.2}:1; {STANDARD} asks {required} of text — {seen}",
                fg = p.foreground,
                fge = p.foreground_entry,
                fgv = p.foreground_value,
                bg = p.ground,
                bge = p.ground_entry,
                bgv = p.ground_value,
                theme = p.theme,
                ratio = p.ratio,
                required = p.required,
                seen = p.seen,
            )
        })
        .collect();
    ContrastReport {
        fingerprint: design.fingerprint(),
        standard: STANDARD.to_string(),
        minimum_text: MINIMUM_TEXT,
        minimum_non_text: MINIMUM_NON_TEXT,
        sources: described,
        measured: pairs.len(),
        composed,
        readable: findings.is_empty(),
        findings,
        pairs,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn design() -> &'static DesignSystem {
        DesignSystem::compiled().expect("the declaration compiled into this executable")
    }

    fn share_dir() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../share")
    }

    fn consumers() -> Vec<(String, String)> {
        CONSUMERS
            .iter()
            .filter_map(|name| {
                let path = share_dir().join(name);
                std::fs::read_to_string(&path)
                    .ok()
                    .map(|t| (format!("share/{name}"), t))
            })
            .collect()
    }

    #[test]
    fn black_on_white_is_the_ratio_the_standard_names() {
        // the one worked example scripts/lib/ui-contrast.mjs documents itself with
        assert_eq!(ratio("#000", "#fff"), Some(21.0));
        assert_eq!(ratio("#fff", "#fff"), Some(1.0));
        // symmetric: the lighter is always the numerator
        assert_eq!(ratio("#fff", "#000"), ratio("#000", "#fff"));
    }

    #[test]
    fn oklch_resolves_to_the_hex_tailwind_publishes() {
        // the declaration quotes Tailwind's own palette; these are the hexes Tailwind
        // ships for the same steps, so a wrong transform is caught by a value and not by
        // a plausible-looking number
        assert_eq!(srgb("oklch(21% 0.034 264.665)"), Some([16.0, 24.0, 40.0])); // gray-900 #101828
        assert_eq!(
            srgb("oklch(54.6% 0.245 262.881)"),
            Some([21.0, 93.0, 252.0])
        ); // blue-600 #155dfc
        assert_eq!(
            srgb("oklch(55.1% 0.027 264.364)"),
            Some([106.0, 114.0, 130.0])
        ); // gray-500 #6a7282
        assert_eq!(srgb("#fff"), Some([255.0, 255.0, 255.0]));
        // what is not measurable is refused rather than guessed
        assert_eq!(srgb("oklch(55% 0.02 264 / 50%)"), None);
        assert_eq!(srgb("color-mix(in oklab, white 50%, black)"), None);
    }

    #[test]
    fn a_rule_that_sets_both_states_a_pair() {
        let css = "@layer components { .thing { color: var(--mj-on-accent); background: var(--mj-accent-fill); } }";
        let report = measure(design(), &[("fixture.css".into(), css.into())]);
        assert!(report.pairs.iter().any(|p| p.foreground == "on-accent"
            && p.ground == "accent-fill"
            && p.theme == "light"));
        // and only that pair: nothing else is a ground here
        assert!(report.pairs.iter().all(|p| p.ground == "accent-fill"));
        assert_eq!(report.sources[0].rules, 1);
    }

    #[test]
    fn a_foreground_with_no_ground_lands_on_every_ground_a_container_sets() {
        let css = "
          .card { background: var(--mj-raised); border: 1px solid var(--mj-line); }
          .well { background: var(--mj-sunken); }
          .note { color: var(--mj-muted); }
        ";
        let report = measure(design(), &[("fixture.css".into(), css.into())]);
        let grounds: BTreeSet<&str> = report
            .pairs
            .iter()
            .filter(|p| p.foreground == "muted" && p.theme == "light")
            .map(|p| p.ground.as_str())
            .collect();
        assert_eq!(grounds, BTreeSet::from(["raised", "sunken"]));
    }

    #[test]
    fn a_status_is_never_measured_against_another_status() {
        let css = ".mj-badge { color: var(--mj-status-fg, var(--mj-fg)); background: var(--mj-status-bg, var(--mj-sunken)); }";
        let report = measure(design(), &[("fixture.css".into(), css.into())]);
        let light: Vec<(&str, &str)> = report
            .pairs
            .iter()
            .filter(|p| p.theme == "light")
            .map(|p| (p.foreground.as_str(), p.ground.as_str()))
            .collect();
        assert!(light.contains(&("ok", "ok-bg")));
        assert!(light.contains(&("bad", "bad-bg")));
        // the fallback pairs with the fallback, and with nothing else
        assert!(light.contains(&("fg", "sunken")));
        assert!(!light.contains(&("ok", "bad-bg")));
        assert!(!light.contains(&("ok", "sunken")));
        assert!(!light.contains(&("fg", "ok-bg")));
    }

    #[test]
    fn a_border_is_measured_but_not_enforced() {
        let css = ".thing { background: var(--mj-sunken); border: 1px solid var(--mj-line); }";
        let report = measure(design(), &[("fixture.css".into(), css.into())]);
        let border = report
            .pairs
            .iter()
            .find(|p| p.foreground == "line" && p.theme == "light")
            .expect("the border is measured");
        assert_eq!(border.carries, Carries::NonText);
        assert_eq!(border.required, MINIMUM_NON_TEXT);
        assert!(!border.enforced);
        assert!(
            report.readable,
            "a border below the threshold is not a finding"
        );
    }

    #[test]
    fn a_pair_below_the_threshold_is_named_precisely() {
        // gray-400 on white is 2.6:1 — the value `faint` carried until it was measured
        let css = ".thing { color: var(--mj-faint); background: var(--mj-bg); }";
        let mut declaration = design().clone();
        declaration
            .roles
            .0
            .iter_mut()
            .filter(|(name, _)| name == "faint")
            .for_each(|(_, role)| role.light = "gray-400".into());
        let report = measure(&declaration, &[("fixture.css".into(), css.into())]);
        assert!(!report.readable);
        let finding = report
            .findings
            .iter()
            .find(|f| f.starts_with("faint ") && f.contains("light theme"))
            .expect("the failing pair is named");
        for part in [
            "faint",
            "gray-400",
            "bg",
            "light",
            "2.6",
            "4.5",
            "fixture.css",
        ] {
            assert!(
                finding.contains(part),
                "the finding does not name {part}: {finding}"
            );
        }
    }

    #[test]
    fn the_declaration_this_executable_carries_is_readable() {
        let sources = consumers();
        if sources.len() < CONSUMERS.len() {
            return; // an installed crate without the distribution beside it
        }
        let report = measure(design(), &sources);
        assert!(
            report.readable,
            "the declaration pairs colours that cannot be read:\n{}",
            report.findings.join("\n")
        );
        assert!(
            report.measured > 40,
            "only {} pairs were derived",
            report.measured
        );
        // both themes, and the pairs the declaration is explicit about
        let light: BTreeSet<(&str, &str)> = report
            .pairs
            .iter()
            .filter(|p| p.theme == "light" && p.carries == Carries::Text)
            .map(|p| (p.foreground.as_str(), p.ground.as_str()))
            .collect();
        for pair in [
            ("fg", "sunken"),
            ("muted", "raised"),
            ("faint", "sunken"),
            ("on-accent", "accent-fill"),
            ("ok", "ok-bg"),
            ("bad", "bg"),
        ] {
            assert!(light.contains(&pair), "{pair:?} was not derived");
        }
        assert!(report.pairs.iter().any(|p| p.theme == "dark"));
    }
}
