//! The remote-build guard (I3956): before a provider's shell tool runs a command, does it
//! start a build on a cluster host outside the offload path?
//!
//! On 2026-10-22 a 33-minute `cargo test --doc` ran on lundra, the build host, at load 21. It
//! had been started with a bare `ssh lundra 'cd …; cargo test'`, so it carried no owner and
//! waited for no queue. Its rustc outlived the cargo and the shell that were killed above it.
//! `majordomus offload derive` (I2398) is the path that queues a build, exports its owner
//! and stops it as a process group (slice 2). This is the check that stops the other path
//! before it runs.
//!
//! The judgement reads the command as a shell would, closely enough to find the program each
//! simple command runs:
//!
//! - quoting (single, double, backslash), `;`, `&&`, `||`, `|`, `&`, newlines, subshells and
//!   redirections;
//! - leading assignments and the wrappers that run another program (`env A=B`, `nice -n 10`,
//!   `nohup`, `timeout 60`, `sudo`, …);
//! - a script handed to a shell (`bash -c '…'`, `sh -lc '…'`) or to `eval`, read again;
//! - `ssh`, whose options are skipped the way OpenSSH's `getopt` does (`-o X`, `-p22`, `-tt`,
//!   options after the destination, `--`), and whose remote command is the rest of its words
//!   joined by spaces and read again, as the remote shell reads it.
//!
//! A command run on a host the offload declaration names (`.ai/repo/offload/*.yaml`, by its
//! id, its address or the first label of its name) is refused when its program is a build:
//! `cargo` (unless it only asks for a version or a list), `rustc`, `scripts/derive` or a
//! `scripts/rust-*` script. Everything else is let through: a read (`ps`, `df`, `uptime`),
//! `majordomus offload …` on the host, an interactive login, and any command on a host no
//! declaration names. The offload command's own ssh runs inside the executable, never
//! through the provider's shell tool, so this never sees it.

use std::collections::BTreeSet;
use std::path::Path;

use crate::offload::declaration::{OffloadDeclaration, OffloadHost};

/// The rule a refusal names: the offload is the one way a build runs on another machine.
pub(crate) const RULE: &str = "project.a-derive-offload-waits-its-turn";

/// How deep a script handed to a shell, to `eval` or to ssh is read again. A command nested
/// deeper than this is let through rather than judged.
const DEPTH: u8 = 8;

/// A build a command would start on a declared cluster host, outside the offload path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RemoteBuild {
    /// The declared host's id.
    pub(crate) host: String,
    /// The ssh destination as the command spelt it.
    pub(crate) destination: String,
    /// The build program, as the remote command spelt it.
    pub(crate) program: String,
}

impl RemoteBuild {
    /// The text a provider hands the model: what was refused, under which rule, and the way.
    pub(crate) fn refusal(&self) -> String {
        format!(
            "majordomus guard: `ssh {dest}` reaches the declared offload host {host}, and the \
             command starts `{program}` there outside the offload path ({RULE}; I3956)\n  \
             run it as `majordomus offload derive --host {host} <worktree>`: it waits its turn \
             in the host's queue, carries its owner, and stops as a process group with \
             `majordomus offload stop <tag>`\n  \
             a read (ps, df, uptime) and `ssh {dest} majordomus offload …` are not refused",
            dest = self.destination,
            host = self.host,
            program = self.program,
        )
    }
}

/// The build a provider's pre-tool payload would start on a declared host, when it would.
///
/// The payload is Claude Code's (`tool_input.command`, `cwd`). The checkout whose
/// declaration is read is the one `cwd` lies in, or `base` when the payload names none;
/// `hosts` reads that checkout's declared hosts and is asked only when the command runs ssh
/// at all, because this runs before every shell command a provider makes.
pub(crate) fn from_payload(
    payload: &str,
    base: Option<&Path>,
    hosts: impl FnOnce(&Path) -> Vec<OffloadHost>,
) -> Option<RemoteBuild> {
    let v: serde_json::Value = serde_json::from_str(payload).ok()?;
    let command = v["tool_input"]["command"].as_str()?;
    if !command.contains("ssh") {
        return None;
    }
    let cwd = v["cwd"].as_str().map(Path::new).or(base)?;
    let repository = crate::repository::Repository::discover(cwd).ok()?;
    let declared = hosts(repository.root());
    remote_build(command, &declared)
}

