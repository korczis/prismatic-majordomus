//! `majordomus design kit`: the repository's user interface against a kit profile, through
//! the registry's `design.kit`, and `design kit adopt`, through `design.kit.adopt`
//! (I4334, ADR 0230).
//!
//! The command renders the capability's answer and chooses the exit code from it; the
//! enumeration, the judgement and the verdict are the capability's, so the terminal, the
//! `--format json` document, MCP and HTTP cannot disagree about whether the kit holds.
//!
//! One path does not go through the registry: a `--repo` that Majordomus does not
//! supervise has no layer to load and so no registry to ask, and the owner's mandate covers
//! those repositories too. There the command calls the same judgement the capability calls,
//! [`crate::design::kit::inspect`], with the distribution's share directory, and answers
//! with the same document.

use std::io::Write;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::app::App;
use crate::capability::CapabilityError;
use crate::cli::{DesignArgs, DesignCommand, KitAction, OutputFormat, RepoArgs};
use crate::design::kit::{self, AdoptAction, AdoptReport, KitReport, KitVerdict};
use crate::error::{Error, Result};

/// The exit code when a finding stands under `--check`.
pub const EXIT_FINDINGS: u8 = 10;
/// The exit code of a policy that cannot be read.
pub const EXIT_BAD_POLICY: u8 = 2;

/// Run `majordomus design`.
pub fn run(args: DesignArgs) -> Result<u8> {
    run_to(args, &mut std::io::stdout().lock())
}

fn run_to(args: DesignArgs, w: &mut dyn Write) -> Result<u8> {
    let DesignCommand::Kit(kit_args) = args.command;
    match kit_args.action {
        None => {
            let value = judged(&args.repo)?;
            let report: KitReport =
                serde_json::from_value(value.clone()).map_err(|e| Error::Protocol {
                    reason: format!(
                        "design.kit answered with something this command cannot read: {e}"
                    ),
                })?;
            emit(w, args.format, &value, || render(&report))?;
            Ok(exit_code(&report, kit_args.check))
        }
        Some(KitAction::Adopt { profile, dry_run }) => {
            let app = App::load(&args.repo)?;
            let input = json!({ "profile": profile.name(), "dry_run": dry_run });
            let value = execute(&app.context, &["design", "kit", "adopt"], input)?;
            let report: AdoptReport =
                serde_json::from_value(value.clone()).map_err(|e| Error::Protocol {
                    reason: format!(
                        "design.kit.adopt answered with something this command cannot read: {e}"
                    ),
                })?;
            emit(w, args.format, &value, || render_adoption(&report))?;
            Ok(0)
        }
    }
}

/// The judgement, from the registry when the repository is supervised and from the same
/// function the capability calls when it is not.
fn judged(repo: &RepoArgs) -> Result<Value> {
    match App::load(repo) {
        Ok(app) => execute(&app.context, &["design", "kit"], json!({})),
        Err(Error::RepositoryNotFound { start }) => {
            let root = toplevel(&start);
            let share = crate::share::Share::locate(repo.share.as_deref(), &root)?;
            let report = kit::inspect(&root, share.dir()).map_err(|reason| Error::Protocol {
                reason: format!("design kit: {reason}"),
            })?;
            serde_json::to_value(&report).map_err(|e| Error::Protocol {
                reason: e.to_string(),
            })
        }
        Err(other) => Err(other),
    }
}

/// The root of the git work tree holding `start`, or `start` itself outside one.
fn toplevel(start: &Path) -> PathBuf {
    let start = start.canonicalize().unwrap_or_else(|_| start.to_path_buf());
    start
        .ancestors()
        .find(|d| d.join(".git").exists())
        .map(Path::to_path_buf)
        .unwrap_or(start)
}

fn execute(ctx: &crate::capability::Context, words: &[&str], input: Value) -> Result<Value> {
    let words: Vec<String> = words.iter().map(|w| w.to_string()).collect();
    let id = ctx
        .registry
        .by_cli(&words)
        .map(|c| c.id.as_str())
        .ok_or_else(|| Error::Protocol {
            reason: format!(
                "no capability is exposed as `majordomus {}`",
                words.join(" ")
            ),
        })?;
    ctx.execute(id, input).map_err(|e| match e {
        CapabilityError::Refused(reason) => Error::Refused { code: 15, reason },
        CapabilityError::NotFound(reason) => Error::Refused { code: 12, reason },
        CapabilityError::InvalidInput(reason) => Error::Refused { code: 2, reason },
        other => Error::Protocol {
            reason: other.to_string(),
        },
    })
}

