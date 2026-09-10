//! The design system: one declaration, typed, and every surface a projection of it.
//!
//! `share/design/tokens.yaml` is the only place a visual decision is made for any surface
//! this repository renders — the site published to GitHub Pages, the Cockpit, the pages the
//! executable renders itself, and the Swagger UI shell. This module reads that file into
//! [`DesignSystem`], refuses it when it is inconsistent, fingerprints it, and answers the
//! questions every projection asks of it: what a role resolves to in each theme, which
//! status a state word carries, which Flowbite name is a synonym of which role.
//! [`render`] turns the answers into the stylesheets, datasets and documents
//! `majordomus generate design` writes; the `design` capability module serves them.
//!
//! The executable carries a copy of the declaration (`tokens.yaml` beside this file, a
//! generated artifact kept current by `generate --check`) so that the capabilities, the
//! Cockpit's inspector and `explain` answer from the declaration the stylesheets were
//! projected from, and so that the crate still builds when packaged alone.
//!
//! The model is the crate's own: nothing outside constructs a [`Role`] or a [`Token`], and
//! what a client reads is the JSON the `design` capabilities answer with. Its examples are
//! therefore unit tests rather than `///` examples — see `the_compiled_declaration_answers_
//! for_every_surface` below, which is the example this header would otherwise carry.

use std::borrow::Cow;
use std::fmt;
use std::marker::PhantomData;
use std::sync::LazyLock;

use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::metadata::yaml;
use crate::policy::sha256_hex;

pub(crate) mod contrast;
pub mod render;

/// The format version this module reads.
pub const SCHEMA: u64 = 2;

/// Where the declaration lives, repository-relative.
pub const SOURCE: &str = "share/design/tokens.yaml";

/// Where the declaration lives inside the tool's data directory.
pub const SHARE_PATH: &str = "design/tokens.yaml";

/// The prefix of every custom property the design system emits for a surface to read.
pub const PREFIX: &str = "--mj-";

/// The schema id of `docs/generated/design.{json,yaml}`.
pub const DOCUMENT_SCHEMA: &str = "majordomus/design-system/v1";

/// The schema id of `site/data/registry/design.json`.
pub const SITE_SCHEMA: &str = "majordomus-site-design/v1";

/// The compiled copy of the declaration: a generated artifact, byte for byte the canonical
/// file behind a provenance banner, so the executable never answers from a design other
/// than the one its stylesheets were projected from.
const COMPILED: &str = include_str!("tokens.yaml");

// ------------------------------------------------------------------ an ordered map

/// A map that keeps the declaration's order. A role, a state, a type step is presented in
/// the order a person wrote it, on every surface, and the fingerprint is the fingerprint
/// of that order too; a `BTreeMap` would alphabetise `raised` before `sunken` and put the
/// page background last.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ordered<T>(pub Vec<(String, T)>);

impl<T> Default for Ordered<T> {
    fn default() -> Self {
        Ordered(Vec::new())
    }
}

impl<T> Ordered<T> {
    /// The entry under `key`, when there is one.
    pub fn get(&self, key: &str) -> Option<&T> {
        self.0.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }
    /// Every entry, in declaration order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &T)> {
        self.0.iter().map(|(k, v)| (k.as_str(), v))
    }
    /// Every key, in declaration order.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.0.iter().map(|(k, _)| k.as_str())
    }
    /// How many entries.
    pub fn len(&self) -> usize {
        self.0.len()
    }
    /// Whether there are none.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<T: Serialize> Serialize for Ordered<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (k, v) in &self.0 {
            map.serialize_entry(k, v)?;
        }
        map.end()
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Ordered<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct OrderedVisitor<T>(PhantomData<T>);
        impl<'de, T: Deserialize<'de>> Visitor<'de> for OrderedVisitor<T> {
            type Value = Ordered<T>;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a map whose keys are token names")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut access: A) -> Result<Self::Value, A::Error> {
                let mut out: Vec<(String, T)> = Vec::with_capacity(access.size_hint().unwrap_or(0));
                while let Some((k, v)) = access.next_entry::<String, T>()? {
                    if out.iter().any(|(seen, _)| *seen == k) {
                        return Err(serde::de::Error::custom(format!(
                            "duplicate token name '{k}'"
                        )));
                    }
                    out.push((k, v));
                }
                Ok(Ordered(out))
            }
        }
        deserializer.deserialize_map(OrderedVisitor(PhantomData))
    }
}

impl<T: JsonSchema> JsonSchema for Ordered<T> {
    fn inline_schema() -> bool {
        true
    }
    fn schema_name() -> Cow<'static, str> {
        Cow::Owned(format!("Ordered_{}", T::schema_name()))
    }
    fn schema_id() -> Cow<'static, str> {
        Cow::Owned(format!("Ordered<{}>", T::schema_id()))
    }
    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        let value = generator.subschema_for::<T>();
        schemars::json_schema!({
            "type": "object",
            "description": "Token names to entries, in declaration order.",
            "propertyNames": { "pattern": NAME_PATTERN },
            "additionalProperties": value
        })
    }
}

