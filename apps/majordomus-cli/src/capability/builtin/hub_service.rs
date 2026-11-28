//! The capabilities of the hub as a host service (I4816): `mesh.hub` reads it back,
//! `mesh.hub.install` and `mesh.hub.uninstall` write and remove its units, and
//! `mesh.hub.update` brings it to the published release. The logic is
//! [`crate::mesh::hub_service`]; this file builds the real host and the spec from the
//! repository, its declaration and the process's environment.

use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::time::Duration;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::mesh::declaration;
use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::mesh::hub_service::{
    self, Host, HubServiceReport, HubStatusReport, HubUpdateReport, Manager, Spec,
};

/// The input of `mesh.hub`, the status: what a caller over HTTP or MCP may name. The
/// executable and the release origin are this process's own (the installer's launcher,
/// `MAJORDOMUS_RELEASE_BASE_URL`), never a caller's, because the status runs the one and
/// reads the other.
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HubStatusInput {
    /// The hub port; default the port the declaration gives this machine's address, else 8791.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    /// A staging directory that stands for the home: no service manager or hub is asked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root: Option<String>,
}

impl BenchmarkCases for HubStatusInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![NamedCase::new("declared", HubStatusInput::default())]
    }
}

/// The input of `mesh.hub.install`, `mesh.hub.uninstall` and `mesh.hub.update`, offered on
/// the command line alone.
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HubInput {
    /// Carry the plan out. Absent, install, uninstall and update only plan and change nothing.
    #[serde(default)]
    pub apply: bool,
    /// A staging directory that stands for the home: the units are written under it, no
    /// service manager is asked and no hub is reached.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root: Option<String>,
    /// The hub port; default the port the declaration gives this machine's address, else 8791.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    /// The executable the units run; default the installer's launcher, else this executable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub executable: Option<String>,
    /// Where the published release and the installer are read; default
    /// `MAJORDOMUS_RELEASE_BASE_URL`, else https://majordomus.dev.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release_base: Option<String>,
    /// Update: try again the versions that failed on this node.
    #[serde(default)]
    pub retry: bool,
}

impl BenchmarkCases for HubInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![NamedCase::new("plan", HubInput::default())]
    }
}

/// Run one argv to its end, its standard input closed, and answer whether it succeeded and
/// what it printed on either stream.
pub(crate) fn run_command(argv: &[String]) -> (bool, String) {
    let Some((program, args)) = argv.split_first() else {
        return (false, "empty command".into());
    };
    match std::process::Command::new(program)
        .args(args)
        .stdin(std::process::Stdio::null())
        .output()
    {
        Ok(o) => (
            o.status.success(),
            format!(
                "{}{}",
                String::from_utf8_lossy(&o.stdout),
                String::from_utf8_lossy(&o.stderr)
            ),
        ),
        Err(e) => (false, format!("{program}: {e}")),
    }
}