fn emit(
    w: &mut dyn Write,
    format: OutputFormat,
    value: &Value,
    text: impl FnOnce() -> String,
) -> Result<()> {
    let out = match format {
        OutputFormat::Json => {
            serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
        }
        OutputFormat::Text => text(),
    };
    writeln!(w, "{out}").map_err(Error::Transport)
}

/// The exit code a report earns: a bad policy always refuses, a finding refuses only under
/// `--check`, and no interface is a pass.
fn exit_code(report: &KitReport, check: bool) -> u8 {
    match report.verdict {
        KitVerdict::BadPolicy => EXIT_BAD_POLICY,
        KitVerdict::Findings if check => EXIT_FINDINGS,
        _ => 0,
    }
}

/// The report as a person reads it: every finding with its remedy, every exemption with
/// its reason, then the verdict.
fn render(report: &KitReport) -> String {
    let mut out = Vec::new();
    let policy = if report.policy_declared {
        "declared in .ai/repo/policy.yaml"
    } else {
        "no design_kit block; the default profile"
    };
    out.push(format!(
        "design kit: {} ({policy}), {}",
        report.profile, report.profile_source
    ));
    for e in &report.policy_errors {
        out.push(format!("POLICY  {e}"));
    }
    for f in &report.findings {
        let path = if f.path.is_empty() {
            "."
        } else {
            f.path.as_str()
        };
        out.push(format!("FAIL  {:?}  {path}  {}", f.signal, f.evidence));
        out.push(format!("      remedy: {}", f.remedy));
    }
    for x in &report.exempted {
        out.push(format!(
            "EXEMPT  {:?}  {}  {}  because: {}",
            x.finding.signal, x.finding.path, x.finding.evidence, x.because
        ));
    }
    for n in &report.reported {
        out.push(format!("NOTE  {n}"));
    }
    for n in &report.notices {
        out.push(format!("NOTICE  {n}"));
    }
    if let Some(l) = &report.licence {
        out.push(format!(
            "licence: {:?}, {:?} ({}){}",
            l.variant,
            l.state,
            l.evidence,
            if l.markers.is_empty() {
                String::new()
            } else {
                format!("; licensed markers: {}", l.markers.join("; "))
            }
        ));
    }
    for s in &report.surfaces {
        let dir = if s.dir.is_empty() {
            "."
        } else {
            s.dir.as_str()
        };
        out.push(format!(
            "surface {dir} ({:?}): {}",
            s.kind,
            if s.usage.is_empty() {
                "no usage marker".to_string()
            } else {
                s.usage.join("; ")
            }
        ));
    }
    if report.ui {
        out.push(format!(
            "interface: {}; stacks: {}; layouts: {}",
            report.ui_evidence.join("; "),
            if report.stacks.is_empty() {
                "none".to_string()
            } else {
                report.stacks.join(" ")
            },
            if report.layouts.is_empty() {
                "none".to_string()
            } else {
                report.layouts.join(" ")
            }
        ));
    }
    out.push(match report.verdict {
        KitVerdict::NoUi => "design kit: no user interface; the rule holds vacuously".to_string(),
        KitVerdict::Clean => format!(
            "design kit: clean ({} exemption(s) covering {} finding(s))",
            report.exemptions.len(),
            report.exempted.len()
        ),
        KitVerdict::Findings => format!("design kit: {} finding(s)", report.findings.len()),
        KitVerdict::BadPolicy => format!(
            "design kit: the policy cannot be read ({} problem(s)); nothing was judged",
            report.policy_errors.len()
        ),
    });
    out.join("\n")
}