/// What a token name looks like: lowercase, digits and hyphens, starting with a letter.
/// The same shape the layer's identifiers have, and the shape a CSS custom property and a
/// class modifier can carry without escaping.
pub const NAME_PATTERN: &str = "^[a-z][a-z0-9-]*$";

fn is_name(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

// ------------------------------------------------------------------------ the model

/// The whole declaration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DesignSystem {
    /// The format version; this executable reads [`SCHEMA`].
    pub schema: u64,
    /// Who this is: the name and the marks.
    pub identity: Identity,
    /// The two type stacks.
    pub font: Fonts,
    /// The raw palette: a name to a CSS colour literal. Internal; a consumer never reads
    /// one directly.
    pub palette: Ordered<String>,
    /// The semantic surface: a role to a palette entry per theme.
    pub roles: Ordered<Role>,
    /// The status meanings and the vocabulary filed under them.
    pub status: Status,
    /// The named type scale and letter-spacings.
    #[serde(rename = "type")]
    pub type_: TypeScale,
    /// Layout values more than one component shares.
    pub layout: Ordered<Named>,
    /// Corner radii.
    pub radius: Ordered<Named>,
    /// Durations.
    pub motion: Ordered<Named>,
    /// The theme contract: the class that means dark and the storage key of the choice.
    pub theme: Theme,
    /// The widths a browser audit measures every page at, ascending.
    pub viewports: Vec<u32>,
    /// Other vocabularies declared as synonyms of the roles.
    pub alias: Aliases,
}

/// Who this is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    /// The product name every surface shows.
    pub name: String,
    /// The mark, relative to the design directory.
    pub mark: String,
    /// The full logo, relative to the design directory.
    pub logo: String,
    /// The social preview image, relative to the design directory.
    pub social: String,
}

/// The two type stacks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Fonts {
    /// The system stack.
    pub sans: String,
    /// The monospace stack.
    pub mono: String,
}

/// A semantic role: what it is for, and the palette entry it resolves to in each theme.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Role {
    /// What the role is for, in a line.
    pub about: String,
    /// The palette entry in the light theme.
    pub light: String,
    /// The palette entry in the dark theme.
    pub dark: String,
}

/// A value per theme.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Pair {
    /// A palette entry or a role, in the light theme.
    pub light: String,
    /// A palette entry or a role, in the dark theme.
    pub dark: String,
}

/// One status meaning: its text, its ground and its border, per theme.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StatusRole {
    /// What the meaning covers, in a line.
    pub about: String,
    /// The text colour: `--mj-<status>`.
    pub fg: Pair,
    /// The ground: `--mj-<status>-bg`.
    pub bg: Pair,
    /// The border: `--mj-<status>-line`.
    pub line: Pair,
}

/// The status semantics: the meanings, and the words filed under each.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Status {
    /// The meanings.
    pub roles: Ordered<StatusRole>,
    /// The vocabulary: a meaning to the state words that carry it.
    pub states: Ordered<Vec<String>>,
}

/// One step of the type scale.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TypeStep {
    /// What the step is for.
    pub about: String,
    /// The font size, with its unit.
    pub size: String,
    /// The line height, unitless.
    pub leading: f64,
}

/// The named type scale and letter-spacings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TypeScale {
    /// The steps: `text-<name>` in Tailwind, `--text-<name>` everywhere.
    pub scale: Ordered<TypeStep>,
    /// The letter-spacings: `tracking-<name>` in Tailwind, `--tracking-<name>` everywhere.
    pub tracking: Ordered<Named>,
}

/// A named scalar with its reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Named {
    /// What it is for.
    pub about: String,
    /// The CSS value.
    pub value: String,
}

/// The theme contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Theme {
    /// The class on the root element that means dark.
    pub class: String,
    /// The `localStorage` key the person's choice is kept under.
    #[serde(rename = "storage-key")]
    pub storage_key: String,
}

/// Other vocabularies, each a name to the token it is a synonym of.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Aliases {
    /// Flowbite's semantic colour names, as the site's templates write them: each is
    /// declared as `--color-<name>: var(--mj-<token>)` after Flowbite's own theme.
    pub flowbite: Ordered<String>,
}

// ---------------------------------------------------------------------- the answers

/// What a colour reference resolved to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Resolved {
    /// The palette entry or role the declaration named.
    pub reference: String,
    /// The CSS value a stylesheet gets: the literal of a palette entry, or `var(--mj-<role>)`.
    pub css: String,
    /// The literal after every reference is followed.
    pub literal: String,
}

/// One part of a colour token — a role has one, a status has its text, ground and border.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ColourPart {
    /// `fg`, `bg` or `line`.
    pub part: String,
    /// The custom property a surface reads.
    pub css: String,
    /// In the light theme.
    pub light: Resolved,
    /// In the dark theme.
    pub dark: Resolved,
}

