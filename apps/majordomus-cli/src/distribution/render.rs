//! The projections of the distribution model. Every one of them is written here and
//! nowhere else: the release build matrix, the installer's generated metadata, the
//! installation guide, and the dataset the website renders. A surface that needs a
//! platform, an artifact name or an installation URL reads one of these.

use serde_json::{json, Value};

use super::release::{Channel, Releases};
use super::{Model, TargetStatus};

/// The placeholder the naming function leaves for the tag a release will have. The build
/// matrix carries names with this in them, so that the workflow substitutes a tag rather
/// than composing a name of its own.
pub const TAG_PLACEHOLDER: &str = "{tag}";

/// The line that opens the installer's generated region.
pub const REGION_BEGIN: &str =
    "# >>> generated from share/distribution.yaml — do not edit here >>>";

/// The line that closes it.
pub const REGION_END: &str = "# <<< generated <<<";

/// The release build matrix: one entry per published target, with the runner it is built
/// on and the artifact name the naming function derives for a tag yet to be chosen.
pub fn matrix(model: &Model) -> Value {
    let include: Vec<Value> = model
        .published()
        .map(|t| {
            let build = t
                .build
                .as_ref()
                .expect("a published target declares a build");
            json!({
                "id": t.id,
                "target": t.rust_target,
                "os": t.os,
                "arch": t.arch,
                "libc": t.libc,
                "status": t.status,
                "runner": build.runner,
                "packages": build.packages.join(" "),
                "native": build.native,
                "artifact": t.artifact_name(&model.project, &model.archive, TAG_PLACEHOLDER),
                "root": t.archive_root(&model.project, TAG_PLACEHOLDER),
            })
        })
        .collect();
    json!({
        "schema": "majordomus/distribution-matrix/v1",
        "binary": model.project.binary,
        "repository": model.project.repository,
        "checksums_file": model.archive.checksums_file,
        "include": include,
    })
}

/// The matrix as the workflow reads it.
pub fn matrix_json(model: &Model) -> String {
    let mut s = serde_json::to_string_pretty(&matrix(model)).unwrap_or_default();
    s.push('\n');
    s
}

/// The dataset the website renders: the commands to show, the platforms to list, and the
/// release the pages name. The site states none of these itself.
pub fn site_dataset(model: &Model, releases: &Releases) -> String {
    let targets: Vec<Value> = model
        .targets
        .iter()
        .map(|t| {
            json!({
                "id": t.id,
                "title": t.title(&model.titles),
                "os": t.os,
                "arch": t.arch,
                "libc": t.libc,
                "rust_target": t.rust_target,
                "status": t.status,
                "reason": t.reason,
            })
        })
        .collect();
    let latest = releases.latest_stable().map(|r| {
        json!({
            "tag": r.tag,
            "version": r.version,
            "published_at": r.published_at,
            "notes_url": r.notes_url,
            "url": model.release_url(&r.tag),
            "artifacts": r.artifacts.len(),
        })
    });
    let value = json!({
        "schema": "majordomus/distribution/v1",
        "binary": model.project.binary,
        "repository": model.project.repository,
        "installer_url": model.installer_url(),
        "install_command": model.install_command(),
        "install_and_init_command": model.install_and_init_command(),
        "next_command": format!("{} init", model.project.binary),
        "verify_command": format!("{} --version", model.project.binary),
        "install_dir": model.installer.install_dir,
        "prefix": model.installer.prefix,
        "latest_url": model.release_url(super::release::LATEST),
        "checksum": model.archive.checksum,
        "supported": model.published().count(),
        "targets": targets,
        "latest": latest,
    });
    let mut s = serde_json::to_string_pretty(&value).unwrap_or_default();
    s.push('\n');
    s
}