/// The adoption as a person reads it.
fn render_adoption(report: &AdoptReport) -> String {
    let mut out = Vec::new();
    for w in &report.writes {
        let action = match w.action {
            AdoptAction::Created => "create",
            AdoptAction::Updated => "update",
            AdoptAction::Unchanged => "unchanged",
            AdoptAction::Kept => "kept",
        };
        out.push(format!("{action:<10} {}  {}", w.path, w.detail));
    }
    for n in &report.notes {
        out.push(format!("note       {n}"));
    }
    out.push(format!(
        "design kit adopt: {} ({}){}",
        report.profile,
        report.rule,
        if report.dry_run {
            ", dry run: nothing written"
        } else {
            ""
        }
    ));
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{Cli, Command, DiscoveryMode};
    use crate::design::kit::{AdoptWrite, KitExempted, KitFinding, KitSignal};
    use clap::Parser;

    fn report(verdict: KitVerdict) -> KitReport {
        let mut r = KitReport::bad_policy("flowbite-pro", Vec::new());
        r.policy_declared = false;
        r.verdict = verdict;
        r
    }

    #[test]
    fn the_exit_code_is_the_verdict_and_check_decides_a_finding() {
        assert_eq!(exit_code(&report(KitVerdict::NoUi), true), 0);
        assert_eq!(exit_code(&report(KitVerdict::Clean), true), 0);
        assert_eq!(
            exit_code(&report(KitVerdict::Findings), true),
            EXIT_FINDINGS
        );
        assert_eq!(exit_code(&report(KitVerdict::Findings), false), 0);
        assert_eq!(
            exit_code(&report(KitVerdict::BadPolicy), false),
            EXIT_BAD_POLICY
        );
    }

    #[test]
    fn the_rendering_names_the_signal_the_path_the_remedy_and_the_verdict() {
        let mut r = report(KitVerdict::Findings);
        r.ui = true;
        r.ui_evidence = vec!["package.json declares flowbite".into()];
        r.findings = vec![KitFinding {
            signal: KitSignal::S4,
            path: "package.json".into(),
            evidence: "package.json declares daisyui".into(),
            remedy: "remove it".into(),
        }];
        r.exempted = vec![KitExempted {
            finding: KitFinding {
                signal: KitSignal::S5,
                path: "reel.html".into(),
                evidence: "no navbar".into(),
                remedy: "x".into(),
            },
            exemption: "reel.html".into(),
            because: "a render canvas".into(),
        }];
        r.reported = vec!["Alpine.js".into()];
        r.notices = vec!["examples/x/package.json declares react and is no surface".into()];
        r.surfaces = vec![kit::KitSurface {
            dir: String::new(),
            kind: kit::KitSurfaceKind::Root,
            manifests: vec!["package.json".into()],
            usage: Vec::new(),
        }];
        r.licence = Some(kit::KitLicence {
            evidence: "kit-evidence/v1".into(),
            state: kit::LicenceState::NotJudged,
            variant: kit::KitVariant::Undecided,
            markers: vec!["flowbite-pro-x/README.md (a distribution)".into()],
        });
        let t = render(&r);
        assert!(t.contains("NOTICE  examples/x/package.json"), "{t}");
        assert!(t.contains("surface . (Root): no usage marker"), "{t}");
        assert!(
            t.contains(
                "licence: Undecided, NotJudged (kit-evidence/v1); licensed markers: flowbite-pro-x"
            ),
            "{t}"
        );
        assert!(
            t.contains("FAIL  S4  package.json  package.json declares daisyui"),
            "{t}"
        );
        assert!(t.contains("remedy: remove it"));
        assert!(t.contains("EXEMPT  S5  reel.html  no navbar  because: a render canvas"));
        assert!(t.contains("NOTE  Alpine.js"));
        assert!(t.contains("stacks: none; layouts: none"));
        assert!(t.ends_with("design kit: 1 finding(s)"));
        assert!(render(&report(KitVerdict::NoUi)).ends_with("holds vacuously"));
        assert!(render(&report(KitVerdict::Clean)).contains("clean (0 exemption(s)"));
        let mut bad = report(KitVerdict::BadPolicy);
        bad.policy_declared = true;
        bad.policy_errors = vec!["design_kit.exempt[0] gives no because".into()];
        let t = render(&bad);
        assert!(t.contains("POLICY  design_kit.exempt[0] gives no because"));
        assert!(t.contains("declared in .ai/repo/policy.yaml"));
        assert!(t.ends_with("nothing was judged"));
        let mut root = report(KitVerdict::Findings);
        root.findings = vec![KitFinding {
            signal: KitSignal::S1,
            path: String::new(),
            evidence: "e".into(),
            remedy: "r".into(),
        }];
        root.ui = true;
        root.stacks = vec!["tailwind-4".into()];
        root.layouts = vec!["index.html".into()];
        let t = render(&root);
        assert!(t.contains("FAIL  S1  .  e"));
        assert!(t.contains("stacks: tailwind-4; layouts: index.html"));
    }

    #[test]
    fn the_adoption_reads_as_what_happens_to_each_file() {
        let r = AdoptReport {
            profile: "flowbite-pro".into(),
            rule: "project.ui-uses-flowbite-pro".into(),
            dry_run: true,
            writes: [
                AdoptAction::Created,
                AdoptAction::Updated,
                AdoptAction::Unchanged,
                AdoptAction::Kept,
            ]
            .into_iter()
            .map(|action| AdoptWrite {
                path: "p".into(),
                action,
                detail: "d".into(),
            })
            .collect(),
            ci_workflow: None,
            notes: vec!["wire the hook".into()],
        };
        let t = render_adoption(&r);
        for word in [
            "create",
            "update",
            "unchanged",
            "kept",
            "note       wire the hook",
        ] {
            assert!(t.contains(word), "{word}: {t}");
        }
        assert!(t.ends_with("dry run: nothing written"));
        let written = AdoptReport {
            dry_run: false,
            ..r
        };
        assert!(render_adoption(&written).ends_with("(project.ui-uses-flowbite-pro)"));
    }

    fn args(argv: &[&str]) -> DesignArgs {
        let Command::Design(args) = Cli::try_parse_from(argv).unwrap().command else {
            panic!("not the design command")
        };
        args
    }

    #[test]
    fn an_unsupervised_tree_is_judged_by_the_same_function() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".git")).unwrap();
        std::fs::write(
            dir.path().join("package.json"),
            r#"{"dependencies": {"tailwindcss": "^4", "bootstrap": "5"}}"#,
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        let share = crate::synthetic::crate_share();
        let root = dir.path().to_string_lossy().to_string();
        let share_arg = share.to_string_lossy().to_string();
        let mut a = args(&[
            "majordomus",
            "design",
            "kit",
            "--check",
            "--repo",
            &format!("{root}/src"),
            "--share",
            &share_arg,
            "--format",
            "json",
        ]);
        a.repo.discovery = DiscoveryMode::Filesystem;
        let mut out = Vec::new();
        let code = run_to(a, &mut out).unwrap();
        assert_eq!(code, EXIT_FINDINGS);
        let v: Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(v["verdict"], "findings");
        assert!(v["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["signal"] == "S4"));

        // the same tree as text, without --check, reports and passes
        let a = args(&[
            "majordomus",
            "design",
            "kit",
            "--repo",
            &root,
            "--share",
            &share_arg,
        ]);
        let mut out = Vec::new();
        assert_eq!(run_to(a, &mut out).unwrap(), 0);
        assert!(String::from_utf8(out).unwrap().contains("FAIL  S4"));

        // adoption needs a supervised repository
        let a = args(&[
            "majordomus",
            "design",
            "kit",
            "adopt",
            "flowbite-pro",
            "--repo",
            &root,
        ]);
        assert!(matches!(
            run_to(a, &mut Vec::new()),
            Err(Error::RepositoryNotFound { .. })
        ));
        assert_eq!(
            toplevel(&dir.path().join("src")),
            dir.path().canonicalize().unwrap()
        );
        let plain = tempfile::tempdir().unwrap();
        assert_eq!(toplevel(plain.path()), plain.path().canonicalize().unwrap());
    }

    #[test]
    fn a_supervised_repository_is_judged_and_adopts_through_the_registry() {
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let root = repo.root().to_string_lossy().to_string();
        let share = crate::synthetic::crate_share()
            .to_string_lossy()
            .to_string();
        let a = args(&[
            "majordomus",
            "design",
            "kit",
            "--check",
            "--repo",
            &root,
            "--share",
            &share,
            "--discovery",
            "filesystem",
            "--format",
            "json",
        ]);
        let mut out = Vec::new();
        assert_eq!(run_to(a, &mut out).unwrap(), 0, "no interface is a pass");
        let v: Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(v["verdict"], "no_ui");

        let a = args(&[
            "majordomus",
            "design",
            "kit",
            "adopt",
            "flowbite-pro",
            "--dry-run",
            "--repo",
            &root,
            "--share",
            &share,
            "--discovery",
            "filesystem",
        ]);
        let mut out = Vec::new();
        match run_to(a, &mut out) {
            Ok(code) => {
                assert_eq!(code, 0);
                let t = String::from_utf8(out).unwrap();
                assert!(t.contains("dry run: nothing written"), "{t}");
            }
            // a synthetic layer without a policy file is refused, and says so
            Err(Error::Refused { code, reason }) => {
                assert_eq!(code, 15);
                assert!(reason.contains("policy"), "{reason}");
            }
            Err(other) => panic!("unexpected: {other}"),
        }
    }
}
