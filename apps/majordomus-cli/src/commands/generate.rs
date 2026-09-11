//! `majordomus generate`: write the committed projections, or with `--check` say whether
//! the committed ones still match the registry. Check mode writes nothing.

use std::io::Write;

use crate::app::App;
use crate::cli::{GenerateArgs, GenerateTarget};
use crate::error::{Error, Result};
use crate::generate::{self, Target};

/// Run `majordomus generate`.
pub fn run(args: GenerateArgs) -> Result<u8> {
    let app = App::load(&args.repo)?;
    // Before anything is planned, and before anything is judged: an executable of another
    // generation rewrites every artifact from a model this tree does not have, and nothing
    // in the bytes says so, so the damage reads as an ordinary regeneration. Where the
    // repository being generated is the one that declares this executable, the two must be
    // the same generation.
    refuse_foreign_generation(&app)?;
    let targets: &[Target] = match args.target {
        GenerateTarget::All => Target::ALL,
        GenerateTarget::Openapi => &[Target::OpenApi],
        GenerateTarget::Docs => &[Target::Docs],
        GenerateTarget::Benchmarks => &[Target::Benchmarks],
        GenerateTarget::Registry => &[Target::Registry],
        GenerateTarget::Allow => &[Target::Allow],
        GenerateTarget::Providers => &[Target::Providers],
        GenerateTarget::Site => &[Target::Site],
        GenerateTarget::Distribution => &[Target::Distribution],
        GenerateTarget::Manifest => &[Target::Manifest],
        GenerateTarget::Web => &[Target::Web],
        GenerateTarget::Changelog => &[Target::Changelog],
        GenerateTarget::Deployment => &[Target::Deployment],
        GenerateTarget::Graph => &[Target::Graph],
        GenerateTarget::Design => &[Target::Design],
    };
    let artifacts = generate::plan(&app, targets)?;
    // the contract before the bytes: an artifact that carries no provenance, declares an
    // encoding its suffix contradicts, or breaks the schema it names is never written and
    // never reported in sync
    let schemas = generate::GeneratedSchemas::load(&app.share.generated_schemas_dir())?;
    generate::verify(&artifacts, &schemas)?;
    let root = args
        .out
        .clone()
        .unwrap_or_else(|| app.repository.root().to_path_buf());
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    if args.check {
        generate::check(&root, &artifacts)?;
        for a in &artifacts {
            writeln!(
                out,
                "OK   generated   {} — {}, matches the registry",
                a.path,
                a.schema.as_deref().unwrap_or(a.format.suffix())
            )
            .map_err(Error::Transport)?;
        }
        writeln!(out, "generate --check: in sync").map_err(Error::Transport)?;
    } else {
        for path in generate::write(&root, &artifacts)? {
            writeln!(
                out,
                "{}",
                path.strip_prefix(&root).unwrap_or(&path).display()
            )
            .map_err(Error::Transport)?;
        }
    }
    Ok(0)
}