/// What kind of thing a token is.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum TokenKind {
    /// A type stack.
    Font,
    /// A raw palette entry.
    Palette,
    /// A semantic surface role.
    Role,
    /// A status meaning.
    Status,
    /// A state word, filed under a status.
    State,
    /// A step of the type scale.
    Type,
    /// A letter-spacing.
    Tracking,
    /// A shared layout value.
    Layout,
    /// A corner radius.
    Radius,
    /// A duration.
    Motion,
    /// The theme contract.
    Theme,
}

impl TokenKind {
    /// The word as serialised: `role`, `status`, `state`, `type`, ... — the same word the
    /// JSON carries, so a client filters on it without knowing this enum.
    pub fn as_str(self) -> &'static str {
        match self {
            TokenKind::Font => "font",
            TokenKind::Palette => "palette",
            TokenKind::Role => "role",
            TokenKind::Status => "status",
            TokenKind::State => "state",
            TokenKind::Type => "type",
            TokenKind::Tracking => "tracking",
            TokenKind::Layout => "layout",
            TokenKind::Radius => "radius",
            TokenKind::Motion => "motion",
            TokenKind::Theme => "theme",
        }
    }

    /// Every kind, in the order the inventory presents them.
    pub const ALL: &'static [TokenKind] = &[
        TokenKind::Font,
        TokenKind::Role,
        TokenKind::Status,
        TokenKind::State,
        TokenKind::Type,
        TokenKind::Tracking,
        TokenKind::Layout,
        TokenKind::Radius,
        TokenKind::Motion,
        TokenKind::Theme,
        TokenKind::Palette,
    ];
}

/// One token, explained: where it came from, what it resolves to, what reads it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Token {
    /// The name as declared.
    pub name: String,
    /// What kind of token.
    pub kind: TokenKind,
    /// What it is for, from the declaration.
    pub about: String,
    /// The custom properties and Tailwind utilities it becomes.
    pub css: Vec<String>,
    /// For a colour token: its parts, each resolved per theme.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parts: Vec<ColourPart>,
    /// For a scalar token: the value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// For a state word: the status it is filed under.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    /// For a status: the state words filed under it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub states: Vec<String>,
    /// The Flowbite names that are synonyms of it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
    /// The generated files the token reaches.
    pub projections: Vec<String>,
}

/// A status colour reference: `ok`, `ok-bg` or `ok-line`, parsed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatusRef<'a> {
    /// The status role.
    pub role: &'a str,
    /// `fg`, `bg` or `line`.
    pub part: &'static str,
}

static COMPILED_MODEL: LazyLock<Result<DesignSystem, String>> =
    LazyLock::new(|| DesignSystem::parse(COMPILED));

impl DesignSystem {
    /// The declaration compiled into this executable.
    ///
    /// An error here is a build defect — the compiled copy is a generated artifact of a
    /// validated source — and is reported, never unwrapped, so a served process says what is
    /// wrong instead of dying.
    pub fn compiled() -> Result<&'static DesignSystem, &'static str> {
        COMPILED_MODEL.as_ref().map_err(String::as_str)
    }

    /// Read and validate a declaration.
    ///
    /// A declaration that does not carry every section is refused by the field that is
    /// missing; one that carries them all is refused by [`DesignSystem::validate`], which
    /// names the key path and the reason. Either way nothing is projected.
    pub fn parse(text: &str) -> Result<DesignSystem, String> {
        let design: DesignSystem =
            yaml::parse_into(text).map_err(|e| format!("the declaration does not parse: {e}"))?;
        let findings = design.validate();
        if findings.is_empty() {
            Ok(design)
        } else if findings.len() == 1 {
            Err(findings[0].clone())
        } else {
            Err(format!("{} (and {} more)", findings[0], findings.len() - 1))
        }
    }

