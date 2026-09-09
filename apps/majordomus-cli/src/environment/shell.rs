//! Shell export: the few variables a shell working in this repository benefits from,
//! emitted so that `eval "$(majordomus env export)"` is safe whatever the repository is
//! called and wherever it lives.
//!
//! # Why this exists at all
//!
//! Without it, a `.envrc` grows a pipeline: a `git` here, a `sed` there, a `jq` to read
//! the server's lease. Each is a second implementation of something this executable
//! already knows, none of them is tested, and all of them run on every `cd`. The adapter
//! should carry no logic, so the logic lives here and the adapter evaluates it.
//!
//! # Escaping
//!
//! Every value is quoted, not escaped-in-place: a repository whose path holds a quote, a
//! space, a newline, a `$(...)` or a backtick must produce an assignment that a shell
//! reads as one literal string. POSIX single quotes admit no interpolation at all, so the
//! only character needing care is the quote itself. `fish` uses single quotes too but does
//! process backslash inside them, so it gets its own rule rather than the same one with a
//! hope attached.
//!
//! [`quote_posix`] and [`quote_fish`] are exercised against a corpus of hostile values in
//! `nothing_in_a_value_can_escape_its_quotes`, and the emitted script is run through a
//! real shell in the crate's integration tests.

use std::fmt::Write as _;

use super::RepositoryEnvironment;

/// The shell dialect an export is written for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Dialect {
    /// `export NAME='value'` — what `direnv`, `bash`, `zsh`, `sh` and `ksh` all read.
    #[default]
    Posix,
    /// `set -gx NAME 'value'`.
    Fish,
}

impl Dialect {
    /// Parse a dialect by name; `None` for a shell this does not write.
    ///
    /// ```
    /// use majordomus_cli::environment::shell::Dialect;
    /// assert_eq!(Dialect::parse("direnv"), Some(Dialect::Posix));
    /// assert_eq!(Dialect::parse("bash"), Some(Dialect::Posix));
    /// assert_eq!(Dialect::parse("fish"), Some(Dialect::Fish));
    /// assert_eq!(Dialect::parse("powershell"), None);
    /// ```
    pub fn parse(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "direnv" | "posix" | "sh" | "bash" | "zsh" | "ksh" => Some(Dialect::Posix),
            "fish" => Some(Dialect::Fish),
            _ => None,
        }
    }

    /// The name as written.
    pub fn as_str(self) -> &'static str {
        match self {
            Dialect::Posix => "posix",
            Dialect::Fish => "fish",
        }
    }
}

/// One variable an export may set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variable {
    /// The name, `[A-Za-z_][A-Za-z0-9_]*`.
    pub name: &'static str,
    /// The value.
    pub value: String,
    /// What it is for, written into the script as a comment.
    pub purpose: &'static str,
}

/// The variables this snapshot exports, in a stable order.
///
/// Deliberately few. Every variable is a name a person's shell now carries everywhere,
/// and one that is merely interesting is one more thing to unset; each of these is
/// something a command in this repository actually reads.
pub fn variables(environment: &RepositoryEnvironment, share: Option<&str>) -> Vec<Variable> {
    let mut out = vec![Variable {
        name: "MAJORDOMUS_ROOT",
        value: environment.repository.root.clone(),
        purpose: "the repository this shell is working in",
    }];
    if let Some(share) = share {
        out.push(Variable {
            name: "MAJORDOMUS_SHARE",
            value: share.to_string(),
            purpose: "the kinds and schemas the executable reads at run time",
        });
    }
    // The completion adapter's default binary is `majordomus`, and in this repository that
    // name on the path is the *shell tool*, which has no `completion query`. Left unset,
    // every TAB in the checkout that ships the completion would silently answer nothing and
    // fall back to file completion — the one failure mode a completion must not have,
    // because it looks exactly like "there is nothing to complete".
    //
    // The value is this process's own executable, not the `bin/majordomus-cli` launcher.
    // The launcher builds the crate when its output is missing or stale, which is right for
    // a person running a command and catastrophic for a keypress: TAB would block on a
    // compiler. Whatever is answering this call has already been resolved and is already
    // built, so naming it is both the fastest answer and the honest one.
    if let Ok(exe) = std::env::current_exe() {
        out.push(Variable {
            name: crate::command_graph::shell::BIN_ENV,
            value: exe.to_string_lossy().into_owned(),
            purpose: "the executable the shell completion asks for candidates; never builds",
        });
    }

    // Only while a server is actually holding the lease: a variable naming an address
    // nothing answers at is worse than no variable, because a script will use it.
    if let Some(url) = environment
        .service("index")
        .and_then(|s| s.url.as_ref())
        .filter(|_| {
            environment
                .service("index")
                .is_some_and(|s| s.availability != super::ServiceAvailability::NotRunning)
        })
    {
        out.push(Variable {
            name: "MAJORDOMUS_URL",
            value: url.trim_end_matches('/').to_string(),
            purpose: "the shared server of this repository, while one is running",
        });
    }
    out
}