/// Refuse when this executable is not of the generation of the tree it was asked to derive.
///
/// The question is only asked where the repository carries *this* crate's manifest: a
/// foreign repository has no opinion about this executable, and generating there is exactly
/// what a released binary is for. A manifest at the same path that declares some other
/// package — a test fixture's crate, a repository that keeps its own tool at that path — is
/// a foreign repository too, whatever it states.
///
/// Where the repository does declare this crate, three things are asked in turn, and any
/// one of them answered is enough to proceed:
///
/// 1. **The version.** Every artifact carries `majordomus-cli <version>` in its provenance
///    header, so a binary of another version restamps all of them with a version the tree
///    does not have. That is the 0.2.0-into-0.3.1 incident, and the cheapest question.
/// 2. **The generation.** Was this executable built from *these* sources? [`crate::GENERATION`]
///    against the same digest taken over the tree's crate. Equal means it is this tree's
///    executable by construction — which is the ordinary case, because `scripts/derive`
///    builds it from the tree it is about to derive, and it stays true when a session edits
///    a capability and rebuilds.
/// 3. **The model.** When the sources differ, the executable may still be interchangeable:
///    a build from a sibling worktree whose crate differs by a comment produces the same
///    artifacts. So the registry it projects is compared against the registry the tree has
///    committed at `docs/generated/registry.json` — its builtin half, since the declarative
///    kinds in that document are the repository's content and not this executable's model.
///    Equal means the same model, observed rather than assumed, and a correctly-matching
///    prebuilt `MAJORDOMUS_BIN` keeps working without the ten-minute rebuild several
///    sessions set it to avoid.
///
/// None of the three is a verdict about the *tree*. If this executable's model disagrees
/// with a tree it was not built from, the tree is not the thing that is out of date, and
/// saying it was is how this became destructive: the operator's next move is to "fix" the
/// tree by running the command that overwrites it. The refusal names the executable, exits
/// [`REFUSED`](crate::error::Error::Refused) rather than the contract-unmet code a stale
/// artifact uses, and writes nothing.
fn refuse_foreign_generation(app: &App) -> Result<()> {
    let crate_dir = app.repository.root().join(CRATE_DIR);
    let manifest = crate_dir.join("Cargo.toml");
    let Ok(text) = std::fs::read_to_string(&manifest) else {
        return Ok(());
    };
    if package_field(&text, "name").as_deref() != Some(CRATE_NAME) {
        return Ok(());
    }
    // A tree that carries the manifest but not the crate root is not a checkout of this
    // crate, and has no generation for this executable to be foreign to.
    //
    // The fixture repositories the test suite builds copy `Cargo.toml` beside the data
    // directory precisely so the generator projects *this* tree's design, and they copy
    // nothing else of the crate. Refusing them said "your executable is stale" about a
    // directory that builds no executable at all — the same lie the refusal below exists to
    // prevent, pointed the other way, and it made 109_design_system and 120_design_contrast
    // red on master.
    //
    // The question is `src/lib.rs`, not `src/`: `generate design` writes its own projections
    // into `src/web/`, `src/design/` and `src/cockpit/`, so after one run the fixture has a
    // `src` directory that contains no Rust at all and a generation that moves with every
    // projection it writes. The crate root is what distinguishes a tree that builds this
    // executable from a tree this executable merely writes into.
    if !crate_dir.join("src/lib.rs").is_file() {
        return Ok(());
    }
    if let Some(declared) = package_version(&text) {
        if declared != crate::VERSION {
            return Err(Error::Refused {
                code: REFUSED,
                reason: format!(
                    "this executable is {running} and {CRATE_DIR}/Cargo.toml declares \
                     {declared}: generating would stamp every artifact with a version this \
                     tree does not have. The tree is not stale; this executable is. \
                     Rebuild it — `cargo build --manifest-path {CRATE_DIR}/Cargo.toml` — or \
                     run `scripts/derive` with MAJORDOMUS_BIN unset, which builds it before \
                     it generates.",
                    running = crate::VERSION,
                ),
            });
        }
    }
    let tree = crate::generation::crate_generation(&crate_dir);
    if tree.as_deref() == Some(crate::GENERATION) {
        return Ok(());
    }
    if projects_the_committed_registry(app) {
        return Ok(());
    }
    Err(Error::Refused {
        code: REFUSED,
        reason: format!(
            "this executable is not of this tree's generation, and deriving with it would \
             rewrite every artifact from a model this tree does not have.\n  \
             executable  {exe}\n              generation {ours}, built from commit {commit}\n  \
             tree        {root}\n              generation {theirs}, over {CRATE_DIR}/{{{inputs}}}\n\
             The tree is not stale; this executable is, so nothing was written and nothing \
             was judged. Rebuild it — `cargo build --manifest-path {CRATE_DIR}/Cargo.toml` \
             — or run `scripts/derive` with MAJORDOMUS_BIN unset, which builds it before it \
             generates. Do not run `majordomus generate` with this executable: that is the \
             move that destroys the artifacts.",
            exe = std::env::current_exe()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| "(unknown path)".into()),
            ours = short(crate::GENERATION),
            commit = short(crate::COMMIT),
            root = app.repository.root().display(),
            theirs = tree.as_deref().map_or("unreadable", short),
            inputs = crate::generation::GENERATION_INPUTS.join(", "),
        ),
    })
}

/// Does this executable project the registry this tree has committed?
///
/// The registry manifest is the model every other artifact is a projection of, and
/// `docs/generated/registry.json` is that model as the tree carries it. Comparing them
/// asks whether this executable and the one that wrote the tree's artifacts agree about
/// what the repository *is* — a property of the model, not of a string.
///
/// Two kinds of member are dropped before comparing, and for the same reason: neither is
/// this executable's model. The provenance members are the wrapper [`generate::Document`]
/// adds — `generator` in particular carries the version, which rule 1 has already decided.
/// The index-derived members are the repository's own content: `declarative_kinds` is
/// whichever kinds the tree has objects of, so a session that adds a document of a new kind
/// makes the committed registry legitimately stale, and comparing that member would report
/// the *executable* as foreign for something the executable had no part in. Any other
/// member is compared, so one added later makes this answer `false` and the derivation
/// refuse — the safe direction.
fn projects_the_committed_registry(app: &App) -> bool {
    let path = app
        .repository
        .root()
        .join(generate::OUT_DIR)
        .join("registry.json");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return false;
    };
    let Ok(serde_json::Value::Object(committed)) = serde_json::from_str(&text) else {
        return false;
    };
    let serde_json::Value::Object(ours) = generate::registry_manifest(&app.context.registry) else {
        return false;
    };
    match (
        serde_json::to_string(&model_only(committed)),
        serde_json::to_string(&model_only(ours)),
    ) {
        (Ok(theirs), Ok(ours)) => theirs == ours,
        _ => false,
    }
}

