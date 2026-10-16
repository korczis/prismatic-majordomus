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

/// The fixture both halves read. Case 504 requires this declaration, spelled exactly, and
/// the read of it in `samples`, so that the shell and the Rust halves cannot drift onto
/// different data.
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

/// A review's subject, body, verdict and notes are free text another session types, and the
/// mesh signs them into the journal and replicates them in clear (I2708). Every credential
/// shape the fixture states as redacted, written into each of them, reaches the journal on
/// disk and the folded state as its marker, and its body appears in neither; written as the
/// verdict, it is refused before it is signed.
#[test]
fn a_review_is_journaled_with_the_marker_never_the_secret() {
    use std::sync::Arc;

    use majordomus_cli::mesh::config::{CooperationConfig, TrustConfig};
    use majordomus_cli::mesh::cooperation::{CheckoutFacts, Cooperation, CooperationSetup};
    use majordomus_cli::mesh::identity::NodeIdentity;
    use majordomus_cli::mesh::link::HttpTransport;
    use majordomus_cli::mesh::registry::MeshRegistry;
    use majordomus_cli::mesh::repository::of_root_commits;

    let dir = tempfile::tempdir().unwrap();
    let journal = dir.path().join("journal");
    let cooperation = Cooperation::new(CooperationSetup {
        identity: Arc::new(NodeIdentity::ephemeral().unwrap()),
        runtime: "0000000000000001".into(),
        repository: of_root_commits(&["root".into()]),
        endpoints: vec!["127.0.0.1:9".into()],
        version: "test".into(),
        config: CooperationConfig::default(),
        trust: TrustConfig::default(),
        journal_path: Some(journal.clone()),
        registry: Arc::new(MeshRegistry::new()),
        transport: Arc::new(HttpTransport),
        board: None,
        checkout: CheckoutFacts::default(),
    })
    .unwrap();

    // a line that is already a marker has no secret to keep out
    let secrets: Vec<Sample> = samples()
        .into_iter()
        .filter(|s| s.redacted && !s.prefix.starts_with("[redacted:"))
        .collect();
    assert!(!secrets.is_empty(), "the fixture states credentials");
    for s in &secrets {
        let line = format!("{}{}", s.prefix, s.body);
        let asked = cooperation
            .request_review("s1", line.clone(), vec![], None, None, Some(line.clone()))
            .unwrap();
        // a verdict is a word of a fixed vocabulary: a credential there is refused before
        // anything is signed, and the refusal names the marker, not the secret
        let refused = cooperation
            .answer_review(&asked.key, "s2", line.clone(), None)
            .expect_err("a credential is not a verdict");
        assert!(
            !refused.to_string().contains(&s.body),
            "the refusal of a {} verdict repeats it",
            s.name
        );
        cooperation
            .answer_review(&asked.key, "s2", "commented".into(), Some(line.clone()))
            .unwrap();
        cooperation
            .withdraw_review(&asked.key, "s1", Some(line.clone()))
            .unwrap();
    }

    let written = std::fs::read_to_string(&journal).expect("the journal is on disk");
    let state = serde_json::to_string(&cooperation.state()).unwrap();
    let mut leaked = Vec::new();
    for s in &secrets {
        let marker = format!("[redacted:{}]", s.name);
        for (place, text) in [("journal", &written), ("state", &state)] {
            if text.contains(&s.body) {
                leaked.push(format!("{place} carries the body of a {} line", s.name));
            }
            if !text.contains(&marker) {
                leaked.push(format!("{place} carries no {marker}"));
            }
        }
    }
    assert!(leaked.is_empty(), "{}", leaked.join("\n"));

    // text with nothing credential-shaped in it is signed as written
    let plain = cooperation
        .request_review("s1", "feature/mesh-cooperation".into(), vec![], None, None, None)
        .unwrap();
    cooperation
        .answer_review(&plain.key, "s2", "approved".into(), None)
        .unwrap();
    let review = cooperation
        .state()
        .reviews
        .into_iter()
        .find(|r| r.key == plain.key)
        .unwrap();
    assert_eq!(review.subject, "feature/mesh-cooperation");
    assert_eq!(review.answers[0].verdict, "approved");
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

#[test]
fn the_kinds_are_listed_in_byte_order_not_in_the_order_the_table_applied_them() {
    // the table applies github-pat before bearer-token, and openai-key before the
    // assignment rule; `mj_capture_redacted_kinds` lists them as `LC_ALL=C sort -u` does
    let pat = format!("{}{}", "github_pat_", "u".repeat(24));
    let bearer = format!("{}{}", "Bearer ", "t".repeat(24));
    let out = redact_secrets(&format!("{pat} {bearer}"));
    assert_eq!(out.text, "[redacted:github-pat] [redacted:bearer-token]");
    assert_eq!(out.kinds, ["bearer-token", "github-pat"]);

    let key = format!("{}{}", "sk-", "v".repeat(24));
    let out = redact_secrets(&format!("{key} PASSWORD={}", "w".repeat(16)));
    assert_eq!(
        out.text,
        "[redacted:openai-key] PASSWORD=[redacted:assignment]"
    );
    assert_eq!(out.kinds, ["assignment", "openai-key"]);
}

/// A refusal names the shape it refused on and repeats none of what it kept back.
fn refused_on(text: &str, root: &Path, kind: &str, withheld: &str) {
    let refused = public_text(text, root, None).expect_err("the text is refused");
    assert!(refused.contains(&format!("`{kind}`")), "{refused}");
    assert!(
        !refused.contains(withheld),
        "the refusal repeats what it withheld: {refused}"
    );
}

#[test]
fn a_private_key_is_refused_whole_though_its_header_alone_is_redacted() {
    // the shape matches the header; the key's body follows it, and nothing matches that
    let dir = tempfile::tempdir().unwrap();
    let body = ["M".repeat(64), "N".repeat(64), "P".repeat(24)].join("\n");
    let pem = pem(&body);
    assert_eq!(redact_secrets(&pem).kinds, ["private-key-header"]);
    refused_on(&pem, dir.path(), "private-key-header", &"M".repeat(64));
}

#[test]
fn a_credentials_file_is_refused_whole_though_its_key_id_alone_is_redacted() {
    // the key id is one half of a pair, and no shape matches the secret access key
    let dir = tempfile::tempdir().unwrap();
    let secret = format!("{}/{}", "k".repeat(20), "K".repeat(19));
    let file = format!(
        "[default]\naws_access_key_id = {}{}\naws_secret_access_key = {secret}\n",
        "AKIA",
        "Q".repeat(16)
    );
    assert_eq!(redact_secrets(&file).kinds, ["aws-access-key-id"]);
    refused_on(&file, dir.path(), "aws-access-key-id", &secret);
}

/// A PEM private key, split so that no committed file carries its armour whole.
fn pem(body: &str) -> String {
    format!(
        "{}{}\n{body}\n{}{}",
        "-----BEGIN RSA ", "PRIVATE KEY-----", "-----END RSA ", "PRIVATE KEY-----"
    )
}

#[test]
fn text_redacted_once_is_refused_as_the_original_would_be() {
    // the prompt archive stores text redacted once; publishing it must not take the
    // marker for a credential already dealt with
    let dir = tempfile::tempdir().unwrap();
    let body = ["M".repeat(64), "N".repeat(64)].join("\n");
    let once = redact_secrets(&pem(&body)).text;
    assert_eq!(redact_secrets(&once).kinds, ["private-key-header"]);
    refused_on(&once, dir.path(), "private-key-header", &body);

    // the marker alone decides: no footer, no other announcement left in the text
    let headed = format!("{}{}\n{body}", "-----BEGIN ", "PRIVATE KEY-----");
    let once = redact_secrets(&headed).text;
    assert_eq!(once, format!("[redacted:private-key-header]\n{body}"));
    refused_on(&once, dir.path(), "private-key-header", &body);

    let secret = format!("{}/{}", "k".repeat(20), "K".repeat(19));
    let file = format!(
        "aws_access_key_id = {}{}\naws_secret_access_key = {secret}\n",
        "AKIA",
        "Q".repeat(16)
    );
    let once = redact_secrets(&file).text;
    refused_on(&once, dir.path(), "aws-access-key-id", &secret);
    // an id marked once and a secret no rule knows the name of
    let bare = format!("[redacted:aws-access-key-id] {secret}");
    refused_on(&bare, dir.path(), "aws-access-key-id", &secret);
}

#[test]
fn a_private_key_s_tail_is_refused_though_its_header_was_cut_off() {
    // a failure's tail keeps the end of what was printed, which is the footer
    let dir = tempfile::tempdir().unwrap();
    let body = ["N".repeat(64), "P".repeat(24)].join("\n");
    let tail = format!("{body}\n{}{}", "-----END RSA ", "PRIVATE KEY-----");
    assert!(redact_secrets(&tail).kinds.is_empty());
    refused_on(&tail, dir.path(), "PRIVATE KEY-----", &body);

    let pgp = format!("{body}\n{}{}", "-----END PGP ", "PRIVATE KEY BLOCK-----");
    refused_on(&pgp, dir.path(), "PRIVATE KEY BLOCK-----", &body);

    // a public key's footer announces nothing secret
    let public = format!("{body}\n{}{}", "-----END ", "PUBLIC KEY-----");
    assert_eq!(public_text(&public, dir.path(), None).unwrap().text, public);
}

#[test]
fn a_secret_access_key_is_refused_without_the_id_it_pairs_with() {
    // the tail of an environment dump, or a role's temporary credentials, with no key id
    let dir = tempfile::tempdir().unwrap();
    let secret = format!("{}+{}", "s".repeat(20), "S".repeat(19));
    for text in [
        format!("AWS_REGION=eu-west-1\nAWS_SECRET_ACCESS_KEY={secret}\n"),
        format!("{{\"Credentials\": {{\"SecretAccessKey\": \"{secret}\"}}}}"),
    ] {
        assert!(redact_secrets(&text).kinds.is_empty(), "{text}");
        refused_on(&text, dir.path(), "secret-access-key", &secret);
    }
    // naming the variable is not assigning it
    let reason = "skipped: AWS_SECRET_ACCESS_KEY is not set";
    assert_eq!(public_text(reason, dir.path(), None).unwrap().text, reason);
}

/// A credential-shaped value in any tracing field is absent from the log (I2708). Every
/// line of the fixture the redactor takes out is logged to a `server.log` through the
/// subscriber `logging::init` installs: as a span's field, as a string field, as a displayed
/// field, as a debugged field and inside the message. None of them is in the file, and each
/// is there as its marker, so the fields were written redacted rather than not at all.
#[test]
fn a_tracing_field_never_writes_a_credential() {
    use majordomus_cli::logging;
    use tracing_subscriber::EnvFilter;

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("server.log");
    let file = std::sync::Arc::new(std::fs::File::create(&path).unwrap());
    // a line that is already its own redaction (a marker the fixture feeds back) is not a
    // credential, so it is not one the log could leak
    let lines: Vec<(String, String)> = samples()
        .into_iter()
        .filter(|s| s.redacted)
        .map(|s| {
            let line = format!("{}{}", s.prefix, s.body);
            let marked = redact_secrets(&line).text;
            (line, marked)
        })
        .filter(|(line, marked)| line != marked)
        .collect();
    assert!(lines.len() > 10, "the fixture lost its redacted lines");

    let subscriber = logging::subscriber(EnvFilter::new("trace"), file);
    tracing::subscriber::with_default(subscriber, || {
        for (line, _) in &lines {
            let span = tracing::info_span!("request", origin = line.as_str());
            let _inside = span.enter();
            tracing::warn!(
                value = line.as_str(),
                shown = %line,
                debugged = ?line,
                "refused {line}"
            );
        }
    });

    let log = std::fs::read_to_string(&path).unwrap();
    let mut leaked = Vec::new();
    for (line, marked) in &lines {
        if log.contains(line.as_str()) || log.contains(&format!("{line:?}")) {
            leaked.push(line.clone());
        }
        assert!(
            log.contains(&format!("refused {marked}")),
            "the message was not written as {marked:?}:\n{log}"
        );
        assert!(
            log.contains(&format!("origin={marked:?}")),
            "the span's field was not written as {marked:?}:\n{log}"
        );
    }
    assert!(
        leaked.is_empty(),
        "{} credential(s) reached the log:\n{log}",
        leaked.len()
    );
}