/// The script that sets them.
///
/// Has no side effects of any kind: it assigns variables and nothing else. It runs no
/// command, changes no directory, defines no function and touches no file, so that
/// evaluating it can do nothing but what reading it says.
pub fn export(
    environment: &RepositoryEnvironment,
    share: Option<&str>,
    dialect: Dialect,
) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "# generated by `majordomus env export`; assignments only, no side effects"
    );
    for variable in variables(environment, share) {
        debug_assert!(
            is_identifier(variable.name),
            "{} is not a shell identifier",
            variable.name
        );
        if !is_identifier(variable.name) {
            continue;
        }
        let _ = writeln!(out, "# {}", variable.purpose);
        let _ = match dialect {
            Dialect::Posix => writeln!(
                out,
                "export {}={}",
                variable.name,
                quote_posix(&variable.value)
            ),
            Dialect::Fish => writeln!(
                out,
                "set -gx {} {}",
                variable.name,
                quote_fish(&variable.value)
            ),
        };
    }
    out
}

/// Is this a name a shell will accept as a variable?
pub fn is_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Quote a value for `sh`, `bash`, `zsh` and `direnv`.
///
/// A POSIX single-quoted string ends at the first `'` and interprets nothing inside it, so
/// a value is safe once every `'` is closed, escaped outside the quotes, and reopened.
///
/// ```
/// use majordomus_cli::environment::shell::quote_posix;
/// assert_eq!(quote_posix("/plain/path"), "'/plain/path'");
/// assert_eq!(quote_posix("with space"), "'with space'");
/// assert_eq!(quote_posix("$(whoami)"), "'$(whoami)'");
/// assert_eq!(quote_posix("it's"), r#"'it'\''s'"#);
/// assert_eq!(quote_posix(""), "''");
/// ```
pub fn quote_posix(value: &str) -> String {
    format!("'{}'", value.replace('\'', r"'\''"))
}