/// A registry document with everything that is not this executable's model removed.
fn model_only(members: serde_json::Map<String, serde_json::Value>) -> serde_json::Value {
    serde_json::Value::Object(
        members
            .into_iter()
            .filter(|(k, _)| {
                !PROVENANCE_MEMBERS.contains(&k.as_str())
                    && !INDEX_DERIVED_MEMBERS.contains(&k.as_str())
            })
            .collect(),
    )
}

/// The prefix of a digest a diagnostic names — a generation, or the commit a build recorded.
/// The whole value identifies the thing; twelve hex characters distinguish it, which is what
/// a person comparing two lines of a refusal needs.
fn short(digest: &str) -> &str {
    if digest.len() <= SHORT_DIGEST {
        digest
    } else {
        &digest[..SHORT_DIGEST]
    }
}

/// How much of a digest [`short`] keeps.
const SHORT_DIGEST: usize = 12;

/// The members `generate::Document` writes above a document's own: provenance, not model.
const PROVENANCE_MEMBERS: &[&str] = &["schema", "generated", "generator"];

/// The members of the registry document that come from the index rather than from the
/// executable: a property of the repository being derived, and no evidence about which
/// executable is deriving it.
const INDEX_DERIVED_MEMBERS: &[&str] = &["declarative_kinds"];

/// The exit code of a refusal, from the exit-code contract in `docs/CLI.md`: the command
/// declined. Not `CONTRACT_UNMET`, which is what a stale artifact reports — telling them
/// apart is what lets `scripts/derive-check` stop printing "run scripts/derive" at an
/// operator whose executable, not whose tree, is the thing out of date.
const REFUSED: u8 = 15;

/// Where this crate lives in the repository that declares it.
const CRATE_DIR: &str = "apps/majordomus-cli";

/// The name of the crate this executable was built from, as its own manifest declares it.
const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

/// The `version` of the manifest's `[package]` table.
fn package_version(text: &str) -> Option<String> {
    package_field(text, "version")
}