/// Every host the checkout at `root` declares for the offload, read from
/// `.ai/repo/offload/*.yaml` without building the index. A file that is not a valid
/// declaration names no host; none at all is an empty list, and nothing is then judged.
pub(crate) fn declared_hosts(root: &Path) -> Vec<OffloadHost> {
    let Ok(entries) = std::fs::read_dir(root.join(".ai/repo/offload")) else {
        return Vec::new();
    };
    let files: BTreeSet<_> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "yaml"))
        .collect();
    files
        .iter()
        .filter_map(|f| OffloadDeclaration::read(f).ok())
        .flat_map(|d| d.hosts)
        .collect()
}

/// The build `command` would start on one of `hosts`, read as a shell reads it.
pub(crate) fn remote_build(command: &str, hosts: &[OffloadHost]) -> Option<RemoteBuild> {
    scan(command, hosts, None, 0)
}

/// Where a script runs: here (`None`), or on a declared host, by its id and the destination
/// the command spelt.
type On<'a> = Option<(&'a str, &'a str)>;

fn scan(script: &str, hosts: &[OffloadHost], on: On<'_>, depth: u8) -> Option<RemoteBuild> {
    if depth > DEPTH {
        return None;
    }
    commands(script)
        .iter()
        .find_map(|words| judge(words, hosts, on, depth))
}

/// The build one simple command starts where it runs, if it starts one.
fn judge(words: &[String], hosts: &[OffloadHost], on: On<'_>, depth: u8) -> Option<RemoteBuild> {
    let words = unwrap(words);
    let (program, args) = words.split_first()?;
    let name = basename(program);
    match name {
        "ssh" => {
            let (destination, rest) = ssh_target(args)?;
            let host = declared(hosts, host_of(destination));
            let remote = rest.join(" ");
            match host {
                Some(h) => scan(
                    &remote,
                    hosts,
                    Some((h.id.as_str(), destination)),
                    depth + 1,
                ),
                // an undeclared host is not judged, but a hop it makes to a declared one is
                None => scan(&remote, hosts, None, depth + 1),
            }
        }
        "bash" | "sh" | "zsh" | "dash" | "ksh" => match shell_script(args) {
            Script::Inline(s) => scan(s, hosts, on, depth + 1),
            Script::File(f) => build_on(f, &args[1..], on),
            Script::None => None,
        },
        "eval" => scan(&args.join(" "), hosts, on, depth + 1),
        _ => build_on(program, args, on),
    }
}

/// `program` as the build it is on the declared host `on`, when it is one there.
fn build_on(program: &str, args: &[String], on: On<'_>) -> Option<RemoteBuild> {
    let (host, destination) = on?;
    is_build(program, args).then(|| RemoteBuild {
        host: host.to_string(),
        destination: destination.to_string(),
        program: program.to_string(),
    })
}

/// Whether `program` with `args` builds: cargo (unless it only asks what it is), rustc,
/// `scripts/derive` and the `scripts/rust-*` scripts, wherever they are spelt from.
fn is_build(program: &str, args: &[String]) -> bool {
    let name = basename(program);
    match name {
        "cargo" => {
            !args.iter().all(|a| {
                matches!(
                    a.as_str(),
                    "--version" | "-V" | "-vV" | "--list" | "--help" | "-h"
                )
            }) || args.is_empty()
        }
        "rustc" => true,
        _ => {
            let parent = Path::new(program)
                .parent()
                .and_then(Path::file_name)
                .and_then(|p| p.to_str());
            parent == Some("scripts") && (name == "derive" || name.starts_with("rust-"))
        }
    }
}

/// The last component of a program's path.
fn basename(program: &str) -> &str {
    program.rsplit('/').next().unwrap_or(program)
}

/// The words of a simple command from the program it runs: leading assignments and the
/// wrappers that run another program, with their options, taken off.
fn unwrap(mut words: &[String]) -> &[String] {
    loop {
        let Some((first, rest)) = words.split_first() else {
            return words;
        };
        if is_assignment(first) {
            words = rest;
            continue;
        }
        let Some((with_value, positional)) = wrapper(basename(first)) else {
            return words;
        };
        words = skip_options(rest, with_value);
        for _ in 0..positional {
            words = words.split_first().map_or(words, |(_, r)| r);
        }
        // `env` takes its own assignments after its options
        while words.first().is_some_and(|w| is_assignment(w)) {
            words = &words[1..];
        }
    }
}

