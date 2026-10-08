//! The avahi backend of the Bonjour provider: the system's DNS-SD service on Linux,
//! reached through the two tools avahi ships, as child processes.
//!
//! `avahi-publish -s <name> <type> <port> <txt>…` holds a registration for as long as it
//! runs, and `avahi-browse -rpk <type>` prints one parseable line per resolved instance
//! and keeps running. avahi has no tool that replaces a TXT record in place, so a new
//! envelope is a new publisher: the old child is ended and reaped, the new one started,
//! and every browser sees the instance leave and return. That costs one goodbye, one
//! probe and one announcement per interval, and roughly a second in which the instance is
//! not registered.
//!
//! `avahi-browse` resolves an instance when it appears and never again, and an instance
//! that left and returned within a second does not always appear to have left: measured
//! on avahi 0.8, a browser that stayed saw the republished TXT record in one run of
//! three. So the browser is replaced too, every [`REBROWSE_AFTER`]: a new one resolves
//! every instance from what the daemon holds now.
//!
//! Every child is owned here and reaped here: replacing the publisher, an end of
//! browsing, and dropping the value each kill and wait.
//!
//! The parser reads what avahi-browse 0.8 printed on a live daemon (Debian bookworm, in a
//! container; the test's recorded line is that output): a record's strings come last
//! first, and a `"` or `\\` inside one is written with a `\\` before it. A line it cannot
//! read yields no instance, and an envelope it reassembled wrongly is refused by its
//! signature.

use std::ffi::{OsStr, OsString};
use std::io::{BufRead, BufReader};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use super::bonjour::{Advertised, DnsSd, Found};

/// How long one `avahi-browse` is kept before a new one takes its place. Well inside the
/// minute an advertised presence lasts, and long enough that the browser has printed
/// what the daemon knows.
const REBROWSE_AFTER: Duration = Duration::from_secs(10);

/// avahi's tools, and the children this value runs.
pub(crate) struct Avahi {
    publish: PathBuf,
    browse: PathBuf,
    publisher: Option<Child>,
    browser: Option<Child>,
    lines: Option<Receiver<Vec<u8>>>,
    /// The service type being browsed, and since when by the browser that runs now.
    browsing: Option<(String, Instant)>,
    rebrowse_after: Duration,
}

impl Avahi {
    /// The backend over two explicit programs.
    pub(crate) fn new(publish: PathBuf, browse: PathBuf) -> Self {
        Avahi {
            publish,
            browse,
            publisher: None,
            browser: None,
            lines: None,
            browsing: None,
            rebrowse_after: REBROWSE_AFTER,
        }
    }

    /// The backend over `avahi-publish` and `avahi-browse` as found on a `PATH`, or the
    /// reason there is none: a machine without avahi's tools has no Bonjour, and says so.
    pub(crate) fn on_path(path: Option<OsString>) -> Result<Self, String> {
        let find = |program: &str| {
            std::env::split_paths(&path.clone().unwrap_or_default())
                .map(|dir| dir.join(program))
                .find(|candidate| candidate.is_file())
        };
        match (find("avahi-publish"), find("avahi-browse")) {
            (Some(publish), Some(browse)) => Ok(Avahi::new(publish, browse)),
            _ => Err("avahi-publish and avahi-browse are not on PATH (install avahi-utils and run avahi-daemon)".into()),
        }
    }

    /// The process ids of the running children, publisher first: what a test reaps by.
    #[cfg(test)]
    fn children(&self) -> Vec<u32> {
        [&self.publisher, &self.browser]
            .into_iter()
            .flatten()
            .map(Child::id)
            .collect()
    }
}

/// End a child and collect it, so that neither a process nor a zombie is left.
fn reap(child: &mut Option<Child>) {
    if let Some(mut child) = child.take() {
        let _ = child.kill();
        let _ = child.wait();
    }
}

fn spawn(program: &Path, args: &[&OsStr], stdout: Stdio) -> Result<Child, String> {
    Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(stdout)
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("{}: {e}", program.display()))
}