/// One quoted scalar of the manifest's `[package]` table. Read without a TOML parser,
/// because the fields wanted here are quoted scalars in a table whose header is
/// unambiguous, and a dependency to read them would be a dependency this crate argues
/// against carrying.
fn package_field(text: &str, key: &str) -> Option<String> {
    let mut in_package = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_package = line == "[package]";
            continue;
        }
        if !in_package {
            continue;
        }
        let Some(rest) = line.strip_prefix(key) else {
            continue;
        };
        let Some(rest) = rest.trim_start().strip_prefix('=') else {
            continue;
        };
        let rest = rest.trim();
        let value = rest.split('#').next().unwrap_or(rest).trim();
        return Some(value.trim_matches('"').to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{package_field, package_version, short, CRATE_NAME, PROVENANCE_MEMBERS, REFUSED};

    #[test]
    fn a_short_digest_is_short_and_never_panics_on_a_short_input() {
        assert_eq!(short("unknown"), "unknown");
        assert_eq!(short(&"a".repeat(64)).len(), super::SHORT_DIGEST);
    }

    #[test]
    fn the_provenance_members_are_exactly_what_the_document_wrapper_adds() {
        // The model comparison subtracts these from the committed registry document. If the
        // wrapper ever adds a fourth, this fails here rather than by quietly comparing a
        // provenance line as if it were part of the model.
        let document = crate::generate::Document::new(
            "registry",
            crate::generate::REGISTRY_SCHEMA,
            "a source line",
            serde_json::json!({ "modules": [] }),
        );
        let stamped = document.stamped("9.9.9");
        let added: Vec<&str> = stamped
            .as_object()
            .expect("a stamped document is an object")
            .keys()
            .filter(|k| k.as_str() != "modules")
            .map(String::as_str)
            .collect();
        assert_eq!(added, PROVENANCE_MEMBERS);
    }

    #[test]
    fn the_registry_model_carries_no_provenance_member_of_its_own() {
        // otherwise the subtraction would remove part of the model and two generations that
        // differ only there would compare equal
        let registry = crate::capability::registry::CapabilityRegistry::builder()
            .build()
            .expect("an empty registry composes");
        let model = crate::generate::registry_manifest(&registry);
        for member in PROVENANCE_MEMBERS {
            assert!(
                model.get(member).is_none(),
                "the registry model declares {member}, which the comparison drops"
            );
        }
    }

    #[test]
    fn a_fixture_carrying_only_the_manifest_is_not_a_tree_of_any_generation() {
        // test/cases/109_design_system.sh and 120_design_contrast.sh build a repository that
        // carries share/ and apps/majordomus-cli/Cargo.toml and nothing else of the crate,
        // because the design generator projects only where the data directory and the
        // manifest live. Such a tree has no sources, so it builds no executable and cannot
        // be "a generation" that an executable is foreign to. Before this, the guard hashed
        // the single manifest, found it unequal to every real generation, and refused both
        // cases on master.
        let tree = tempfile::tempdir().expect("a temporary directory");
        let crate_dir = tree.path().join(super::CRATE_DIR);
        std::fs::create_dir_all(&crate_dir).expect("the crate directory");
        std::fs::write(
            crate_dir.join("Cargo.toml"),
            format!(
                "[package]\nname = \"{CRATE_NAME}\"\nversion = \"{v}\"\n",
                v = crate::VERSION
            ),
        )
        .expect("the manifest");

        // the manifest alone hashes to something, and it is nobody's generation
        let manifest_only = crate::generation::crate_generation(&crate_dir);
        assert!(
            manifest_only.is_some(),
            "a lone manifest is still an input, which is why the guard could see it"
        );
        assert_ne!(
            manifest_only.as_deref(),
            Some(crate::GENERATION),
            "the fixture cannot share this executable's generation; that is the whole point"
        );

        // `generate design` then writes its projections under src/, so "has a src directory"
        // is not the discriminator: the fixture acquires one and its generation moves again,
        // which is why the first attempt at this fix left both design cases red.
        std::fs::create_dir_all(crate_dir.join("src/design")).expect("the projection directory");
        std::fs::write(
            crate_dir.join("src/design/tokens.yaml"),
            "# GENERATED FILE\n",
        )
        .expect("a projection");
        assert!(crate_dir.join("src").is_dir(), "the generator made one");
        assert_ne!(
            crate::generation::crate_generation(&crate_dir).as_deref(),
            manifest_only.as_deref(),
            "a projection written under src moves the fixture's generation"
        );

        // the crate root is what a tree that builds this executable has, and it is absent
        assert!(
            !crate_dir.join("src/lib.rs").is_file(),
            "a fixture carries no crate root, however much the generator writes under src"
        );
    }

    #[test]
    fn a_refusal_is_not_the_code_a_stale_artifact_reports() {
        // the whole point of the distinction: derive-check must be able to tell "your
        // executable is wrong" from "your tree is stale", and it only has the exit code
        let stale = crate::error::Error::Stale {
            files: vec!["docs/generated/registry.json".into()],
        };
        assert_ne!(REFUSED, stale.exit_code());
    }

    #[test]
    fn a_manifest_declaring_another_crate_is_a_foreign_repository() {
        // A fixture's crate at the path this repository keeps its own tool: the version it
        // states is that crate's, and no opinion about this executable.
        let fixture = "[package]\nname = \"fixture\"\nversion = \"1.0.0\"\n";
        assert_ne!(package_field(fixture, "name").as_deref(), Some(CRATE_NAME));
        let own = format!("[package]\nname = \"{CRATE_NAME}\"\nversion = \"0.3.1\"\n");
        assert_eq!(package_field(&own, "name").as_deref(), Some(CRATE_NAME));
        // `name` is not the prefix of some other key that happens to start with it
        assert_eq!(
            package_field("[package]\nnamespace = \"x\"\n", "name"),
            None
        );
    }

    #[test]
    fn reads_the_package_version_and_not_a_dependency_version() {
        let manifest = "[package]\nname = \"majordomus-cli\"\nversion = \"0.3.1\"\n\n[dependencies]\nclap = { version = \"4.5\" }\n";
        assert_eq!(package_version(manifest).as_deref(), Some("0.3.1"));
    }

    #[test]
    fn a_version_outside_the_package_table_is_not_the_package_version() {
        let manifest = "[dependencies]\nserde = { version = \"1\" }\n";
        assert_eq!(package_version(manifest), None);
    }

    #[test]
    fn a_trailing_comment_is_not_part_of_the_version() {
        let manifest = "[package]\nversion = \"1.2.3\"   # bumped by `release version`\n";
        assert_eq!(package_version(manifest).as_deref(), Some("1.2.3"));
    }
}