/// The installer's generated region: the facts the script must not restate. Shell, in the
/// subset the installer itself is written in, single-quoted so that nothing in the model
/// can be expanded by the shell that reads it.
pub fn installer_region(model: &Model) -> String {
    let mut s = String::new();
    s.push_str(&format!("{REGION_BEGIN}\n"));
    s.push_str(
        "# Every value below comes from share/distribution.yaml. Change the model and run\n",
    );
    s.push_str("# `just derive`; editing this region is undone by the next generation and refused by CI.\n");
    s.push_str(&format!("MJ_BINARY={}\n", sq(&model.project.binary)));
    s.push_str(&format!(
        "MJ_REPOSITORY={}\n",
        sq(&model.project.repository)
    ));
    s.push_str(&format!("MJ_BASE_URL={}\n", sq(&model.installer.base_url)));
    s.push_str(&format!(
        "MJ_DOWNLOAD_PREFIX={}\n",
        sq(&model.project.download_prefix())
    ));
    s.push_str(&format!(
        "MJ_ARCHIVE_EXTENSION={}\n",
        sq(&model.archive.extension)
    ));
    s.push_str(&format!(
        "MJ_DEFAULT_INSTALL_DIR=\"{}\"\n",
        model.installer.install_dir
    ));
    s.push_str(&format!(
        "MJ_DEFAULT_PREFIX=\"{}\"\n",
        model.installer.prefix
    ));
    s.push_str("\n# One line per target: <os> <arch> <libc or -> <rust target> <status>\n");
    s.push_str("MJ_TARGETS='");
    let rows: Vec<String> = model
        .targets
        .iter()
        .map(|t| {
            format!(
                "{} {} {} {} {}",
                json_str(&t.os),
                json_str(&t.arch),
                t.libc.map(|l| json_str(&l)).unwrap_or_else(|| "-".into()),
                t.rust_target,
                json_str(&t.status)
            )
        })
        .collect();
    s.push_str(&rows.join("\n"));
    s.push_str("'\n");
    s.push_str("\n# The supported platforms, as the unsupported-platform message lists them\n");
    s.push_str("MJ_SUPPORTED_TITLES='");
    let titles: Vec<String> = model.published().map(|t| t.title(&model.titles)).collect();
    s.push_str(&titles.join("\n"));
    s.push_str("'\n");
    s.push_str("\n# Why a declared target is not built: <rust target> <reason>\n");
    s.push_str("MJ_UNAVAILABLE='");
    let unavailable: Vec<String> = model
        .targets
        .iter()
        .filter(|t| t.status == TargetStatus::Unavailable)
        .map(|t| {
            format!(
                "{} {}",
                t.rust_target,
                t.reason
                    .as_deref()
                    .unwrap_or("no reason recorded")
                    .replace('\n', " ")
            )
        })
        .collect();
    s.push_str(&unavailable.join("\n"));
    s.push_str("'\n");
    s.push_str(&format!("{REGION_END}\n"));
    s
}

/// Splice the generated region into the installer template. The template owns the
/// behaviour; this owns the facts.
pub fn installer(model: &Model, template: &str) -> std::result::Result<String, String> {
    let begin = template
        .find(REGION_BEGIN)
        .ok_or_else(|| format!("the template carries no `{REGION_BEGIN}` line"))?;
    let end_at = template
        .find(REGION_END)
        .ok_or_else(|| format!("the template carries no `{REGION_END}` line"))?;
    if end_at < begin {
        return Err("the installer template's generated region closes before it opens".into());
    }
    let end = end_at + REGION_END.len() + 1;
    let mut out = String::new();
    out.push_str(&template[..begin]);
    out.push_str(&installer_region(model));
    out.push_str(&template[end.min(template.len())..]);
    Ok(out)
}