    /// Every inconsistency in the declaration, each in a line. Empty means the declaration
    /// can be projected.
    ///
    /// What is refused is what would fail silently downstream: a role naming a palette
    /// entry that does not exist would become an empty custom property, which CSS treats
    /// as unset and every reader falls back from; a state word filed under two meanings
    /// would be coloured by whichever selector the browser saw last; an unbalanced quote
    /// in a value would be dropped by the parser with every gate still green.
    pub fn validate(&self) -> Vec<String> {
        let mut out = Vec::new();
        let mut refuse = |line: String| out.push(line);

        if self.schema != SCHEMA {
            refuse(format!(
                "schema: the declaration is version {}; this executable reads {SCHEMA}",
                self.schema
            ));
        }
        for (name, value) in self.palette.iter() {
            if !is_name(name) {
                refuse(format!("palette.{name}: not a token name ({NAME_PATTERN})"));
            }
            if let Some(reason) = malformed(value) {
                refuse(format!("palette.{name}: {reason}"));
            }
        }
        for stack in [("sans", &self.font.sans), ("mono", &self.font.mono)] {
            if let Some(reason) = malformed(stack.1) {
                refuse(format!("font.{}: {reason}", stack.0));
            }
        }
        for (name, role) in self.roles.iter() {
            if !is_name(name) {
                refuse(format!("roles.{name}: not a token name ({NAME_PATTERN})"));
            }
            for (theme, reference) in [("light", &role.light), ("dark", &role.dark)] {
                if self.palette.get(reference).is_none() {
                    refuse(format!(
                        "roles.{name}.{theme}: '{reference}' is not a palette entry"
                    ));
                }
            }
        }
        for (name, status) in self.status.roles.iter() {
            if !is_name(name) {
                refuse(format!(
                    "status.roles.{name}: not a token name ({NAME_PATTERN})"
                ));
            }
            if self.roles.get(name).is_some() {
                refuse(format!(
                    "status.roles.{name}: a status may not carry the name of a role"
                ));
            }
            for (part, pair) in [
                ("fg", &status.fg),
                ("bg", &status.bg),
                ("line", &status.line),
            ] {
                for (theme, reference) in [("light", &pair.light), ("dark", &pair.dark)] {
                    if self.palette.get(reference).is_none() && self.roles.get(reference).is_none()
                    {
                        refuse(format!(
                            "status.roles.{name}.{part}.{theme}: '{reference}' is neither a palette entry nor a role"
                        ));
                    }
                }
            }
        }
        let mut seen: Vec<(&str, &str)> = Vec::new();
        for (role, words) in self.status.states.iter() {
            if self.status.roles.get(role).is_none() {
                refuse(format!("status.states.{role}: not a status role"));
            }
            for word in words {
                if !is_name(word) {
                    refuse(format!(
                        "status.states.{role}: '{word}' is not a state word ({NAME_PATTERN})"
                    ));
                }
                if let Some((_, other)) = seen.iter().find(|(w, _)| *w == word.as_str()) {
                    refuse(format!(
                        "status.states.{role}: '{word}' is already filed under '{other}'; a word carries one meaning"
                    ));
                }
                if word != role && self.status.roles.get(word).is_some() {
                    refuse(format!(
                        "status.states.{role}: '{word}' is the name of another status and cannot be filed under this one"
                    ));
                }
                seen.push((word.as_str(), role));
            }
        }
        for (name, step) in self.type_.scale.iter() {
            if !is_name(name) {
                refuse(format!(
                    "type.scale.{name}: not a token name ({NAME_PATTERN})"
                ));
            }
            if !has_length_unit(&step.size) {
                refuse(format!(
                    "type.scale.{name}.size: '{}' carries no length unit",
                    step.size
                ));
            }
            if !(step.leading.is_finite() && step.leading > 0.0) {
                refuse(format!(
                    "type.scale.{name}.leading: '{}' is not a positive number",
                    step.leading
                ));
            }
        }
        for (section, entries) in [
            ("type.tracking", &self.type_.tracking),
            ("layout", &self.layout),
            ("radius", &self.radius),
            ("motion", &self.motion),
        ] {
            for (name, named) in entries.iter() {
                if !is_name(name) {
                    refuse(format!(
                        "{section}.{name}: not a token name ({NAME_PATTERN})"
                    ));
                }
                if let Some(reason) = malformed(&named.value) {
                    refuse(format!("{section}.{name}.value: {reason}"));
                }
            }
        }
        for name in self.layout.keys() {
            if self.roles.get(name).is_some() || self.status.roles.get(name).is_some() {
                refuse(format!(
                    "layout.{name}: collides with a colour token of the same name; both would be --mj-{name}"
                ));
            }
        }
        if !is_name(&self.theme.class) {
            refuse(format!(
                "theme.class: '{}' is not a class name ({NAME_PATTERN})",
                self.theme.class
            ));
        }
        if self.theme.storage_key.is_empty() || self.theme.storage_key.contains(['\'', '"', '\\']) {
            refuse("theme.storage-key: must be a plain key with no quotes".to_string());
        }
        if self.viewports.is_empty() {
            refuse("viewports: at least one width is needed".to_string());
        }
        if self.viewports.windows(2).any(|w| w[0] >= w[1]) {
            refuse("viewports: widths are listed ascending and once".to_string());
        }
        for (name, target) in self.alias.flowbite.iter() {
            if !is_name(name) {
                refuse(format!(
                    "alias.flowbite.{name}: not a token name ({NAME_PATTERN})"
                ));
            }
            if self.colour_css(target).is_none() {
                refuse(format!(
                    "alias.flowbite.{name}: '{target}' is neither a role nor a status colour (ok, ok-bg, ok-line)"
                ));
            }
        }
        out
    }

    /// The literal a palette entry holds.
    pub fn palette_value(&self, name: &str) -> Option<&str> {
        self.palette.get(name).map(String::as_str)
    }

    /// The status a state word is filed under, when it is filed.
    pub fn role_of_state(&self, word: &str) -> Option<&str> {
        if self.status.roles.get(word).is_some() {
            return Some(self.status.roles.iter().find(|(k, _)| *k == word)?.0);
        }
        self.status
            .states
            .iter()
            .find(|(_, words)| words.iter().any(|w| w == word))
            .map(|(role, _)| role)
    }