/// A wrapper that runs the program after it: the options of its that take a value, and how
/// many positional words come before the program.
fn wrapper(name: &str) -> Option<(&'static [&'static str], usize)> {
    let pair: (&'static [&'static str], usize) = match name {
        "env" => (&["-u", "-C", "-S", "--unset", "--chdir"], 0),
        "nice" => (&["-n", "--adjustment"], 0),
        "ionice" => (&["-c", "-n", "-p", "--class", "--classdata"], 0),
        "sudo" => (
            &[
                "-u", "-g", "-h", "-p", "-C", "-D", "-U", "--user", "--group",
            ],
            0,
        ),
        "timeout" | "gtimeout" => (&["-s", "-k", "--signal", "--kill-after"], 1),
        "stdbuf" => (&["-i", "-o", "-e"], 0),
        "taskset" => (&[], 1),
        "nohup" | "time" | "command" | "exec" | "builtin" | "setsid" | "caffeinate" => (&[], 0),
        _ => return None,
    };
    Some(pair)
}

/// `words` after the leading options, an option in `with_value` taking the word after it
/// (`-n 10`) unless it carries its value (`-n10`, `--adjustment=10`). `--` ends them.
fn skip_options<'w>(mut words: &'w [String], with_value: &[&str]) -> &'w [String] {
    while let Some((w, rest)) = words.split_first() {
        if w == "--" {
            return rest;
        }
        if !w.starts_with('-') || w == "-" {
            return words;
        }
        words = rest;
        if with_value.contains(&w.as_str()) {
            words = words.split_first().map_or(words, |(_, r)| r);
        }
    }
    words
}