/// Quote a value for `fish`.
///
/// `fish` single quotes are not POSIX single quotes: they do process `\\` and `\'`, so
/// both are escaped and everything else — `$`, backticks, newlines — is literal.
///
/// ```
/// use majordomus_cli::environment::shell::quote_fish;
/// assert_eq!(quote_fish("/plain/path"), "'/plain/path'");
/// assert_eq!(quote_fish("it's"), r"'it\'s'");
/// assert_eq!(quote_fish(r"back\slash"), r"'back\\slash'");
/// assert_eq!(quote_fish("$(whoami)"), "'$(whoami)'");
/// ```
pub fn quote_fish(value: &str) -> String {
    format!("'{}'", value.replace('\\', r"\\").replace('\'', r"\'"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The values here come from the filesystem and from a lease file, so they are
    /// attacker-influenced in exactly the way that matters: a repository can be cloned
    /// into a directory with any name at all, and its path is then evaluated by a shell on
    /// every `cd`. Nothing in a value may end its own quoting.
    const HOSTILE: &[&str] = &[
        "'; rm -rf /; echo '",
        "$(touch /tmp/pwned)",
        "`touch /tmp/pwned`",
        "${IFS}evil",
        "a\nb",
        "a\\'b",
        "\\",
        "''''",
        "\"double\"",
        "a;b|c&d",
        "*",
        "~",
        "$HOME",
        "",
    ];

    #[test]
    fn nothing_in_a_value_can_escape_its_quotes() {
        for value in HOSTILE {
            let posix = quote_posix(value);
            assert!(posix.starts_with('\'') && posix.ends_with('\''));
            // Every quote inside is part of the `'\''` sequence and never a bare one that
            // would end the string early and leave the rest as shell code.
            let inside = &posix[1..posix.len() - 1];
            assert!(
                !inside.contains('\'') || inside.contains(r"'\''"),
                "{value:?} produced {posix}"
            );

            let fish = quote_fish(value);
            assert!(fish.starts_with('\'') && fish.ends_with('\''));
            let inside = &fish[1..fish.len() - 1];
            let mut chars = inside.chars().peekable();
            while let Some(c) = chars.next() {
                if c == '\\' {
                    // an escape must consume the character after it
                    assert!(chars.next().is_some(), "{value:?} produced {fish}");
                } else {
                    assert_ne!(c, '\'', "{value:?} produced {fish}");
                }
            }
        }
    }

    #[test]
    fn a_quoted_value_survives_a_real_shell() {
        for value in HOSTILE {
            let script = format!("printf '%s' {}", quote_posix(value));
            let out = std::process::Command::new("sh")
                .arg("-c")
                .arg(&script)
                .output()
                .expect("sh runs");
            assert!(out.status.success(), "{script} failed");
            assert_eq!(
                String::from_utf8_lossy(&out.stdout),
                *value,
                "{script} did not round-trip"
            );
            // and it must not have run anything
            assert!(!std::path::Path::new("/tmp/pwned").exists());
        }
    }

    #[test]
    fn a_name_that_is_not_an_identifier_is_refused() {
        assert!(is_identifier("MAJORDOMUS_ROOT"));
        assert!(is_identifier("_x1"));
        for bad in ["1BAD", "has-dash", "has space", "", "a;b", "PATH=x"] {
            assert!(!is_identifier(bad), "{bad:?} was accepted");
        }
    }

    #[test]
    fn every_exported_name_is_an_identifier() {
        for name in ["MAJORDOMUS_ROOT", "MAJORDOMUS_SHARE", "MAJORDOMUS_URL"] {
            assert!(is_identifier(name));
        }
    }

    #[test]
    fn the_script_assigns_and_does_nothing_else() {
        let environment = crate::environment::render::tests_support::environment();
        let script = export(&environment, Some("/a/share"), Dialect::Posix);
        for line in script.lines().filter(|l| !l.starts_with('#')) {
            assert!(line.starts_with("export "), "{line:?} is not an assignment");
        }
        assert!(script.contains("MAJORDOMUS_ROOT="));
        assert!(script.contains("MAJORDOMUS_SHARE='/a/share'"));
    }

    #[test]
    fn fish_gets_its_own_dialect_and_not_a_posix_script() {
        let environment = crate::environment::render::tests_support::environment();
        let script = export(&environment, None, Dialect::Fish);
        assert!(script.contains("set -gx MAJORDOMUS_ROOT "));
        assert!(!script.contains("export "));
    }

    #[test]
    fn no_address_is_exported_when_no_server_is_running() {
        let mut environment = crate::environment::render::tests_support::environment();
        for service in &mut environment.services {
            service.url = None;
            service.availability = super::super::ServiceAvailability::NotRunning;
        }
        let script = export(&environment, None, Dialect::Posix);
        assert!(
            !script.contains("MAJORDOMUS_URL"),
            "an address nothing answers at was exported"
        );
    }
}
