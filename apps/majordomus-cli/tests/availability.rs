//! Availability and visibility: classified once from what a capability declares, read as
//! fields by every projection, and decided by no page for itself.

use majordomus_cli::capability::{builtin, Availability, CapabilityRegistry, Visibility};

fn registry() -> CapabilityRegistry {
    CapabilityRegistry::builder()
        .with_builtin(builtin::all())
        .build()
        .unwrap()
}

#[test]
fn every_capability_is_classified_by_what_it_declares() {
    let registry = registry();
    let mut seen = 0;
    for c in registry.iter() {
        assert_eq!(
            c.availability,
            Availability::classify(c.kind, &c.exposure),
            "{} carries an availability its declaration does not support",
            c.id
        );
        assert_eq!(
            c.visibility,
            Visibility::classify(&c.exposure),
            "{} carries a visibility its declaration does not support",
            c.id
        );
        seen += 1;
    }
    assert!(seen > 0, "the registry composed nothing to classify");
}

#[test]
fn a_capability_reachable_over_a_transport_is_public_and_needs_a_process() {
    let registry = registry();
    for c in registry.iter() {
        if c.exposure.http.is_some() || c.exposure.mcp.is_some() {
            assert_eq!(c.visibility, Visibility::Public, "{}", c.id);
        }
        if c.kind.is_executable() {
            assert_eq!(
                c.availability,
                Availability::Runtime,
                "{} is executed, and a handler needs a process",
                c.id
            );
        }
    }
}

/// The failure this whole contract exists to prevent: a page deciding for itself whether
/// a surface is available, usually by looking at the address it was served from. A rule
/// written that way is invisible to the model, untestable from outside the browser, and
/// wrong the first time the site moves.
///
/// What is forbidden is the decision, not the mention: a documentation link that happens
/// to name the published host is a link, and this check would be worthless if it could
/// not tell the two apart. So a finding is a line that reads the serving address *and*
/// compares it.
#[test]
fn no_projection_decides_availability_from_where_it_is_served() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the worktree root")
        .to_path_buf();
    // reading where we are served from
    let address = [
        "location.hostname",
        "location.host",
        "location.origin",
        "document.domain",
        "window.location",
    ];
    // and turning it into a verdict
    let comparison = [
        "includes(",
        "contains(",
        "startsWith(",
        "endsWith(",
        "==",
        "!=",
        "match(",
    ];
    let mut findings = Vec::new();
    for dir in ["apps/majordomus-cli/src", "share/cockpit/src"] {
        let dir = root.join(dir);
        if !dir.exists() {
            continue;
        }
        walk(&dir, &mut |path, text| {
            for (n, line) in text.lines().enumerate() {
                let reads_address = address.iter().any(|a| line.contains(a));
                let decides = comparison.iter().any(|c| line.contains(c));
                if reads_address && decides {
                    findings.push(format!(
                        "{}:{}: {}",
                        path.strip_prefix(&root).unwrap_or(path).display(),
                        n + 1,
                        line.trim()
                    ));
                }
            }
        });
    }
    assert!(
        findings.is_empty(),
        "availability comes from the capability's metadata, not from the address a page \
         was served from:\n  {}",
        findings.join("\n  ")
    );
}

fn walk(dir: &std::path::Path, f: &mut impl FnMut(&std::path::Path, &str)) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, f);
        } else if matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("rs" | "js" | "html" | "css")
        ) {
            if let Ok(text) = std::fs::read_to_string(&path) {
                f(&path, &text);
            }
        }
    }
}