    /// Parse a status colour reference: `ok` is the text, `ok-bg` the ground, `ok-line`
    /// the border.
    pub fn status_ref<'a>(&'a self, token: &'a str) -> Option<StatusRef<'a>> {
        if let Some((role, _)) = self.status.roles.iter().find(|(k, _)| *k == token) {
            return Some(StatusRef { role, part: "fg" });
        }
        for (suffix, part) in [("-bg", "bg"), ("-line", "line")] {
            if let Some(role) = token.strip_suffix(suffix) {
                if let Some((role, _)) = self.status.roles.iter().find(|(k, _)| *k == role) {
                    return Some(StatusRef { role, part });
                }
            }
        }
        None
    }

    /// The custom property a colour token — a role or a status colour — is read as.
    pub fn colour_css(&self, token: &str) -> Option<String> {
        if self.roles.get(token).is_some() || self.status_ref(token).is_some() {
            Some(format!("{PREFIX}{token}"))
        } else {
            None
        }
    }

    /// Resolve a colour reference — a palette entry or a role — for one theme.
    pub fn resolve(&self, reference: &str, dark: bool) -> Option<Resolved> {
        if let Some(literal) = self.palette_value(reference) {
            return Some(Resolved {
                reference: reference.to_string(),
                css: literal.to_string(),
                literal: literal.to_string(),
            });
        }
        let role = self.roles.get(reference)?;
        let entry = if dark { &role.dark } else { &role.light };
        let literal = self.palette_value(entry)?.to_string();
        Some(Resolved {
            reference: reference.to_string(),
            css: format!("var({PREFIX}{reference})"),
            literal,
        })
    }

    /// The whole declaration's fingerprint: SHA-256 of its canonical JSON. Comments and
    /// formatting of the source do not move it; a value or an order does.
    pub fn fingerprint(&self) -> String {
        sha256_hex(&serde_json::to_string(self).unwrap_or_default())
    }

    /// The first twelve hexadecimal digits of the fingerprint: what the stylesheets carry
    /// as `--mj-design` so that a served page can be compared with the executable serving
    /// it.
    pub fn short_fingerprint(&self) -> String {
        self.fingerprint()[..12].to_string()
    }

    /// The one inline script every surface runs before first paint: read the stored
    /// choice, fall back to the system, set the class. One statement, so that a
    /// content-security policy can name its digest.
    pub fn theme_bootstrap(&self) -> String {
        format!(
            "try{{var t=localStorage.getItem('{key}');if(t==='dark'||(t!=='light'&&matchMedia('(prefers-color-scheme: dark)').matches)){{document.documentElement.classList.add('{class}')}}}}catch(e){{}}",
            key = self.theme.storage_key,
            class = self.theme.class
        )
    }

