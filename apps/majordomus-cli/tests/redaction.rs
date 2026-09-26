//! The one text redactor, held equal to the shell table it ports.
//!
//! Two implementations of one credential table: `lib/capture.sh`, which the prompt archive
//! redacts with, and `majordomus_cli::redaction`, which published evidence goes through. They
//! are held equal twice. The table's shapes are read out of `lib/capture.sh` and compared,
//! name for name and in order, with the port's. And one fixture,
//! `test/fixtures/redaction/shapes.tsv`, is run through the port here and through the shell
//! by test case 504, each against the expectation its line states.
//!
//! No credential-shaped string is written in this file or in the fixture: every sample is
//! assembled from a prefix and a body at run time, and every machine path is made here too,
//! from a temporary directory, so that no committed file names one.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use majordomus_cli::redaction::{
    normalise_machine_paths, public_text, redact_secrets, shape_names,
};

/// The repository this crate belongs to.
fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The fixture both halves read. Case 504 greps this file for the path, so that the shell
/// and the Rust halves cannot drift onto different data.
const FIXTURE: &str = "test/fixtures/redaction/shapes.tsv";

/// One line of the fixture.
struct Sample {
    name: String,
    prefix: String,
    body: String,
    redacted: bool,
}

fn samples() -> Vec<Sample> {
    let text = std::fs::read_to_string(repository().join(FIXTURE)).expect("the fixture exists");
    let mut lines = text.lines();
    assert_eq!(
        lines.next(),
        Some("name\tprefix\tbody\texpect"),
        "the header"
    );
    lines
        .map(|line| {
            let columns: Vec<&str> = line.split('\t').collect();
            let [name, prefix, body, expect] = columns[..] else {
                panic!("four tab-separated columns: {line:?}");
            };
            assert!(
                ![name, prefix, body].contains(&""),
                "an empty column: {line:?}"
            );
            Sample {
                name: name.to_string(),
                prefix: prefix.to_string(),
                body: body.to_string(),
                redacted: match expect {
                    "redacted" => true,
                    "kept" => false,
                    other => panic!("an expectation is redacted or kept, not {other:?}"),
                },
            }
        })
        .collect()
}

#[test]
fn the_shapes_are_lib_capture_sh_s_table_in_its_order() {
    let source = std::fs::read_to_string(repository().join("lib/capture.sh")).unwrap();
    let opening = "\nMJ_CAPTURE_SECRETS='";
    let start = source
        .find(opening)
        .expect("lib/capture.sh declares MJ_CAPTURE_SECRETS");
    let body = &source[start + opening.len()..];
    let table = &body[..body.find('\'').expect("the table's quote is closed")];
    let shell: Vec<&str> = table
        .lines()
        .map(|row| row.split('\t').next().unwrap())
        .collect();
    assert!(!shell.is_empty(), "no row was read out of the table");

    let names = shape_names();
    let (assignment, ported) = names.split_last().unwrap();
    assert_eq!(
        ported, shell,
        "the port and lib/capture.sh name different shapes"
    );
    assert_eq!(*assignment, "assignment");
}

#[test]
fn every_fixture_line_is_redacted_as_it_states() {
    let samples = samples();
    let names = shape_names();
    let mut wrong = Vec::new();
    for s in &samples {
        assert!(
            names.contains(&s.name.as_str()),
            "{} is not a shape",
            s.name
        );
        let line = format!("{}{}", s.prefix, s.body);
        let out = redact_secrets(&line);
        let (text, kinds): (String, Vec<&str>) = match (s.redacted, s.name.as_str()) {
            // the assignment rule keeps the name and the separator: the whole prefix
            (true, "assignment") => (
                format!("{}[redacted:assignment]", s.prefix),
                vec!["assignment"],
            ),
            (true, name) => (format!("[redacted:{name}]"), vec![name]),
            (false, _) => (line.clone(), vec![]),
        };
        if out.text != text || out.kinds != kinds {
            wrong.push(format!(
                "{} (body of {}): expected {text:?} {kinds:?}, got {:?} {:?}",
                s.name,
                s.body.len(),
                out.text,
                out.kinds
            ));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));

    // every shape is exercised both ways, so a fixture that lost one is not a pass
    let seen: BTreeSet<(&str, bool)> = samples
        .iter()
        .map(|s| (s.name.as_str(), s.redacted))
        .collect();
    for name in names {
        for redacted in [true, false] {
            let expect = if redacted { "redacted" } else { "kept" };
            assert!(
                seen.contains(&(name, redacted)),
                "{name} has no {expect} line"
            );
        }
    }
}

#[test]
fn machine_paths_are_normalised_root_first_then_home() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("account");
    let root = home.join("dev").join("checkout");
    std::fs::create_dir_all(&root).unwrap();
    let canonical = std::fs::canonicalize(&root).unwrap();

    let text = format!(
        "{}/src/lib.rs:4\n{}/Cargo.toml\n{}/.cargo/registry",
        root.display(),
        canonical.display(),
        home.display()
    );
    let out = normalise_machine_paths(&text, &root, Some(&home));
    assert_eq!(
        out,
        "<repo>/src/lib.rs:4\n<repo>/Cargo.toml\n<home>/.cargo/registry"
    );

    // a given spelling with a trailing separator is the same root
    let slashed = PathBuf::from(format!("{}/", root.display()));
    let out = normalise_machine_paths(&format!("{}/x", root.display()), &slashed, None);
    assert_eq!(out, "<repo>/x");
    // no home given, no home replaced
    let away = format!("{}/.profile", home.display());
    assert_eq!(normalise_machine_paths(&away, &root, None), away);
}

#[test]
fn public_text_normalises_redacts_and_names_what_it_refuses() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let key = format!("{}{}", "sk-ant-", "q".repeat(24));
    let text = format!(
        "{}/target/debug: {key}\nexport PASSWORD={}",
        root.display(),
        "r".repeat(16)
    );
    let out = public_text(&text, root, None).expect("normalised and redacted, it is publishable");
    assert_eq!(
        out.text,
        "<repo>/target/debug: [redacted:anthropic-key]\nexport PASSWORD=[redacted:assignment]"
    );
    assert_eq!(out.kinds, ["anthropic-key", "assignment"]);

    // another account's home is a machine path neither rule can make safe
    let elsewhere = format!("cache at {}{}", "/Use", "rs/someone/Library");
    let refused = public_text(&elsewhere, root, None).unwrap_err();
    assert!(refused.contains("`/Users/`"), "{refused}");
    assert!(refused.contains("an absolute path"), "{refused}");

    // a key id too short for its shape is kept by the redactor, and still refused here
    let short = format!("{}{}", "AKIA", "Z".repeat(8));
    let refused = public_text(&short, root, None).unwrap_err();
    assert!(refused.contains("`AKIA`"), "{refused}");
    assert!(
        !refused.contains(&short),
        "the refusal repeats the text it refused: {refused}"
    );
}