/// `NAME=value`, as a shell reads an assignment before a command.
fn is_assignment(word: &str) -> bool {
    let Some((name, _)) = word.split_once('=') else {
        return false;
    };
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// What a shell is asked to run.
#[derive(Debug, PartialEq, Eq)]
enum Script<'a> {
    /// A script given with `-c`.
    Inline(&'a str),
    /// A script file.
    File(&'a str),
    /// Nothing: an interactive shell.
    None,
}

/// The script a shell's `args` hand it: the first word after its options when one of them
/// carries `c` (`-c`, `-lc`, `-ec`), otherwise the first word, a file.
fn shell_script(args: &[String]) -> Script<'_> {
    let mut inline = false;
    for a in args {
        if a == "--" {
            continue;
        }
        if let Some(flags) = a.strip_prefix('-').filter(|f| !f.starts_with('-')) {
            inline |= flags.contains('c');
            continue;
        }
        if a.starts_with("--") {
            continue;
        }
        return if inline {
            Script::Inline(a)
        } else {
            Script::File(a)
        };
    }
    Script::None
}

/// The options of OpenSSH's ssh that take a value.
const SSH_WITH_VALUE: &str = "BbcDEeFIiJLlmOoPpQRSWw";

/// The destination ssh's `args` name and the words of the remote command after it, options
/// skipped as OpenSSH does: clustered (`-tt`, `-4p22`), with a value attached or in the next
/// word, before the destination and after it, `--` ending them. `None` when no destination
/// is named.
fn ssh_target(args: &[String]) -> Option<(&str, &[String])> {
    let mut destination = None;
    let mut i = 0;
    while i < args.len() {
        let w = &args[i];
        i += 1;
        if w == "--" {
            if destination.is_none() {
                destination = Some(args.get(i)?.as_str());
                i += 1;
            }
            break;
        }
        match w.strip_prefix('-').filter(|f| !f.is_empty()) {
            Some(flags) => {
                if let Some(at) = flags.find(|c| SSH_WITH_VALUE.contains(c)) {
                    if at + 1 == flags.len() {
                        i += 1;
                    }
                }
            }
            None if destination.is_none() => destination = Some(w.as_str()),
            None => {
                i -= 1;
                break;
            }
        }
    }
    Some((destination?, &args[i.min(args.len())..]))
}

/// The host of an ssh destination: `host`, `user@host`, `ssh://user@host:port`, in lower case.
fn host_of(destination: &str) -> String {
    let (uri, rest) = match destination.strip_prefix("ssh://") {
        Some(r) => (true, r),
        None => (false, destination),
    };
    let host = rest.rsplit('@').next().unwrap_or(rest);
    let host = if uri {
        host.split([':', '/']).next().unwrap_or(host)
    } else {
        host
    };
    host.to_ascii_lowercase()
}

/// The declared host `host` is, by its id, the host of its address, or a name whose first
/// label is its id (`lundra.tailnet.ts.net`).
fn declared(hosts: &[OffloadHost], host: String) -> Option<&OffloadHost> {
    let label = host.split('.').next().unwrap_or_default();
    hosts.iter().find(|h| {
        let id = h.id.to_ascii_lowercase();
        host == id || label == id || host == host_of(&h.address)
    })
}

/// One lexical piece of a command line.
#[derive(Debug, PartialEq, Eq)]
enum Token {
    /// A word, its quotes removed.
    Word(String),
    /// What ends a simple command: `;`, `&`, `|`, a newline, a parenthesis or a backtick.
    End,
    /// A redirection: the word after it is its target, not an argument.
    Redirect,
}

/// The simple commands of `script`, each its words, a redirection's target left out.
fn commands(script: &str) -> Vec<Vec<String>> {
    let mut out = Vec::new();
    let mut current = Vec::new();
    let mut target = false;
    for t in tokens(script) {
        match t {
            Token::Word(_) if target => target = false,
            Token::Word(w) => current.push(w),
            Token::Redirect => target = true,
            Token::End => {
                target = false;
                out.push(std::mem::take(&mut current));
            }
        }
    }
    out.push(current);
    out.retain(|c| !c.is_empty());
    out
}

/// The tokens of `script`, quoted as a POSIX shell quotes: `'…'` literally, `"…"` with `\`
/// escaping `"`, `\`, `$` and a backtick, `\` outside quotes escaping one character, `#` at
/// the start of a word opening a comment to the end of the line.
fn tokens(script: &str) -> Vec<Token> {
    let mut out = Vec::new();
    let mut word = String::new();
    let mut open = false;
    let mut chars = script.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            ' ' | '\t' => flush(&mut word, &mut open, &mut out),
            '\n' | ';' | '|' | '(' | ')' | '`' => {
                flush(&mut word, &mut open, &mut out);
                out.push(Token::End);
            }
            '&' if chars.peek() == Some(&'>') => {
                flush(&mut word, &mut open, &mut out);
                chars.next();
                out.push(Token::Redirect);
            }
            '&' => {
                flush(&mut word, &mut open, &mut out);
                out.push(Token::End);
            }
            '<' | '>' => {
                flush(&mut word, &mut open, &mut out);
                while chars.next_if(|n| matches!(n, '>' | '<' | '&')).is_some() {}
                out.push(Token::Redirect);
            }
            '#' if !open => while chars.next_if(|n| *n != '\n').is_some() {},
            '\'' => {
                open = true;
                word.extend(chars.by_ref().take_while(|n| *n != '\''));
            }
            '"' => {
                open = true;
                while let Some(n) = chars.next() {
                    match n {
                        '"' => break,
                        '\\' => match chars.next_if(|e| matches!(e, '"' | '\\' | '$' | '`')) {
                            Some(e) => word.push(e),
                            None => word.push('\\'),
                        },
                        _ => word.push(n),
                    }
                }
            }
            '\\' => match chars.next() {
                Some('\n') | None => {}
                Some(e) => {
                    open = true;
                    word.push(e);
                }
            },
            _ => {
                open = true;
                word.push(c);
            }
        }
    }
    flush(&mut word, &mut open, &mut out);
    out
}