    /// Every token, explained, in the inventory's order.
    pub fn tokens(&self) -> Vec<Token> {
        let mut out = Vec::new();
        let aliases_of = |token: &str| -> Vec<String> {
            self.alias
                .flowbite
                .iter()
                .filter(|(_, target)| *target == token)
                .map(|(name, _)| format!("--color-{name}"))
                .collect()
        };
        for (name, stack, about) in [
            (
                "sans",
                &self.font.sans,
                "the system stack every surface reads in",
            ),
            (
                "mono",
                &self.font.mono,
                "the monospace stack for code, identifiers and logs",
            ),
        ] {
            out.push(Token {
                name: name.into(),
                kind: TokenKind::Font,
                about: about.into(),
                css: vec![format!("--font-{name}"), format!("font-{name}")],
                parts: Vec::new(),
                value: Some(stack.clone()),
                role: None,
                states: Vec::new(),
                aliases: Vec::new(),
                projections: render::projections_of(TokenKind::Font),
            });
        }
        for (name, role) in self.roles.iter() {
            let css = format!("{PREFIX}{name}");
            out.push(Token {
                name: name.into(),
                kind: TokenKind::Role,
                about: role.about.clone(),
                css: vec![css.clone()],
                parts: vec![ColourPart {
                    part: "fg".into(),
                    css,
                    light: self
                        .resolve(&role.light, false)
                        .unwrap_or_else(|| unresolved(&role.light)),
                    dark: self
                        .resolve(&role.dark, true)
                        .unwrap_or_else(|| unresolved(&role.dark)),
                }],
                value: None,
                role: None,
                states: Vec::new(),
                aliases: aliases_of(name),
                projections: render::projections_of(TokenKind::Role),
            });
        }
        for (name, status) in self.status.roles.iter() {
            let parts: Vec<ColourPart> = [
                ("fg", &status.fg),
                ("bg", &status.bg),
                ("line", &status.line),
            ]
            .into_iter()
            .map(|(part, pair)| {
                let css = if part == "fg" {
                    format!("{PREFIX}{name}")
                } else {
                    format!("{PREFIX}{name}-{part}")
                };
                ColourPart {
                    part: part.into(),
                    css,
                    light: self
                        .resolve(&pair.light, false)
                        .unwrap_or_else(|| unresolved(&pair.light)),
                    dark: self
                        .resolve(&pair.dark, true)
                        .unwrap_or_else(|| unresolved(&pair.dark)),
                }
            })
            .collect();
            let mut aliases = aliases_of(name);
            aliases.extend(aliases_of(&format!("{name}-bg")));
            aliases.extend(aliases_of(&format!("{name}-line")));
            out.push(Token {
                name: name.into(),
                kind: TokenKind::Status,
                about: status.about.clone(),
                css: parts
                    .iter()
                    .map(|p| p.css.clone())
                    .chain([format!(".mj-badge--{name}"), format!(".mj-status--{name}")])
                    .collect(),
                parts,
                value: None,
                role: None,
                states: self.status.states.get(name).cloned().unwrap_or_default(),
                aliases,
                projections: render::projections_of(TokenKind::Status),
            });
        }
        for (role, words) in self.status.states.iter() {
            for word in words {
                if word == role {
                    continue;
                }
                out.push(Token {
                    name: word.clone(),
                    kind: TokenKind::State,
                    about: format!("a state word filed under `{role}`"),
                    css: vec![format!(".mj-badge--{word}"), format!(".mj-status--{word}")],
                    parts: Vec::new(),
                    value: None,
                    role: Some(role.to_string()),
                    states: Vec::new(),
                    aliases: Vec::new(),
                    projections: render::projections_of(TokenKind::State),
                });
            }
        }
        for (name, step) in self.type_.scale.iter() {
            out.push(Token {
                name: name.into(),
                kind: TokenKind::Type,
                about: step.about.clone(),
                css: vec![
                    format!("--text-{name}"),
                    format!("--text-{name}--line-height"),
                    format!("text-{name}"),
                ],
                parts: Vec::new(),
                value: Some(format!("{}/{}", step.size, step.leading)),
                role: None,
                states: Vec::new(),
                aliases: Vec::new(),
                projections: render::projections_of(TokenKind::Type),
            });
        }
        for (name, named) in self.type_.tracking.iter() {
            out.push(scalar(
                name,
                TokenKind::Tracking,
                named,
                vec![format!("--tracking-{name}"), format!("tracking-{name}")],
            ));
        }
        for (name, named) in self.layout.iter() {
            out.push(scalar(
                name,
                TokenKind::Layout,
                named,
                vec![format!("{PREFIX}{name}")],
            ));
        }
        for (name, named) in self.radius.iter() {
            out.push(scalar(
                name,
                TokenKind::Radius,
                named,
                vec![format!("--radius-{name}"), format!("rounded-{name}")],
            ));
        }
        for (name, named) in self.motion.iter() {
            out.push(scalar(
                name,
                TokenKind::Motion,
                named,
                vec![format!("{PREFIX}motion-{name}")],
            ));
        }
        out.push(Token {
            name: "theme".into(),
            kind: TokenKind::Theme,
            about: format!(
                "the class `{}` on the root element means dark; the choice is kept under `{}`",
                self.theme.class, self.theme.storage_key
            ),
            css: vec![format!(".{}", self.theme.class)],
            parts: Vec::new(),
            value: Some(self.theme.storage_key.clone()),
            role: None,
            states: Vec::new(),
            aliases: Vec::new(),
            projections: render::projections_of(TokenKind::Theme),
        });
        for (name, literal) in self.palette.iter() {
            out.push(Token {
                name: name.into(),
                kind: TokenKind::Palette,
                about: "a raw palette entry; roles and statuses name it, consumers never do".into(),
                css: Vec::new(),
                parts: Vec::new(),
                value: Some(literal.clone()),
                role: None,
                states: Vec::new(),
                aliases: Vec::new(),
                projections: render::projections_of(TokenKind::Palette),
            });
        }
        out
    }

    /// One token by name — a role, a status, a state word, a type step, a palette entry —
    /// or by the custom property it becomes: `ok`, `succeeded` and `--mj-ok` all reach the
    /// `ok` status, and a name nothing declares answers `None` rather than a guess.
    /// `explain_answers_by_name_and_by_custom_property` below is the example.
    pub fn explain(&self, name: &str) -> Option<Token> {
        let wanted = name.trim();
        let bare = wanted
            .strip_prefix(PREFIX)
            .or_else(|| wanted.strip_prefix("--"))
            .unwrap_or(wanted);
        let tokens = self.tokens();
        tokens
            .iter()
            .find(|t| t.name == bare)
            .or_else(|| {
                tokens.iter().find(|t| {
                    t.css.iter().any(|c| c == wanted || c == bare)
                        || t.parts.iter().any(|p| p.css == wanted)
                })
            })
            .cloned()
    }
}

fn scalar(name: &str, kind: TokenKind, named: &Named, css: Vec<String>) -> Token {
    Token {
        name: name.into(),
        kind,
        about: named.about.clone(),
        css,
        parts: Vec::new(),
        value: Some(named.value.clone()),
        role: None,
        states: Vec::new(),
        aliases: Vec::new(),
        projections: render::projections_of(kind),
    }
}

