//! `majordomus self update`: report or apply an upgrade to the published stable release.
//!
//! The command line of `self.update`, run through the one executor. The text rendering
//! prints the running and the stable version, the verdict and its reason, and under
//! `--apply` the version the launcher reports afterwards; the exit status is the
//! capability's own: 0 nothing to do or done, 10 refused or the installer failed, 12 not
//! answered.

use std::io::Write;

use serde_json::{json, Value};

use crate::app::App;
use crate::capability::CapabilityError;
use crate::cli::{OutputFormat, SelfArgs, SelfCommand};
use crate::error::{Error, Result};

/// Run `majordomus self`.
pub fn run(args: SelfArgs) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let ctx = &app.context;
    let SelfCommand::Update {
        check: _,
        apply,
        version,
        base_url,
        timeout,
    } = args.command;
    let mut input = json!({
        "apply": apply, "version": version, "base_url": base_url, "timeout_seconds": timeout,
    });
    if let Some(m) = input.as_object_mut() {
        m.retain(|_, x| !x.is_null());
    }
    let id = ctx
        .registry
        .by_cli(&["self".to_string(), "update".to_string()])
        .map(|c| c.id.as_str())
        .ok_or_else(|| Error::Protocol {
            reason: "no capability is exposed as `majordomus self update`".into(),
        })?;
    let v = ctx.execute(id, input).map_err(|e| match e {
        CapabilityError::InvalidInput(reason) | CapabilityError::Refused(reason) => {
            Error::Protocol { reason }
        }
        other => Error::Protocol {
            reason: other.to_string(),
        },
    })?;
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    match args.format {
        OutputFormat::Json => writeln!(out, "{}", serde_json::to_string_pretty(&v).unwrap_or_default()),
        OutputFormat::Text => write!(out, "{}", text(&v)),
    }
    .map_err(Error::Transport)?;
    Ok(u8::try_from(v["exit_code"].as_i64().unwrap_or(12)).unwrap_or(12))
}

/// The report as a person reads it.
///
/// ```ignore
/// // exercised through the binary by test/cases/2720_self_update_wraps_the_installer.sh
/// ```
fn text(v: &Value) -> String {
    let s = |p: &str| v.pointer(p).and_then(Value::as_str).unwrap_or("").to_string();
    let mut lines = vec![
        format!("running      v{}", s("/running")),
        format!(
            "stable       {}",
            v["stable"].as_str().map_or("unread".to_string(), |x| format!("v{x}"))
        ),
        format!("metadata     {}", s("/metadata_url")),
        format!("verdict      {}  {}", s("/decision/verdict"), s("/decision/reason")),
        format!("applies      {}", if v["applies"] == json!(true) { "yes" } else { "no" }),
    ];
    if let Some(why) = v["not_installed"].as_str() {
        lines.push(format!("installation none: {why}"));
    } else if let Some(at) = v.get("installation").filter(|x| !x.is_null()) {
        lines.push(format!(
            "installation {} (launchers in {})",
            at["prefix"].as_str().unwrap_or("?"),
            at["install_dir"].as_str().unwrap_or("?")
        ));
    }
    if let Some(a) = v.get("applied").filter(|x| !x.is_null()) {
        let argv: Vec<&str> = a["argv"]
            .as_array()
            .map(|x| x.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        lines.push(format!("ran          {}", argv.join(" ")));
        lines.push(format!(
            "installer    exit {}",
            a["status"].as_i64().map_or("by signal".into(), |c| c.to_string())
        ));
        lines.push(format!(
            "installed    {}",
            a["installed"].as_str().map_or("nothing read back".to_string(), |x| format!("v{x}"))
        ));
        if a["status"].as_i64() != Some(0) {
            for l in a["output"].as_array().into_iter().flatten().filter_map(Value::as_str) {
                lines.push(format!("  {l}"));
            }
        }
    }
    lines.push(String::new());
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_applied_upgrade_names_what_ran_and_what_it_reads_back() {
        let v = json!({
            "running": "0.9.0", "stable": "1.0.0", "metadata_url": "http://x/releases/latest.json",
            "decision": {"verdict": "upgrade", "reason": "newer"}, "applies": true,
            "installation": {"prefix": "/p", "install_dir": "/b", "tree": "/p/versions/0.9.0", "version": "0.9.0"},
            "applied": {"argv": ["sh", "install.sh", "--version", "v1.0.0"], "status": 0, "output": [], "installed": "1.0.0"},
            "exit_code": 0
        });
        let t = text(&v);
        assert!(t.contains("running      v0.9.0"), "{t}");
        assert!(t.contains("verdict      upgrade"), "{t}");
        assert!(t.contains("ran          sh install.sh --version v1.0.0"), "{t}");
        assert!(t.contains("installed    v1.0.0"), "{t}");
    }
}