impl DnsSd for Avahi {
    fn mechanism(&self) -> &'static str {
        "avahi"
    }

    fn publish(&mut self, advertised: &Advertised) -> Result<(), String> {
        reap(&mut self.publisher);
        let port = advertised.port.to_string();
        let mut args: Vec<&OsStr> = vec![
            OsStr::new("-s"),
            OsStr::new(&advertised.name),
            OsStr::new(&advertised.service),
            OsStr::new(&port),
        ];
        args.extend(advertised.txt.iter().map(|s| OsStr::from_bytes(s)));
        self.publisher = Some(spawn(&self.publish, &args, Stdio::null())?);
        Ok(())
    }

    fn browse(&mut self, service: &str) -> Result<(), String> {
        reap(&mut self.browser);
        let mut child = spawn(
            &self.browse,
            &[OsStr::new("-rpk"), OsStr::new(service)],
            Stdio::piped(),
        )?;
        let (tx, rx) = channel();
        // `Stdio::piped()` above is what makes this handle exist.
        let stdout = child.stdout.take().expect("the browser's piped output");
        // The reader ends with the child's output, which ends with the child.
        let _ = std::thread::Builder::new()
            .name("majordomus-mesh-bonjour-avahi".into())
            .spawn(move || {
                let mut reader = BufReader::new(stdout);
                let mut line = Vec::new();
                while matches!(reader.read_until(b'\n', &mut line), Ok(n) if n > 0) {
                    let _ = tx.send(std::mem::take(&mut line));
                }
            });
        self.browser = Some(child);
        self.lines = Some(rx);
        self.browsing = Some((service.to_string(), Instant::now()));
        Ok(())
    }

    fn poll(&mut self, wait: Duration) -> Result<Vec<Found>, String> {
        if let Some((service, _)) = self
            .browsing
            .take_if(|(_, since)| since.elapsed() >= self.rebrowse_after)
        {
            self.browse(&service)?;
        }
        let Some(lines) = &self.lines else {
            return Err("not browsing".into());
        };
        let mut found = Vec::new();
        match lines.recv_timeout(wait) {
            Ok(line) => found.extend(resolved(&line)),
            Err(RecvTimeoutError::Timeout) => return Ok(found),
            Err(RecvTimeoutError::Disconnected) => {
                reap(&mut self.browser);
                reap(&mut self.publisher);
                self.lines = None;
                self.browsing = None;
                return Err("avahi-browse ended (is avahi-daemon running?)".into());
            }
        }
        // whatever else has already arrived, without waiting again
        found.extend(lines.try_iter().filter_map(|line| resolved(&line)));
        Ok(found)
    }
}

impl Drop for Avahi {
    fn drop(&mut self) {
        reap(&mut self.publisher);
        reap(&mut self.browser);
    }
}

/// One line of `avahi-browse -rp`, when it is a resolved instance:
/// `=;<interface>;<protocol>;<name>;<type>;<domain>;<host>;<address>;<port>;<txt>`. The
/// name has `\DDD` for every byte avahi escapes. The TXT field is the record's strings,
/// each between double quotes with one space between two, a `"` or a `\` inside a string
/// written with a `\` before it — and last first: avahi keeps a record's strings in a
/// list it prepends to. They are handed on in the order printed; the envelope is put
/// together by index. A field that is not quoted strings to its end is no instance.
pub(crate) fn resolved(line: &[u8]) -> Option<Found> {
    let line = line.strip_suffix(b"\n").unwrap_or(line);
    let fields: Vec<&[u8]> = line.splitn(10, |b| *b == b';').collect();
    if fields.len() != 10 || fields[0] != b"=" {
        return None;
    }
    let text = |field: &[u8]| String::from_utf8_lossy(&unescaped(field)).into_owned();
    let (interface, protocol) = (text(fields[1]), text(fields[2]));
    let (name, service, domain) = (text(fields[3]), text(fields[4]), text(fields[5]));
    Some(Found {
        name: format!("{name}.{service}.{domain} ({interface} {protocol})"),
        txt: quoted_strings(fields[9])?,
    })
}

/// The strings of a TXT field as `avahi_string_list_to_string` prints them: `"…" "…"`,
/// with `\"` for a quote and `\\` for a backslash inside one, and `\DDD` for a byte it
/// would not print. `None` when the field is empty or is not that from end to end.
fn quoted_strings(field: &[u8]) -> Option<Vec<Vec<u8>>> {
    let mut strings = Vec::new();
    let mut i = 0;
    loop {
        if field.get(i) != Some(&b'"') {
            return None;
        }
        i += 1;
        let mut string = Vec::new();
        loop {
            match *field.get(i)? {
                b'"' => break,
                b'\\' => {
                    let digits = field
                        .get(i + 1..i + 4)
                        .filter(|d| d.iter().all(u8::is_ascii_digit))
                        .map(|d| d.iter().fold(0u16, |n, d| n * 10 + u16::from(d - b'0')))
                        .and_then(|value| u8::try_from(value).ok());
                    match digits {
                        Some(byte) => {
                            string.push(byte);
                            i += 4;
                        }
                        None => {
                            string.push(*field.get(i + 1)?);
                            i += 2;
                        }
                    }
                }
                byte => {
                    string.push(byte);
                    i += 1;
                }
            }
        }
        strings.push(string);
        i += 1;
        match field.get(i..) {
            Some([]) => return Some(strings),
            Some([b' ', ..]) => i += 1,
            _ => return None,
        }
    }
}