fn unresolved(reference: &str) -> Resolved {
    Resolved {
        reference: reference.to_string(),
        css: String::new(),
        literal: String::new(),
    }
}

/// Why a CSS value cannot be emitted, when it cannot.
///
/// The generator this replaces once wrote `--font-sans: ...Emoji;` with an unbalanced
/// quote and once wrote `--bg:` with no value at all; the CSS parser dropped both in
/// silence and every gate stayed green. Neither can be written now.
fn malformed(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return Some("empty value".into());
    }
    if value.matches('"').count() % 2 == 1 {
        return Some("unbalanced double quote".into());
    }
    if value.contains([';', '{', '}', '\n']) {
        return Some("a value may not carry ';', braces or a line break".into());
    }
    if let Some(hex) = value.strip_prefix('#') {
        if !matches!(hex.len(), 3 | 4 | 6 | 8) || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return Some(format!("'{value}' is not a hexadecimal colour"));
        }
    }
    None
}

fn has_length_unit(value: &str) -> bool {
    ["px", "rem", "em"].iter().any(|u| {
        value
            .strip_suffix(u)
            .is_some_and(|n| !n.is_empty() && n.parse::<f64>().is_ok())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn small() -> String {
        r##"schema: 2
identity:
  name: T
  mark: brand/logo-mark.svg
  logo: brand/logo.svg
  social: brand/social-card.png
font:
  sans: ui-sans-serif, system-ui, "Noto Color Emoji"
  mono: ui-monospace, monospace
palette:
  white: "#fff"
  gray-900: oklch(21% 0.034 264.665)
  gray-50: oklch(98.5% 0.002 247.839)
  gray-200: oklch(92.8% 0.006 264.531)
  green-9: oklch(37.8% 0.077 168.94)
  green-1: oklch(97.9% 0.021 166.113)
roles:
  bg:
    about: the page
    light: white
    dark: gray-900
  sunken:
    about: a well
    light: gray-50
    dark: gray-200
  line:
    about: a border
    light: gray-200
    dark: gray-50
  fg:
    about: text
    light: gray-900
    dark: white
status:
  roles:
    ok:
      about: fine
      fg:
        light: green-9
        dark: green-1
      bg:
        light: green-1
        dark: green-9
      line:
        light: green-1
        dark: green-9
    neutral:
      about: nothing
      fg:
        light: fg
        dark: fg
      bg:
        light: sunken
        dark: sunken
      line:
        light: line
        dark: line
  states:
    ok: [ok, done, succeeded]
    neutral: [unknown]
type:
  scale:
    meta:
      about: small
      size: 10px
      leading: 1.4
  tracking:
    caps:
      about: caps
      value: 0.12em
layout:
  measure:
    about: a column
    value: 62rem
radius:
  sm:
    about: small
    value: 0.25rem
motion:
  fast:
    about: quick
    value: 150ms
theme:
  class: dark
  storage-key: color-theme
viewports: [320, 1280]
alias:
  flowbite:
    heading: fg
    success-soft: ok-bg
"##
        .to_string()
    }

    #[test]
    fn a_consistent_declaration_parses_and_keeps_its_order() {
        let d = DesignSystem::parse(&small()).expect("valid");
        let roles: Vec<&str> = d.roles.keys().collect();
        assert_eq!(
            roles,
            ["bg", "sunken", "line", "fg"],
            "declaration order, not alphabetical"
        );
        assert_eq!(d.role_of_state("done"), Some("ok"));
        assert_eq!(
            d.role_of_state("ok"),
            Some("ok"),
            "a status is its own word"
        );
        assert_eq!(d.role_of_state("unknown"), Some("neutral"));
        assert_eq!(d.status_ref("ok-line").map(|r| r.part), Some("line"));
        assert_eq!(d.colour_css("ok-bg").as_deref(), Some("--mj-ok-bg"));
        assert_eq!(d.colour_css("nothing"), None);
    }

    #[test]
    fn the_fingerprint_ignores_comments_and_follows_values() {
        let a = DesignSystem::parse(&small()).unwrap();
        let commented = small().replace("schema: 2", "# a comment\nschema: 2");
        let b = DesignSystem::parse(&commented).unwrap();
        assert_eq!(a.fingerprint(), b.fingerprint());
        let moved = small().replace("light: green-9", "light: green-1");
        let c = DesignSystem::parse(&moved).unwrap();
        assert_ne!(a.fingerprint(), c.fingerprint());
        assert_eq!(a.short_fingerprint().len(), 12);
    }

    #[test]
    fn a_role_naming_no_palette_entry_is_refused() {
        let text = small().replace("light: gray-50", "light: gray-51");
        let err = DesignSystem::parse(&text).unwrap_err();
        assert!(err.contains("roles.sunken.light"), "{err}");
        assert!(err.contains("gray-51"), "{err}");
    }

    #[test]
    fn a_word_filed_under_two_meanings_is_refused() {
        let text = small().replace("neutral: [unknown]", "neutral: [unknown, done]");
        let err = DesignSystem::parse(&text).unwrap_err();
        assert!(err.contains("'done' is already filed under 'ok'"), "{err}");
    }

    #[test]
    fn an_alias_of_nothing_is_refused() {
        let text = small().replace("heading: fg", "heading: fgg");
        let err = DesignSystem::parse(&text).unwrap_err();
        assert!(err.contains("alias.flowbite.heading"), "{err}");
    }

    #[test]
    fn a_malformed_value_never_reaches_a_stylesheet() {
        let unbalanced = small().replace("\"Noto Color Emoji\"", "\"Noto Color Emoji");
        assert!(DesignSystem::parse(&unbalanced)
            .unwrap_err()
            .contains("unbalanced double quote"));
        let empty = small().replace("value: 62rem", "value: \"\"");
        assert!(DesignSystem::parse(&empty)
            .unwrap_err()
            .contains("empty value"));
        let hex = small().replace("\"#fff\"", "\"#ffz\"");
        assert!(DesignSystem::parse(&hex)
            .unwrap_err()
            .contains("hexadecimal"));
    }

    #[test]
    fn a_duplicate_token_name_is_refused_at_parse() {
        let text = small().replace("  sunken:\n    about: a well", "  bg:\n    about: again");
        // the layer's YAML parser refuses a repeated key before the model sees it; the
        // model's own visitor refuses the same thing for any other deserializer
        let err = DesignSystem::parse(&text).unwrap_err();
        assert!(
            err.contains("duplicate") || err.contains("given twice"),
            "{err}"
        );
        let via_json = serde_json::from_str::<Ordered<u8>>(r#"{"a": 1, "a": 2}"#).unwrap_err();
        assert!(
            via_json.to_string().contains("duplicate token name 'a'"),
            "{via_json}"
        );
    }

    #[test]
    fn explain_answers_by_name_and_by_custom_property() {
        let d = DesignSystem::parse(&small()).unwrap();
        let by_name = d.explain("fg").unwrap();
        let by_css = d.explain("--mj-fg").unwrap();
        assert_eq!(by_name, by_css);
        assert_eq!(by_name.aliases, vec!["--color-heading".to_string()]);
        assert_eq!(by_name.parts[0].light.literal, "oklch(21% 0.034 264.665)");
        let status = d.explain("ok").unwrap();
        assert_eq!(status.kind, TokenKind::Status);
        assert_eq!(status.parts.len(), 3);
        assert_eq!(status.aliases, vec!["--color-success-soft".to_string()]);
        let word = d.explain("succeeded").unwrap();
        assert_eq!(word.role.as_deref(), Some("ok"));
        let neutral = d.explain("neutral").unwrap();
        assert_eq!(
            neutral.parts[0].light.css, "var(--mj-fg)",
            "a status may reference a role"
        );
        assert_eq!(neutral.parts[0].light.literal, "oklch(21% 0.034 264.665)");
        assert!(d.explain("--mj-measure").is_some());
        assert!(
            d.explain("text-meta").is_some(),
            "a Tailwind utility name is answered too"
        );
    }

    /// What the module header would show as an example: the declaration this executable
    /// carries answers the three questions every projection asks of it.
    #[test]
    fn the_compiled_declaration_answers_for_every_surface() {
        let design = DesignSystem::compiled().expect("the compiled declaration is valid");
        // a role names one palette entry per theme, and both resolve
        let fg = design.roles.get("fg").expect("the fg role");
        assert!(design.palette_value(&fg.light).is_some());
        assert!(design.palette_value(&fg.dark).is_some());
        // a state word carries exactly one status
        assert_eq!(design.role_of_state("succeeded"), Some("ok"));
        assert_eq!(design.role_of_state("nothing-like-this"), None);
        // and the fingerprint is a function of the declaration alone
        assert_eq!(design.fingerprint().len(), 64);
    }

    #[test]
    fn the_compiled_declaration_is_the_canonical_one() {
        // the compiled copy is a generated artifact of share/design/tokens.yaml; the
        // canonical file is beside the crate in this repository, so the two must agree
        let canonical = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join(SOURCE),
        )
        .expect("the canonical declaration is in the repository");
        let a = DesignSystem::parse(&canonical).expect("canonical parses");
        let b = DesignSystem::compiled().expect("compiled parses");
        assert_eq!(
            a.fingerprint(),
            b.fingerprint(),
            "the compiled copy is stale: run `majordomus generate design` and rebuild"
        );
    }

    #[test]
    fn the_theme_bootstrap_is_one_statement_over_the_declared_contract() {
        let d = DesignSystem::parse(&small()).unwrap();
        let js = d.theme_bootstrap();
        assert!(js.contains("getItem('color-theme')"));
        assert!(js.contains("classList.add('dark')"));
        assert!(!js.contains('\n'));
    }
}