/// The installation guide: the template's prose with every derived value substituted.
pub fn install_doc(
    model: &Model,
    releases: &Releases,
    crate_version: &str,
    template: &str,
) -> std::result::Result<String, String> {
    let example_tag = releases
        .latest_stable()
        .map(|r| r.tag.clone())
        .unwrap_or_else(|| format!("v{crate_version}"));
    let example = model
        .published()
        .next()
        .map(|t| t.artifact_name(&model.project, &model.archive, &example_tag))
        .unwrap_or_default();
    let values: Vec<(&str, String)> = vec![
        ("binary", model.project.binary.clone()),
        ("repository", model.project.repository.clone()),
        ("license", model.project.license.clone()),
        ("base_url", model.installer.base_url.clone()),
        ("installer_url", model.installer_url()),
        ("install_command", model.install_command()),
        ("install_and_init_command", model.install_and_init_command()),
        ("install_dir", model.installer.install_dir.clone()),
        ("prefix", model.installer.prefix.clone()),
        ("checksum", model.archive.checksum.clone()),
        ("checksums_file", model.archive.checksums_file.clone()),
        ("latest_url", model.release_url(super::release::LATEST)),
        ("example_tag", example_tag.clone()),
        ("example_url", model.release_url(&example_tag)),
        ("example_artifact", example),
        ("supported_count", model.published().count().to_string()),
        ("platforms", platform_table(model)),
        ("unavailable", unavailable_section(model)),
        ("releases", release_table(model, releases)),
    ];
    let mut out = template.to_string();
    for (key, value) in &values {
        out = out.replace(&format!("{{{{{key}}}}}"), value);
    }
    if let Some(at) = out.find("{{") {
        let rest = &out[at..];
        let name: String = rest.chars().take(40).collect();
        return Err(format!(
            "the guide's template has an unsubstituted placeholder: {name}"
        ));
    }
    Ok(out)
}

/// The supported-platform table, derived from the model and written nowhere by hand.
pub fn platform_table(model: &Model) -> String {
    let mut s = String::from(
        "| Platform | Architecture | libc | Rust target | Status |\n|---|---|---|---|---|\n",
    );
    for t in &model.targets {
        let os = match t.os {
            super::Os::Macos => &model.titles.os.macos,
            super::Os::Linux => &model.titles.os.linux,
            super::Os::Windows => &model.titles.os.windows,
        };
        let arch = match t.arch {
            super::Arch::X86_64 => &model.titles.arch.x86_64,
            super::Arch::Aarch64 => &model.titles.arch.aarch64,
        };
        let libc = match t.libc {
            Some(super::Libc::Gnu) => model.titles.libc.gnu.as_str(),
            Some(super::Libc::Musl) => model.titles.libc.musl.as_str(),
            None => "—",
        };
        s.push_str(&format!(
            "| {os} | {arch} | {libc} | `{}` | {} |\n",
            t.rust_target,
            json_str(&t.status)
        ));
    }
    s
}

/// The reasons the unbuilt targets are unbuilt, or a sentence saying there are none.
pub fn unavailable_section(model: &Model) -> String {
    let rows: Vec<&super::Target> = model
        .targets
        .iter()
        .filter(|t| t.status == TargetStatus::Unavailable)
        .collect();
    if rows.is_empty() {
        return "Every target the model declares is built.\n".to_string();
    }
    let mut s = String::new();
    for t in rows {
        s.push_str(&format!(
            "**{}** (`{}`) — {}\n\n",
            t.title(&model.titles),
            t.rust_target,
            t.reason.as_deref().unwrap_or("no reason recorded")
        ));
    }
    s
}

/// The releases this repository has published, or a sentence saying there are none yet.
pub fn release_table(model: &Model, releases: &Releases) -> String {
    if releases.is_empty() {
        return format!(
            "No release is published yet. When one is, its metadata appears at `{}` and this table lists it.\n",
            model.release_url(super::release::LATEST)
        );
    }
    let latest = releases.latest_stable().map(|r| r.tag.clone());
    let mut s = String::from(
        "| Release | Published | Channel | Artifacts | Metadata |\n|---|---|---|---|---|\n",
    );
    for r in &releases.releases {
        let mark = if Some(&r.tag) == latest.as_ref() {
            " (latest)"
        } else if r.yanked {
            " (withdrawn)"
        } else {
            ""
        };
        s.push_str(&format!(
            "| `{}`{mark} | {} | {} | {} | [`{}.json`]({}) |\n",
            r.tag,
            &r.published_at[..10.min(r.published_at.len())],
            match r.channel {
                Channel::Stable => "stable",
                Channel::Prerelease => "prerelease",
            },
            r.artifacts.len(),
            r.tag,
            model.release_url(&r.tag)
        ));
    }
    s
}

/// A serde enum's wire name: the vocabulary is declared once, in the type.
fn json_str<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

/// A single-quoted shell word. The model's values are constrained by its schema; a quote
/// inside one would still not escape, and this makes that true rather than assumed.
fn sq(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}