/// The word being read, as a token, when one is open.
fn flush(word: &mut String, open: &mut bool, out: &mut Vec<Token>) {
    if *open {
        out.push(Token::Word(std::mem::take(word)));
        *open = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host(id: &str, address: &str) -> OffloadHost {
        serde_json::from_value(serde_json::json!({ "id": id, "address": address })).unwrap()
    }

    fn cluster() -> Vec<OffloadHost> {
        vec![
            host("lundra", "korczis@100.65.22.118"),
            host("imac", "korczis@100.122.86.48"),
        ]
    }

    fn refused(command: &str) -> RemoteBuild {
        remote_build(command, &cluster()).unwrap_or_else(|| panic!("not refused: {command}"))
    }

    fn allowed(command: &str) {
        assert_eq!(
            remote_build(command, &cluster()),
            None,
            "refused: {command}"
        );
    }

    fn words(ws: &[&str]) -> Vec<String> {
        ws.iter().map(|w| w.to_string()).collect()
    }

    #[test]
    fn every_refused_form_names_its_host_and_program() {
        let r = refused("ssh -o BatchMode=yes lundra 'cd ~/dev/x; cargo test --doc'");
        assert_eq!(
            (r.host.as_str(), r.destination.as_str(), r.program.as_str()),
            ("lundra", "lundra", "cargo")
        );
        assert_eq!(refused("ssh lundra -- cargo build").program, "cargo");
        assert_eq!(refused("ssh lundra env A=B cargo build").program, "cargo");
        assert_eq!(
            refused("ssh lundra nice -n 10 cargo build").program,
            "cargo"
        );
        assert_eq!(
            refused("ssh lundra 'nice -n10 nohup ~/.cargo/bin/cargo b'").program,
            "~/.cargo/bin/cargo"
        );
        assert_eq!(refused("ssh lundra rustc x.rs").program, "rustc");
        assert_eq!(
            refused("ssh lundra 'cd dev/p && scripts/derive'").program,
            "scripts/derive"
        );
        assert_eq!(
            refused("ssh lundra ./scripts/rust-check --doc").program,
            "./scripts/rust-check"
        );
        assert_eq!(
            refused("ssh lundra bash scripts/derive").program,
            "scripts/derive"
        );
        assert_eq!(
            refused("ssh lundra \"bash -lc 'cargo test'\"").program,
            "cargo"
        );
        assert_eq!(refused("bash -c \"ssh lundra cargo test\"").host, "lundra");
        assert_eq!(
            refused("cd /x && ssh -tt -p 22 lundra cargo t 2>&1 | tail").host,
            "lundra"
        );
        assert_eq!(refused("timeout 60 ssh imac eval cargo build").host, "imac");
        let by_address = refused("ssh korczis@100.65.22.118 cargo build");
        assert_eq!(by_address.destination, "korczis@100.65.22.118");
        assert_eq!(
            refused("ssh ssh://korczis@LUNDRA:22 cargo b").host,
            "lundra"
        );
        assert_eq!(refused("ssh lundra.tail1.ts.net cargo b").host, "lundra");
        assert_eq!(
            refused("ssh gateway ssh lundra cargo b").host,
            "lundra",
            "a hop to a declared host"
        );
        assert_eq!(
            refused("ssh -i ~/.ssh/k -lkorczis lundra -t cargo b").program,
            "cargo"
        );
        assert_eq!(
            refused("ssh lundra cargo").program,
            "cargo",
            "a bare cargo still builds nothing safely"
        );
    }

    #[test]
    fn reads_an_offload_and_another_host_are_let_through() {
        allowed("ssh lundra ps aux");
        allowed("ssh lundra 'df -g / ; uptime'");
        allowed("ssh lundra 'ps aux | grep cargo'");
        allowed("ssh lundra pkill -f cargo");
        allowed("ssh lundra cargo --version");
        allowed("ssh lundra majordomus offload stop 0123456789");
        allowed("ssh lundra 'ls > cargo'");
        allowed("ssh lundra");
        allowed("ssh -p 22");
        allowed("ssh lundra -- ");
        allowed("ssh elsewhere cargo build");
        allowed("cargo build");
        allowed("scripts/derive");
        allowed("majordomus offload derive --host lundra .");
        allowed("ssh lundra bash");
        allowed("echo ssh lundra cargo # ssh lundra cargo");
    }

    #[test]
    fn a_nesting_deeper_than_the_limit_is_let_through() {
        let mut command = "cargo build".to_string();
        for _ in 0..=DEPTH {
            command = format!("bash -c {}", shell_quote(&command));
        }
        assert_eq!(
            remote_build(&format!("ssh lundra {}", shell_quote(&command)), &cluster()),
            None
        );
        assert!(scan("ssh lundra cargo b", &cluster(), None, DEPTH + 1).is_none());
    }

    fn shell_quote(s: &str) -> String {
        format!("'{}'", s.replace('\'', "'\\''"))
    }

    #[test]
    fn the_refusal_names_the_rule_and_the_way() {
        let text = refused("ssh lundra cargo test").refusal();
        assert!(text.contains(RULE), "{text}");
        assert!(
            text.contains("majordomus offload derive --host lundra"),
            "{text}"
        );
        assert!(text.contains("majordomus offload stop"), "{text}");
        assert!(text.contains("`ssh lundra`"), "{text}");
    }

    #[test]
    fn a_payload_is_judged_in_the_checkout_of_its_cwd() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".ai")).unwrap();
        let skeleton = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../share/skeleton");
        std::fs::copy(
            skeleton.join("ai/manifest.yaml"),
            dir.path().join(".ai/manifest.yaml"),
        )
        .unwrap();
        let root = dir.path().canonicalize().unwrap();
        let payload = |command: &str, cwd: Option<&Path>| {
            let mut v =
                serde_json::json!({ "tool_name": "Bash", "tool_input": { "command": command } });
            if let Some(c) = cwd {
                v["cwd"] = serde_json::json!(c);
            }
            v.to_string()
        };
        let hosts = |asked: &Path| {
            assert_eq!(asked, root.as_path(), "the checkout of the cwd is read");
            cluster()
        };
        let never = |_: &Path| -> Vec<OffloadHost> { unreachable!("not asked") };
        let build = payload("ssh lundra cargo test", Some(dir.path()));
        assert_eq!(from_payload(&build, None, hosts).unwrap().host, "lundra");
        let no_cwd = payload("ssh lundra cargo test", None);
        assert!(
            from_payload(&no_cwd, Some(dir.path()), hosts).is_some(),
            "base stands in"
        );
        assert!(
            from_payload(&no_cwd, None, never).is_none(),
            "nowhere to read a declaration"
        );
        assert!(from_payload(&payload("ls", Some(dir.path())), None, never).is_none());
        assert!(from_payload("not json", None, never).is_none());
        let edit = r#"{"tool_input":{"file_path":"/a"}}"#;
        assert!(from_payload(edit, None, never).is_none(), "no command");
        let outside = tempfile::tempdir().unwrap();
        let bare = payload("ssh lundra cargo test", Some(outside.path()));
        assert!(
            from_payload(&bare, None, never).is_none(),
            "no checkout above it"
        );
    }

    #[test]
    fn the_declared_hosts_are_read_from_every_declaration_file() {
        let dir = tempfile::tempdir().unwrap();
        assert!(
            declared_hosts(dir.path()).is_empty(),
            "no offload directory"
        );
        let offload = dir.path().join(".ai/repo/offload");
        std::fs::create_dir_all(&offload).unwrap();
        std::fs::write(offload.join("README.md"), "not a declaration").unwrap();
        std::fs::write(offload.join("broken.yaml"), "kind: [").unwrap();
        let declaration = "schema: derive-offload/v1\nkind: derive-offload\nid: derive\n\
             hosts:\n  - id: lundra\n    address: korczis@100.65.22.118\n";
        std::fs::write(offload.join("derive.yaml"), declaration).unwrap();
        let hosts = declared_hosts(dir.path());
        assert_eq!(hosts.len(), 1);
        assert_eq!(hosts[0].id, "lundra");
    }

    #[test]
    fn the_shell_is_tokenised_as_a_shell_reads_it() {
        use Token::*;
        let w = |s: &str| Word(s.to_string());
        assert_eq!(
            tokens(
                r#"a 'b c' "d \"e\" \x" f\ g \
h"#
            ),
            vec![w("a"), w("b c"), w(r#"d "e" \x"#), w("f g"), w("h")]
        );
        assert_eq!(tokens("a&&b"), vec![w("a"), End, End, w("b")]);
        assert_eq!(tokens("a &> f"), vec![w("a"), Redirect, w("f")]);
        assert_eq!(tokens("a 2>&1"), vec![w("a"), w("2"), Redirect, w("1")]);
        assert_eq!(
            tokens("a '' b"),
            vec![w("a"), w(""), w("b")],
            "an empty quoted word"
        );
        assert_eq!(tokens("a \\"), vec![w("a")], "a trailing backslash");
        assert_eq!(tokens("a#b"), vec![w("a#b")], "a hash inside a word");
        assert_eq!(tokens("a # b c\nd"), vec![w("a"), End, w("d")], "a comment");
        assert_eq!(tokens("$(x)"), vec![w("$"), End, w("x"), End]);
        assert_eq!(
            commands("a > f b; c"),
            vec![words(&["a", "b"]), words(&["c"])]
        );
        assert_eq!(commands(">"), Vec::<Vec<String>>::new());
    }

    #[test]
    fn wrappers_and_assignments_are_taken_off() {
        assert_eq!(
            unwrap(&words(&["A=1", "env", "-u", "X", "B=2", "cargo"])),
            words(&["cargo"])
        );
        assert_eq!(
            unwrap(&words(&["nice", "-n", "10", "nohup", "cargo"])),
            words(&["cargo"])
        );
        assert_eq!(
            unwrap(&words(&["timeout", "-s", "KILL", "60", "cargo"])),
            words(&["cargo"])
        );
        assert_eq!(unwrap(&words(&["sudo", "--", "cargo"])), words(&["cargo"]));
        assert_eq!(unwrap(&words(&["taskset"])), Vec::<String>::new());
        assert_eq!(unwrap(&words(&["env", "-"])), words(&["-"]));
        assert_eq!(unwrap(&words(&["nice", "-n"])), Vec::<String>::new());
        assert!(unwrap(&[]).is_empty());
        assert!(is_assignment("_A1=x"));
        assert!(!is_assignment("1A=x"));
        assert!(!is_assignment("=x"));
        assert!(!is_assignment("a-b=x"));
        assert!(!is_assignment("cargo"));
    }

    #[test]
    fn a_shell_is_asked_for_its_script() {
        assert_eq!(shell_script(&words(&["-lc", "x"])), Script::Inline("x"));
        assert_eq!(
            shell_script(&words(&["--norc", "-e", "f"])),
            Script::File("f")
        );
        assert_eq!(shell_script(&words(&["--", "-c"])), Script::None);
        assert_eq!(shell_script(&words(&["--", "f"])), Script::File("f"));
        assert_eq!(shell_script(&[]), Script::None);
    }

    #[test]
    fn ssh_options_are_skipped_as_openssh_skips_them() {
        let a = words(&["-4p22", "-o", "X=1", "-v", "h", "-t", "ls", "-l"]);
        assert_eq!(ssh_target(&a), Some(("h", &a[6..])));
        let b = words(&["-i"]);
        assert_eq!(ssh_target(&b), None, "a value with no word after it");
        let c = words(&["-", "h"]);
        assert_eq!(
            ssh_target(&c),
            Some(("-", &c[1..])),
            "a lone dash is a word"
        );
        let d = words(&["--"]);
        assert_eq!(ssh_target(&d), None);
        let e = words(&["--", "h", "ls"]);
        assert_eq!(ssh_target(&e), Some(("h", &e[2..])));
        let f = words(&["h", "--", "ls"]);
        assert_eq!(ssh_target(&f), Some(("h", &f[2..])));
    }

    #[test]
    fn a_destination_is_reduced_to_its_host() {
        assert_eq!(host_of("Lundra"), "lundra");
        assert_eq!(host_of("u@h"), "h");
        assert_eq!(host_of("ssh://u@h:2222"), "h");
        assert_eq!(host_of("ssh://h/x"), "h");
        assert_eq!(host_of("h:22"), "h:22", "only the URI form carries a port");
        let hosts = cluster();
        assert_eq!(declared(&hosts, "100.122.86.48".into()).unwrap().id, "imac");
        assert!(declared(&hosts, "lundrax".into()).is_none());
    }

    #[test]
    fn only_a_build_program_is_a_build() {
        assert!(is_build("cargo", &[]));
        assert!(is_build("cargo", &words(&["-V", "build"])));
        assert!(!is_build("cargo", &words(&["-V"])));
        assert!(is_build("/usr/bin/rustc", &[]));
        assert!(is_build("x/scripts/rust-coverage", &[]));
        assert!(!is_build("derive", &[]), "a bare name is not the script");
        assert!(!is_build("scripts/ci/derive", &[]));
        assert!(!is_build("scripts/other", &[]));
        assert!(
            build_on("cargo", &[], None).is_none(),
            "nothing runs on a host here"
        );
    }
}