/// avahi's `\DDD` escapes (three decimal digits for one byte) taken back out of a field.
fn unescaped(field: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(field.len());
    let mut i = 0;
    while i < field.len() {
        let byte = field
            .get(i + 1..i + 4)
            .filter(|d| field[i] == b'\\' && d.iter().all(u8::is_ascii_digit))
            .map(|d| d.iter().fold(0u16, |n, d| n * 10 + u16::from(d - b'0')))
            .and_then(|value| u8::try_from(value).ok());
        match byte {
            Some(byte) => {
                out.push(byte);
                i += 4;
            }
            None => {
                out.push(field[i]);
                i += 1;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::bonjour::{envelope_of, txt_of};
    use std::os::unix::fs::PermissionsExt;

    /// What `avahi-browse -rpk _mjt._tcp` 0.8 printed for an instance published with
    /// `avahi-publish -s mjprobe _mjt._tcp 4242 txtvers=1 n=2 e0=… e1=…` on a live daemon,
    /// the run of `q` in `e1` shortened from 240. The `+`, `-` and stranger lines beside it
    /// are in the same format and were not captured.
    const RECORDED: &[u8] = br#"+;eth0;IPv4;mjprobe;_mjt._tcp;local
+;eth0;IPv4;Office\032Printer;_mjt._tcp;local
=;eth0;IPv4;mjprobe;_mjt._tcp;local;611d6621594c.local;172.17.0.3;4242;"e1=qqqq\"tail" "e0={\"v\":1,\"k\":\"a\\\\b\",\"s\":\"x y;z\"}" "n=2" "txtvers=1"
=;eth0;IPv4;Office\032Printer;_mjt._tcp;local;printer.local;192.168.1.9;631;"rp=ipp/print"
-;eth0;IPv4;Office\032Printer;_mjt._tcp;local
=;eth0;IPv6;bare;_mjt._tcp;local;bare.local;fe80::1;9;
"#;

    #[test]
    fn a_resolved_line_gives_the_instance_and_its_txt_strings_as_they_were_published() {
        let found: Vec<Found> = RECORDED
            .split_inclusive(|b| *b == b'\n')
            .filter_map(resolved)
            .collect();
        assert_eq!(found.len(), 2, "two resolved lines carry a TXT field");
        assert_eq!(found[0].name, "mjprobe._mjt._tcp.local (eth0 IPv4)");
        // last first, as avahi prints them, each with its escapes taken out
        assert_eq!(
            found[0].txt,
            [
                &br#"e1=qqqq"tail"#[..],
                br#"e0={"v":1,"k":"a\\b","s":"x y;z"}"#,
                b"n=2",
                b"txtvers=1",
            ]
        );
        // and the envelope is the published one: quotes, backslashes, a space and a
        // semicolon inside it survive, and the order they arrived in does not matter
        assert_eq!(
            envelope_of(&found[0].txt).unwrap(),
            br#"{"v":1,"k":"a\\b","s":"x y;z"}qqqq"tail"#
        );
        // the name's escapes are taken out; a stranger's TXT record is simply no envelope
        assert_eq!(found[1].name, "Office Printer._mjt._tcp.local (eth0 IPv4)");
        assert!(envelope_of(&found[1].txt).is_err());
    }

    #[test]
    fn a_line_that_is_not_a_resolved_instance_gives_nothing() {
        for line in [
            &b""[..],
            b"\n",
            b"=",
            b"=;eth0;IPv4;name;_majordomus._tcp;local;host;addr;1",
            b"=;eth0;IPv4;name;_majordomus._tcp;local;host;addr;1;",
            b"=;eth0;IPv4;name;_majordomus._tcp;local;host;addr;1;\"",
            b"=;eth0;IPv4;name;_majordomus._tcp;local;host;addr;1;no quotes",
            // a string that never closes, a lone escape at the end, and text after a string
            b"=;eth0;IPv4;name;_majordomus._tcp;local;host;addr;1;\"a=1\" \"b",
            b"=;eth0;IPv4;name;_majordomus._tcp;local;host;addr;1;\"a=1\\",
            b"=;eth0;IPv4;name;_majordomus._tcp;local;host;addr;1;\"a=1\"x",
            b"=;eth0;IPv4;name;_majordomus._tcp;local;host;addr;1;\"a=1\" ",
            b"-;eth0;IPv4;name;_majordomus._tcp;local",
            b"Failed to create client object: Daemon not running",
        ] {
            assert!(
                resolved(line).is_none(),
                "{}",
                String::from_utf8_lossy(line)
            );
        }
        // an escape is three digits that name a byte; anything else is itself
        assert_eq!(unescaped(br"a\032b\\999\03"), br"a b\\999\03");
        // inside a TXT string a `\DDD` is a byte too, and any other escaped byte is itself
        assert_eq!(
            quoted_strings(br#""a\032b" "c\\\"d""#).unwrap(),
            [&b"a b"[..], br#"c\"d"#]
        );
    }

    /// A stand-in for one of avahi's tools: a script that records what it was given or
    /// prints what it was told to, and then stays, as the real tool does.
    fn tool(dir: &Path, name: &str, body: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        // Linux refuses to exec a file a forked sibling still holds open for writing; one
        // exec that is not refused proves no writer is left (worktree::direnv's tests).
        for _ in 0..100 {
            match Command::new(&path).arg("--probe").output() {
                Err(e) if e.raw_os_error() == Some(libc::ETXTBSY) => {
                    std::thread::sleep(Duration::from_millis(20))
                }
                _ => break,
            }
        }
        path
    }

    fn tools(dir: &Path, lines: &[u8]) -> Avahi {
        std::fs::write(dir.join("lines"), lines).unwrap();
        let publish = tool(
            dir,
            "avahi-publish",
            "[ \"$1\" = --probe ] && exit 0\nfor a in \"$@\"; do printf '%s\\n' \"$a\"; done > \"$0.tmp\"\nmv \"$0.tmp\" \"$0.args\"\nexec sleep 60",
        );
        let browse = tool(
            dir,
            "avahi-browse",
            "[ \"$1\" = --probe ] && exit 0\nprintf '%s\\n' \"$@\" > \"$0.tmp\"\nmv \"$0.tmp\" \"$0.args\"\ncat \"$(dirname \"$0\")/lines\"\nexec sleep 60",
        );
        Avahi::new(publish, browse)
    }

    /// Whether a process is gone: reaped by this process, so that signalling it finds
    /// nothing.
    fn gone(pid: u32) -> bool {
        // SAFETY: signal 0 delivers nothing; it only asks whether the process exists.
        unsafe { libc::kill(pid as libc::pid_t, 0) != 0 }
    }

    fn wait_for_file(path: &Path) -> String {
        for _ in 0..400 {
            match std::fs::read_to_string(path) {
                Ok(text) if text.ends_with('\n') => return text,
                _ => std::thread::sleep(Duration::from_millis(10)),
            }
        }
        panic!("{} was never written", path.display());
    }

    #[test]
    fn the_tools_are_found_on_a_path_or_their_absence_is_the_reason() {
        let dir = tempfile::tempdir().unwrap();
        let empty = Avahi::on_path(Some(dir.path().as_os_str().to_owned()));
        assert!(empty.err().unwrap().contains("not on PATH"));
        assert!(Avahi::on_path(None).is_err(), "no PATH, no tools");
        let _ = tools(dir.path(), b"");
        let found = Avahi::on_path(Some(dir.path().as_os_str().to_owned())).unwrap();
        assert_eq!(found.mechanism(), "avahi");
        assert_eq!(found.publish, dir.path().join("avahi-publish"));
        assert!(
            found.children().is_empty(),
            "finding the tools runs neither"
        );
    }

    #[test]
    fn publishing_runs_one_publisher_and_browsing_reads_what_the_browser_prints() {
        let dir = tempfile::tempdir().unwrap();
        let mut avahi = tools(dir.path(), RECORDED);
        assert_eq!(
            avahi.poll(Duration::from_millis(1)).unwrap_err(),
            "not browsing"
        );

        // bytes beyond ASCII and quotes inside: an argument is given as it is, never quoted
        let envelope = r#"{"v":2,"name":"café \"x\"","sig":"00"}"#.as_bytes();
        let advertised = Advertised {
            name: "majordomus-641bdb94-01234567".into(),
            service: "_majordomus._tcp".into(),
            port: 8741,
            txt: txt_of(envelope).unwrap(),
        };
        avahi.publish(&advertised).unwrap();
        let first = avahi.children()[0];
        // the tool was given the name, the type, the port and every TXT string, as they are
        let args = wait_for_file(&dir.path().join("avahi-publish.args"));
        assert_eq!(
            args,
            format!(
                "-s\nmajordomus-641bdb94-01234567\n_majordomus._tcp\n8741\ntxtvers=1\nn=1\ne0={}\n",
                String::from_utf8_lossy(envelope)
            )
        );
        // a second publication replaces the first: one publisher, and the old one reaped
        avahi.publish(&advertised).unwrap();
        let second = avahi.children()[0];
        assert_ne!(first, second);
        assert!(gone(first), "the replaced publisher was reaped");

        avahi.browse("_majordomus._tcp").unwrap();
        assert_eq!(
            wait_for_file(&dir.path().join("avahi-browse.args")),
            "-rpk\n_majordomus._tcp\n"
        );
        let mut found = Vec::new();
        for _ in 0..200 {
            found.extend(avahi.poll(Duration::from_millis(20)).unwrap());
            if found.len() == 2 {
                break;
            }
        }
        assert_eq!(found.len(), 2, "{found:?}");
        assert!(envelope_of(&found[0].txt).is_ok());
        // nothing more arrives, and waiting for it is bounded
        assert!(avahi.poll(Duration::from_millis(20)).unwrap().is_empty());

        let children = avahi.children();
        assert_eq!(children.len(), 2);
        drop(avahi);
        assert!(children.into_iter().all(gone), "the drop reaps every child");
    }

    #[test]
    fn a_browser_is_replaced_when_it_has_run_its_time_and_the_instances_are_read_again() {
        let dir = tempfile::tempdir().unwrap();
        let mut avahi = tools(dir.path(), RECORDED);
        avahi.rebrowse_after = Duration::from_millis(200);
        avahi.browse("_mjt._tcp").unwrap();
        let first = avahi.children()[0];
        let read = |avahi: &mut Avahi| {
            let mut found = Vec::new();
            for _ in 0..200 {
                found.extend(avahi.poll(Duration::from_millis(25)).unwrap());
                if found.len() >= 2 {
                    break;
                }
            }
            found.len()
        };
        assert_eq!(read(&mut avahi), 2, "the first browser's two instances");
        assert_eq!(avahi.children(), [first], "inside its time it stays");
        std::thread::sleep(Duration::from_millis(250));
        // the next poll replaces it: the old child is ended and reaped, and what the new
        // one prints is read as if for the first time
        assert_eq!(read(&mut avahi), 2, "the second browser's two instances");
        let second = avahi.children()[0];
        assert_ne!(first, second);
        assert!(gone(first), "the replaced browser is left running");
        drop(avahi);
        assert!(gone(second));
    }

    #[test]
    fn a_browser_that_ends_ends_browsing_and_takes_the_publisher_with_it() {
        let dir = tempfile::tempdir().unwrap();
        let mut avahi = tools(dir.path(), b"");
        // a browser that prints its complaint and exits, as one without a daemon does
        tool(
            dir.path(),
            "avahi-browse",
            "[ \"$1\" = --probe ] && exit 0\necho 'Failed to create client object: Daemon not running'\nexit 1",
        );
        avahi
            .publish(&Advertised {
                name: "n".into(),
                service: "_majordomus._tcp".into(),
                port: 1,
                txt: vec![],
            })
            .unwrap();
        let publisher = avahi.children()[0];
        avahi.browse("_majordomus._tcp").unwrap();
        let mut ended = None;
        for _ in 0..200 {
            match avahi.poll(Duration::from_millis(20)) {
                Ok(found) => assert!(found.is_empty()),
                Err(e) => {
                    ended = Some(e);
                    break;
                }
            }
        }
        assert!(ended.unwrap().starts_with("avahi-browse ended"));
        assert!(gone(publisher) && avahi.children().is_empty());
        assert_eq!(
            avahi.poll(Duration::from_millis(1)).unwrap_err(),
            "not browsing"
        );

        // and a tool that cannot be run at all is an error that names it
        let mut missing = Avahi::new(
            "/nonexistent/avahi-publish".into(),
            "/nonexistent/avahi-browse".into(),
        );
        assert!(missing
            .browse("_majordomus._tcp")
            .unwrap_err()
            .starts_with("/nonexistent/avahi-browse: "));
        let nothing = Advertised {
            name: "n".into(),
            service: "_majordomus._tcp".into(),
            port: 1,
            txt: vec![],
        };
        assert!(missing
            .publish(&nothing)
            .unwrap_err()
            .starts_with("/nonexistent/avahi-publish: "));
        assert!(missing.children().is_empty());
    }
}