/// GET `path` of `base` as JSON within five seconds: the body of a 200, else nothing.
pub(crate) fn get_json(base: &str, path: &str) -> Option<String> {
    crate::mcp::bridge::request(
        base,
        "GET",
        path,
        &[("Accept", "application/json")],
        None,
        Duration::from_secs(5),
    )
    .ok()
    .filter(|r| r.status == 200)
    .map(|r| r.body)
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Whether a process of this id lives: signal 0 reaches it, or it exists under another user.
pub(crate) fn alive(pid: u32) -> bool {
    // pid 0 would ask the whole process group
    let Ok(pid) =
        libc::pid_t::try_from(pid)
            .map_err(|_| ())
            .and_then(|p| if p > 0 { Ok(p) } else { Err(()) })
    else {
        return false;
    };
    // SAFETY: kill with signal 0 sends nothing; it only asks whether the process exists.
    let rc = unsafe { libc::kill(pid, 0) };
    rc == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

/// The release origin: the one asked for, else the installer's own variable, else the
/// published site.
pub(crate) fn release_base(asked: Option<String>, env: Option<String>) -> String {
    asked
        .or(env.filter(|e| !e.is_empty()))
        .unwrap_or_else(|| hub_service::RELEASE_BASE.into())
}

fn uid() -> u32 {
    // SAFETY: getuid reads the real user id and cannot fail.
    unsafe { libc::getuid() }
}

/// The executable the units run: the one asked for, else the installer's launcher (whose path
/// does not move when the version does), else this executable.
pub(crate) fn executable_for(
    asked: Option<&str>,
    launcher: Option<String>,
    exe: Option<PathBuf>,
) -> PathBuf {
    asked
        .map(PathBuf::from)
        .or_else(|| launcher.filter(|l| !l.is_empty()).map(PathBuf::from))
        .or(exe)
        .unwrap_or_else(|| PathBuf::from("majordomus"))
}

/// The hub port and whether this machine is a declared hub on it: the port asked for, else
/// the first declared hub port whose address is this machine's, else the default.
pub(crate) fn port_for(
    asked: Option<u16>,
    config: Option<&crate::mesh::MeshConfig>,
    local: &[Ipv4Addr],
) -> (u16, bool) {
    let declared = crate::mesh::firewall::plan(config, local, None).hub_ports;
    let port = asked
        .or_else(|| declared.first().copied())
        .unwrap_or(hub_service::DEFAULT_PORT);
    (port, declared.contains(&port))
}

struct Built {
    spec: Spec,
    declared: bool,
    asked: bool,
    staging: bool,
    base: String,
}

fn build(ctx: &Context, input: &HubInput) -> Built {
    let asked = input.executable.is_some();
    let config = declaration(ctx).and_then(|d| d.ok());
    let local: Vec<Ipv4Addr> = crate::mesh::address::interfaces()
        .into_iter()
        .map(|(_, ip)| ip)
        .collect();
    let (port, declared) = port_for(input.port, config.as_ref(), &local);
    let staging = input.root.is_some();
    let home = input
        .root
        .clone()
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("/"));
    let spec = Spec {
        manager: Manager::of(std::env::consts::OS),
        home,
        executable: executable_for(
            input.executable.as_deref(),
            std::env::var("MAJORDOMUS_LAUNCHER").ok(),
            std::env::current_exe().ok(),
        ),
        repo: PathBuf::from(&ctx.index.repository.root),
        port,
    };
    Built {
        spec,
        declared,
        asked,
        staging,
        base: release_base(
            input.release_base.clone(),
            std::env::var("MAJORDOMUS_RELEASE_BASE_URL").ok(),
        ),
    }
}

fn with_host<T>(built: &Built, f: impl FnOnce(&Host<'_>) -> T) -> T {
    let host = Host {
        manager: built.spec.manager,
        uid: uid(),
        user: std::env::var("USER").unwrap_or_default(),
        pid: std::process::id(),
        staging: built.staging,
        exec: &run_command,
        get: &get_json,
        alive: &alive,
        now: &now,
        sleep: &std::thread::sleep,
    };
    f(&host)
}

pub(crate) fn mesh_hub(
    ctx: &Context,
    input: HubStatusInput,
) -> Result<HubStatusReport, CapabilityError> {
    let b = build(
        ctx,
        &HubInput {
            port: input.port,
            root: input.root,
            ..HubInput::default()
        },
    );
    Ok(with_host(&b, |h| {
        hub_service::status(h, &b.spec, b.declared, &b.base)
    }))
}

pub(crate) fn mesh_hub_install(
    ctx: &Context,
    input: HubInput,
) -> Result<HubServiceReport, CapabilityError> {
    let b = build(ctx, &input);
    Ok(with_host(&b, |h| {
        hub_service::install(h, &b.spec, b.declared, b.asked, input.apply)
    }))
}

pub(crate) fn mesh_hub_uninstall(
    ctx: &Context,
    input: HubInput,
) -> Result<HubServiceReport, CapabilityError> {
    let b = build(ctx, &input);
    Ok(with_host(&b, |h| {
        hub_service::uninstall(h, &b.spec, input.apply)
    }))
}

pub(crate) fn mesh_hub_update(
    ctx: &Context,
    input: HubInput,
) -> Result<HubUpdateReport, CapabilityError> {
    let b = build(ctx, &input);
    Ok(with_host(&b, |h| {
        hub_service::update(h, &b.spec, &b.base, input.apply, input.retry)
    }))
}

/// Whether `path` is under `root`: a staging report names no path outside it.
#[cfg(test)]
fn under(path: &str, root: &std::path::Path) -> bool {
    std::path::Path::new(path).starts_with(root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synthetic::{Shape, SyntheticRepository};

    #[test]
    fn a_command_is_run_to_its_end_and_an_unknown_one_says_why() {
        let (ok, out) = run_command(&["sh".into(), "-c".into(), "echo out; echo err >&2".into()]);
        assert!(ok && out.contains("out") && out.contains("err"));
        let (ok, _) = run_command(&["sh".into(), "-c".into(), "exit 3".into()]);
        assert!(!ok);
        let (ok, out) = run_command(&["/nonexistent/mj-nothing".into()]);
        assert!(!ok && out.contains("mj-nothing"));
        assert_eq!(run_command(&[]), (false, "empty command".into()));
    }

    #[test]
    fn nothing_answering_is_no_body() {
        assert_eq!(get_json("http://127.0.0.1:9", "/"), None);
        assert!(now() > 1_700_000_000);
        let _ = uid();
        assert!(alive(std::process::id()));
        assert!(!alive(u32::MAX));
        assert!(!alive(0));
        assert!(!alive(999_999_999));
        assert_eq!(release_base(Some("a".into()), Some("b".into())), "a");
        assert_eq!(release_base(None, Some("b".into())), "b");
        assert_eq!(
            release_base(None, Some(String::new())),
            hub_service::RELEASE_BASE
        );
    }

    #[test]
    fn the_executable_is_the_asked_then_the_launcher_then_this_one() {
        let exe = Some(PathBuf::from("/x/majordomus-cli"));
        assert_eq!(
            executable_for(Some("/a"), Some("/l".into()), exe.clone()),
            PathBuf::from("/a")
        );
        assert_eq!(
            executable_for(None, Some("/l".into()), exe.clone()),
            PathBuf::from("/l")
        );
        assert_eq!(
            executable_for(None, Some(String::new()), exe.clone()),
            PathBuf::from("/x/majordomus-cli")
        );
        assert_eq!(
            executable_for(None, None, None),
            PathBuf::from("majordomus")
        );
    }

    #[test]
    fn the_port_is_the_declared_hubs_of_this_machine() {
        let config: crate::mesh::MeshConfig = serde_json::from_value(serde_json::json!({
            "schema": "mesh/v1", "kind": "mesh-declaration", "id": "t", "enabled": true,
            "rendezvous": { "endpoints": ["http://192.168.7.10:8799"] },
        }))
        .unwrap();
        let here = [Ipv4Addr::new(192, 168, 7, 10)];
        assert_eq!(port_for(None, Some(&config), &here), (8799, true));
        assert_eq!(port_for(Some(8791), Some(&config), &here), (8791, false));
        assert_eq!(
            port_for(None, Some(&config), &[Ipv4Addr::new(10, 0, 0, 1)]),
            (8791, false)
        );
        assert_eq!(port_for(None, None, &here), (8791, false));
    }

    #[test]
    fn the_capabilities_answer_in_a_staging_root_without_touching_the_host() {
        let root = tempfile::tempdir().unwrap();
        let repo = SyntheticRepository::new(Shape::default()).unwrap();
        let ctx = repo.context().expect("a context");
        let input = |apply| HubInput {
            apply,
            root: Some(root.path().display().to_string()),
            port: Some(8791),
            executable: Some("/nonexistent/majordomus".into()),
            release_base: Some("http://127.0.0.1:9".into()),
            retry: false,
        };
        // the synthetic repository declares no hub: install is refused, the rest answer
        let installed = mesh_hub_install(&ctx, input(true)).unwrap();
        assert!(!installed.ok && installed.refusal.is_some());
        let status = mesh_hub(
            &ctx,
            HubStatusInput {
                port: Some(8791),
                root: Some(root.path().display().to_string()),
            },
        )
        .unwrap();
        assert!(!status.ok);
        assert!(status.units.iter().all(|u| under(&u.path, root.path())));
        let removed = mesh_hub_uninstall(&ctx, input(true)).unwrap();
        assert!(removed.ok && removed.steps.is_empty());
        let updated = mesh_hub_update(&ctx, input(false)).unwrap();
        assert_eq!(updated.mode, "plan");
        assert!(!updated.ok);
        assert_eq!(
            HubInput::benchmark_cases(&CaseContext { index: &ctx.index }).len(),
            1
        );
        assert_eq!(
            HubStatusInput::benchmark_cases(&CaseContext { index: &ctx.index }).len(),
            1
        );
    }
}
