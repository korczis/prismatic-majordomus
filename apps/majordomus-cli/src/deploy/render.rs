//! The deployment artifacts, rendered from the canonical deployment object: the container
//! image definition, the ignore file that bounds its build context, and — when the object
//! names a provider (ADR 0219) — every file that provider reads, under `deploy/<provider>/`.
//! For Gigalixir those are the Procfile and the three hooks of the inline buildpack
//! ([`gigalixir_artifacts`]).
//!
//! None is authoritative and none is written by hand. Each carries a header naming the
//! object it came from and the command that regenerates it, and
//! `majordomus generate --check` refuses a tree where one of them was edited. That is the
//! whole point of the kind: the port, the routes and the build inputs are stated once, and
//! these files are views of that one statement.

use crate::generate::{Artifact, ArtifactFormat};

use super::{Deployment, ProviderName};

/// Where the image definition goes.
pub const DOCKERFILE: &str = "deploy/Dockerfile";
/// Where the build context's ignore file goes. Docker reads it from the context root, so
/// it lives at the repository root whatever the image definition's own path is.
pub const DOCKERIGNORE: &str = ".dockerignore";

/// The Alpine release both stages are built on: the builder links against its musl and the
/// runtime runs the result, so the two are one number rather than two that could disagree.
/// Pinned, because a floating tag would make the image a different thing on different days.
pub const ALPINE: &str = "3.21";
/// The repository file whose exact `channel` is the builder's Rust version. It is the pin
/// rustup, the workflows and every developer already build with; the image does not get a
/// second one (`rust:1.85` stood here while the repository built with 1.98.1).
pub const TOOLCHAIN_FILE: &str = "rust-toolchain.toml";

/// The builder image for an exact Rust version: `rust:<version>-alpine<ALPINE>`.
///
/// ```
/// use majordomus_cli::deploy::render::{rust_image, ALPINE};
///
/// assert_eq!(rust_image("1.98.1"), format!("rust:1.98.1-alpine{ALPINE}"));
/// ```
pub fn rust_image(rust: &str) -> String {
    format!("rust:{rust}-alpine{ALPINE}")
}

/// The runtime base: the same Alpine release as the builder.
///
/// ```
/// use majordomus_cli::deploy::render::{runtime_image, rust_image};
///
/// let alpine = runtime_image().strip_prefix("alpine:").unwrap().to_string();
/// assert!(rust_image("1.98.1").ends_with(&format!("-alpine{alpine}")));
/// ```
pub fn runtime_image() -> String {
    format!("alpine:{ALPINE}")
}

/// The exact Rust version a builder stage may pin, read from what the repository declares
/// in [`TOOLCHAIN_FILE`]. A channel that is not a version number (`stable`, `nightly`) or no
/// declaration at all is refused, with the correction: an image built from a floating
/// channel is not reproducible, and guessing a version would be the second pin this
/// function exists to prevent.
///
/// ```
/// use majordomus_cli::deploy::render::builder_rust;
///
/// let repo = tempfile::tempdir().unwrap();
/// assert!(builder_rust(repo.path()).is_err(), "no pin, no builder");
/// std::fs::write(repo.path().join("rust-toolchain.toml"), "[toolchain]\nchannel = \"1.98.1\"\n").unwrap();
/// assert_eq!(builder_rust(repo.path()).unwrap(), "1.98.1");
/// ```
pub fn builder_rust(root: &std::path::Path) -> Result<String, String> {
    let declared = crate::environment::toolchain::declared(root)
        .into_iter()
        .find(|t| t.id == "rust" && t.declared_by == TOOLCHAIN_FILE)
        .and_then(|t| t.declared);
    match declared {
        Some(v) if is_exact_version(&v) => Ok(v),
        Some(v) => Err(format!(
            "{TOOLCHAIN_FILE} declares the Rust channel `{v}`, which is not an exact version; \
             the deployment's builder stage pins the toolchain the repository builds with, so \
             set `channel = \"<major>.<minor>.<patch>\"` there"
        )),
        None => Err(format!(
            "the deployment's builder stage pins the Rust toolchain the repository builds with, \
             and this repository declares none: add {TOOLCHAIN_FILE} with a `[toolchain]` \
             table whose `channel` is \"<major>.<minor>.<patch>\""
        )),
    }
}

/// `1.98.1`: three dot-separated numbers, nothing else.
fn is_exact_version(v: &str) -> bool {
    let parts: Vec<&str> = v.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
}
/// The build argument that carries the commit into the builder stage. It is the variable the
/// crate's build script reads, so an image built with it answers the commit on its health
/// routes and one built without it answers `unknown`.
pub const COMMIT_ARG: &str = "MAJORDOMUS_BUILD_COMMIT";
/// The build argument that says whether that commit's tree was dirty (`true`/`false`).
pub const DIRTY_ARG: &str = "MAJORDOMUS_BUILD_DIRTY";
/// The unprivileged user the deployed process runs as.
pub const RUN_USER: &str = "majordomus";
/// Where the layer and the tool distribution live inside the image.
pub const APP_DIR: &str = "/app";
/// The layer's checkout-local half, relative to [`APP_DIR`]: never in the build context
/// (the ignore file excludes it), and the one directory the deployed process writes.
pub const LOCAL_DIR: &str = ".ai/local";

/// The comment header every generated artifact carries: what wrote it, from what, and how
/// to change it. A contributor who edits the output is told which file to change instead.
fn header(comment: &str, source: &str, what: &str) -> String {
    // The first line is the one every generated artifact of this repository opens with;
    // `generate --check` reads exactly that, and this renderer predates the contract.
    format!(
        "{comment} {header}\n\
         {comment} {what}\n\
         {comment} GENERATED by `majordomus generate deployment` from {source}.\n\
         {comment} Do not edit: `majordomus generate --check` refuses a hand edit. Change\n\
         {comment} the deployment object and regenerate.\n\n",
        header = crate::generate::HEADER
    )
}

/// The directory of the package the builder stage compiles: the one input that is a build
/// input rather than something the running process reads.
fn package_dir(d: &Deployment) -> Option<&str> {
    d.build
        .inputs
        .iter()
        .map(String::as_str)
        .find(|p| p.rsplit('/').next() == Some(d.build.package.as_str()))
}

/// The inputs the running process reads: everything the build context carries except the
/// package the builder compiles. The slug a provider receives carries exactly these, since
/// its executable is built before it is assembled ([`super::slug`]).
pub(crate) fn runtime_inputs(d: &Deployment) -> Vec<&str> {
    let pkg = package_dir(d);
    d.build
        .inputs
        .iter()
        .map(String::as_str)
        .filter(|p| Some(*p) != pkg)
        .collect()
}

/// The stages of the builder, in order, before the runtime stage. Each is a layer cache
/// boundary, and what invalidates each is the point of the split:
///
/// - `toolchain`: the pinned Rust image, the C library headers and sccache, which every
///   later `cargo` runs behind (see [`SCCACHE_DIR`]); only a toolchain pin moves it.
/// - `recipe`: the package's manifest and lockfile with the package's own version masked
///   and its bench, test and example targets dropped. Its output, not the files it reads,
///   is what the next stage copies, so a release that bumps only the version reaches the
///   same recipe and does not recompile a dependency.
/// - `deps`: every dependency downloaded and compiled against stub sources, in the profile,
///   features and target of the real build. It is ordinary layer content — unlike a
///   `--mount=type=cache` directory, which a registry cache export never carries — so a
///   fresh runner that imports the cache gets the compiled dependencies too.
/// - `build`: the real sources over the compiled dependencies; only the package itself
///   compiles. The commit arguments live here and nowhere earlier, so a new commit never
///   reaches a dependency.
/// - `release`: a release's own published executable, taken from the `release-binary` build
///   context ([`RELEASE_CONTEXT`]) instead of compiled; see [`BINARY_ARG`].
/// - `binary`: whichever of `build` and `release` [`BINARY_ARG`] names — the one stage the
///   runtime takes its executable from, so the runtime is the same whatever made the binary.
pub const BUILDER_STAGES: [&str; 6] = ["toolchain", "recipe", "deps", "build", "release", "binary"];

/// The build argument that chooses where the image's executable comes from: `build` (the
/// default) compiles it from the context, `release` takes the release's own published,
/// checksummed and attested executable (`.github/workflows/release.yml`). BuildKit builds only
/// the stages the final image reaches, so with `release` nothing is compiled at all, and the
/// image a release publishes runs the very bytes its archives ship.
pub const BINARY_ARG: &str = "MAJORDOMUS_BINARY_FROM";
/// The named build context a `release` build is given: a directory holding
/// `majordomus-<arch>` for each platform (`amd64`, `arm64`), BuildKit's TARGETARCH names.
pub const RELEASE_CONTEXT: &str = "release-binary";

/// Where sccache keeps compiled units inside the builder: a BuildKit cache mount, never layer
/// content. Two caches with two jobs, because neither does the other's:
///
/// - the `deps` *layer* travels with a registry cache export, so a fresh runner gets every
///   dependency compiled while the lockfile is unchanged — and loses all of them the moment it
///   changes;
/// - the sccache *mount* is keyed by each compilation's own inputs, so when the lockfile moves
///   only the crates that actually changed recompile, and it is shared by every build on one
///   builder whatever directory it was started from — every worktree and branch on a machine.
///   A mount never leaves its builder: on an ephemeral runner the workflow carries it in and
///   out explicitly, or it starts empty, which is slower and never wrong.
///
/// A unit sccache cannot see the whole of is compiled, not guessed: the package itself, whose
/// sources and commit change with every build, is a miss by construction, and the image's
/// smoke test checks the commit it names against the one it was built from.
pub const SCCACHE_DIR: &str = "/sccache";
/// The id of the sccache mount, the name under which a builder keeps it between builds and
/// the name a workflow restores it by.
pub const SCCACHE_MOUNT_ID: &str = "majordomus-sccache";
/// The bound sccache evicts to: the dependency graph compiled for one profile and target is
/// well under it, and a bound is what keeps a long-lived builder's disk from growing forever.
/// The builder also sets `SCCACHE_IDLE_TIMEOUT=0`: sccache's server stops after 600 s without
/// a new request, and the package's own release compile alone ran 612 s on a loaded builder,
/// so the server stopped under it and the wrapper compiled the package a second time.
pub const SCCACHE_SIZE: &str = "2G";

/// The container image, as a multi-stage build: builder stages that hold the toolchain
/// ([`BUILDER_STAGES`]) and a runtime that holds the binary, the layer and nothing else.
/// The runtime carries no compiler, no cargo registry, no repository history and no shell
/// history — what is not copied in cannot leak out. `rust` is the exact version the builder
/// pins, from [`builder_rust`].
pub fn dockerfile(d: &Deployment, source: &str, rust: &str) -> String {
    let mut out = header(
        "#",
        source,
        "The container image of this repository's deployment.",
    );
    out.push_str(&format!(
        "# Where the executable comes from: `build` compiles it below; `release` takes a\n\
         # release's published executable from the {RELEASE_CONTEXT} build context instead.\n\
         ARG {BINARY_ARG}=build\n\n"
    ));
    let pkg = package_dir(d).unwrap_or("apps");
    let lib = d.build.package.replace('-', "_");
    // The recipe's two programs, kept out of `format!` so the shell's braces stay single.
    // Cargo.toml: the [package] version becomes 0.0.0 and every [[bench]], [[test]] and
    // [[example]] table is dropped (cargo refuses a manifest whose target files are absent,
    // and none of them is part of `cargo build --bin`). Cargo.lock: the version recorded
    // for the package itself becomes 0.0.0, so `--locked` agrees with the masked manifest.
    let manifest_awk = r#"/^\[/ { t = $0; skip = (t ~ /^\[\[(bench|test|example)\]\]$/) } t == "[package]" && /^version *=/ { $0 = "version = \"0.0.0\"" } !skip"#;
    // The path dependencies the package compiles against, in the dependency stage: cargo
    // reads their manifests to resolve the graph, so they are there before the first build.
    let crates: String = d
        .build
        .crates
        .iter()
        .map(|c| format!("COPY {c} ./{c}\n"))
        .collect();
    let lock_awk = format!(
        r#"p && /^version = / {{ $0 = "version = \"0.0.0\""; p = 0 }} $0 == "name = \"{package}\"" {{ p = 1 }} 1"#,
        package = d.build.package
    );
    out.push_str(&format!(
        "FROM {image} AS toolchain\n\
         RUN apk add --no-cache musl-dev sccache\n\
         ENV RUSTC_WRAPPER=sccache SCCACHE_DIR={SCCACHE_DIR} SCCACHE_CACHE_SIZE={SCCACHE_SIZE} SCCACHE_IDLE_TIMEOUT=0 CARGO_INCREMENTAL=0\n\
         WORKDIR /src\n\n\
         # The dependency graph and nothing else; see BUILDER_STAGES in the renderer.\n\
         FROM toolchain AS recipe\n\
         COPY {pkg}/Cargo.toml {pkg}/Cargo.lock /recipe/in/\n\
         RUN mkdir -p /recipe/out \\\n\
         \x20 && awk '{manifest_awk}' /recipe/in/Cargo.toml > /recipe/out/Cargo.toml \\\n\
         \x20 && awk '{lock_awk}' /recipe/in/Cargo.lock > /recipe/out/Cargo.lock\n\n\
         # Every dependency compiled, as layer content a registry cache can carry.\n\
         FROM toolchain AS deps\n\
         COPY --from=recipe /recipe/out/ ./{pkg}/\n\
         {crates}\
         RUN --mount=type=cache,id={SCCACHE_MOUNT_ID},target={SCCACHE_DIR} mkdir -p {pkg}/src \\\n\
         \x20 && echo 'fn main() {{}}' > {pkg}/src/main.rs && echo 'fn main() {{}}' > {pkg}/build.rs \\\n\
         \x20 && touch {pkg}/src/lib.rs \\\n\
         \x20 && cargo build --locked --{profile} --manifest-path {pkg}/Cargo.toml --package {package} --bin {binary} \\\n\
         \x20 && rm -rf {pkg}/src {pkg}/build.rs {pkg}/target/{profile}/.fingerprint/{package}-* \\\n\
         \x20   {pkg}/target/{profile}/build/{package}-* {pkg}/target/{profile}/deps/{lib}-* \\\n\
         \x20   {pkg}/target/{profile}/deps/lib{lib}-* {pkg}/target/{profile}/deps/{binary}-* \\\n\
         \x20   {pkg}/target/{profile}/{binary} {pkg}/target/{profile}/{binary}.d \\\n\
         \x20 && sccache --show-stats\n\n\
         FROM deps AS build\n\
         COPY {pkg} ./{pkg}\n\
         # The commit this image is built from, compiled into the executable so that\n\
         # /api/v1/live and /api/v1/ready can name it. The build context carries no .git,\n\
         # so the builder cannot ask git and the build must say it:\n\
         #   docker build --build-arg {COMMIT_ARG}=\"$(git rev-parse HEAD)\" ...\n\
         # Left unset, the deployed process answers `unknown` rather than a guess. Declared\n\
         # here and in no earlier stage, so a new commit never invalidates a dependency.\n\
         ARG {COMMIT_ARG}=unknown\n\
         ARG {DIRTY_ARG}=unknown\n\
         RUN --mount=type=cache,id={SCCACHE_MOUNT_ID},target={SCCACHE_DIR} \\\n\
         \x20 cargo build --locked --offline --{profile} --manifest-path {pkg}/Cargo.toml --package {package} --bin {binary} \\\n\
         \x20 && install -Dm755 {pkg}/target/{profile}/{binary} /out/{binary} && sccache --show-stats\n\n\
         # A release's own executable, published, checksummed and attested, compiled by nothing\n\
         # here: --build-arg {BINARY_ARG}=release --build-context {RELEASE_CONTEXT}=<dir>, where\n\
         # <dir> holds {binary}-amd64 and {binary}-arm64.\n\
         FROM scratch AS release\n\
         ARG TARGETARCH\n\
         COPY --from={RELEASE_CONTEXT} {binary}-${{TARGETARCH}} /out/{binary}\n\n\
         FROM ${{{BINARY_ARG}}} AS binary\n\n",
        image = rust_image(rust),
        profile = d.build.profile,
        package = d.build.package,
        binary = d.build.binary,
    ));
    out.push_str(&format!(
        "FROM {runtime}\n\
         # An unprivileged user, which is also why the declared port is above 1024.\n\
         RUN adduser -D -H -u 10001 {RUN_USER}\n\
         WORKDIR {APP_DIR}\n\
         COPY --from=binary /out/{binary} /usr/local/bin/{binary}\n",
        runtime = runtime_image(),
        binary = d.build.binary,
    ));
    for input in runtime_inputs(d) {
        out.push_str(&format!("COPY {input} {APP_DIR}/{input}\n"));
    }
    if runtime_inputs(d).contains(&".ai") {
        // The served layer's checkout-local half is the one place the process writes (its
        // server lease and state): the build context never carries it, and everything copied
        // above is root's, so without this the unprivileged process stopped at start with
        // `/app/.ai/local/state/mcp: Permission denied` and the image never served.
        out.push_str(&format!(
            "# The one directory the process writes: the layer's checkout-local half.\n\
             RUN install -d -o {RUN_USER} -g {RUN_USER} {APP_DIR}/{LOCAL_DIR}\n"
        ));
    }
    out.push_str(&format!(
        "USER {RUN_USER}\n\
         EXPOSE {port}\n\
         # The address, the port and the interface are the object's; this command names the\n\
         # deployment and reads them from it. Discovery is a filesystem walk because the\n\
         # image carries the layer without the repository that tracks it.\n\
         ENTRYPOINT [\"{binary}\"]\n\
         CMD [\"serve\", \"--discovery\", \"filesystem\", \"--deployment\", \"{id}\"]\n",
        port = d.listen.port,
        binary = d.build.binary,
        id = d.id,
    ));
    out
}

/// The build context: nothing but the declared inputs. Everything is excluded and each
/// input is let back in, so a file that appears in the repository tomorrow does not appear
/// in the image tomorrow.
pub fn dockerignore(d: &Deployment, source: &str) -> String {
    let mut out = header(
        "#",
        source,
        "What the deployment's build context carries: only the declared inputs.",
    );
    out.push_str("*\n");
    for input in d.build.inputs.iter().chain(&d.build.crates) {
        out.push_str(&format!("!{input}\n"));
    }
    // The image definition is not an input of the deployment; it is a view of it. It is
    // let back in because a builder that reads the context needs the file it builds from.
    out.push_str(&format!("!{DOCKERFILE}\n"));
    out.push_str(
        "\n# and never these, wherever they appear inside an input\n\
         **/.git\n\
         **/target\n\
         **/node_modules\n\
         **/.DS_Store\n\
         .ai/local\n",
    );
    out
}

// ---------------------------------------------------------------- Gigalixir

/// Where the Gigalixir files go: the provider's own directory, never the repository root,
/// because what is pushed to the provider is assembled (ADR 0219) and the repository is not.
pub const GIGALIXIR_DIR: &str = "deploy/gigalixir";
/// The Procfile: the start command, the interface and the port the provider injects.
pub const GIGALIXIR_PROCFILE: &str = "deploy/gigalixir/Procfile";
/// The inline buildpack's detect hook: it names the application and accepts it.
pub const GIGALIXIR_DETECT: &str = "deploy/gigalixir/bin/detect";
/// The inline buildpack's compile hook: it compiles nothing, it makes the prebuilt
/// executable runnable and proves it starts.
pub const GIGALIXIR_COMPILE: &str = "deploy/gigalixir/bin/compile";
/// The inline buildpack's release hook: no add-ons, no default process beyond the Procfile.
pub const GIGALIXIR_RELEASE: &str = "deploy/gigalixir/bin/release";
/// Where the assembled slug carries the deployment's executable, relative to the slug root:
/// a directory of its own, so the executable never shadows a file of the layer it serves.
pub const SLUG_BIN_DIR: &str = ".majordomus/bin";
/// The buildpack that runs the slug's own hooks, pinned by commit so a push builds with the
/// same code every day. A buildpack list cannot carry a comment, so it is not rendered as a
/// file; the slug assembler writes this line ([`buildpack`]).
pub const INLINE_BUILDPACK: &str =
    "https://github.com/kr/heroku-buildpack-inline#d094b444d6530dcc38dccf5d7844d6990380a08b";

/// The buildpack line a provider's slug is built with: for Gigalixir, the inline buildpack
/// pinned by commit, which runs the generated hooks and compiles nothing.
///
/// ```
/// use majordomus_cli::deploy::{render::{buildpack, INLINE_BUILDPACK}, ProviderName};
///
/// assert_eq!(buildpack(ProviderName::Gigalixir), INLINE_BUILDPACK);
/// let (_, commit) = INLINE_BUILDPACK.split_once('#').unwrap();
/// assert_eq!(commit.len(), 40, "pinned by a full commit, never a branch");
/// ```
pub fn buildpack(provider: ProviderName) -> &'static str {
    match provider {
        ProviderName::Gigalixir => INLINE_BUILDPACK,
    }
}

/// Where the slug carries the deployment's executable: [`SLUG_BIN_DIR`] and the object's
/// `build.binary`.
///
/// ```
/// # let d: majordomus_cli::deploy::Deployment = serde_json::from_value(serde_json::json!({
/// #     "schema": "deployment/v1", "kind": "deployment", "id": "x", "title": "x",
/// #     "application": "x",
/// #     "build": { "package": "majordomus-cli", "binary": "majordomus", "profile": "release",
/// #                "inputs": ["share"] },
/// #     "listen": { "port": 8080, "interface": "all" },
/// #     "health": { "liveness": "/api/v1/live", "readiness": "/api/v1/ready" },
/// #     "resources": { "cpu_kind": "shared", "cpus": 1, "memory_mb": 256 },
/// #     "machines": { "count": 1, "min_running": 0, "autostart": true, "autostop": true },
/// #     "region": "fra",
/// # })).unwrap();
/// use majordomus_cli::deploy::render::slug_executable;
///
/// assert_eq!(slug_executable(&d), ".majordomus/bin/majordomus");
/// ```
pub fn slug_executable(d: &Deployment) -> String {
    format!("{SLUG_BIN_DIR}/{}", d.build.binary)
}

/// The secret environment variable a hosted hub's node key is read from (ADR 0220 decision
/// 2): set on the provider by the owner, named in the start command, never written to the
/// slug, a generated file or a log.
pub const NODE_SEED_ENV: &str = "MAJORDOMUS_MESH_NODE_SEED";

/// The Procfile. The provider injects the port as `PORT`; the object's own port is the
/// fallback, so the process binds the declared one wherever `PORT` is absent. The interface
/// is the object's, and discovery is a filesystem walk because the slug carries the layer
/// without the repository that tracks it. On the mesh the process advertises the provider
/// URL the object names and takes its node key from [`NODE_SEED_ENV`] (ADR 0220): without
/// that secret its mesh stays off, and the server still answers.
///
/// ```
/// # let d: majordomus_cli::deploy::Deployment = serde_json::from_value(serde_json::json!({
/// #     "schema": "deployment/v1", "kind": "deployment", "id": "x", "title": "x",
/// #     "application": "x",
/// #     "build": { "package": "majordomus-cli", "binary": "majordomus", "profile": "release",
/// #                "inputs": ["share"] },
/// #     "listen": { "port": 8080, "interface": "all" },
/// #     "health": { "liveness": "/api/v1/live", "readiness": "/api/v1/ready" },
/// #     "resources": { "cpu_kind": "shared", "cpus": 1, "memory_mb": 256 },
/// #     "machines": { "count": 1, "min_running": 0, "autostart": true, "autostop": true },
/// #     "region": "fra",
/// #     "provider": { "name": "gigalixir", "app": "x-app", "stack": "gigalixir-24",
/// #                   "url": "https://x-app.gigalixirapp.com" },
/// # })).unwrap();
/// use majordomus_cli::deploy::render::gigalixir_procfile;
///
/// let web = gigalixir_procfile(&d, ".ai/repo/deployments/x.yaml");
/// let line = web.lines().find(|l| l.starts_with("web: ")).unwrap();
/// assert!(line.contains("serve --repo . --discovery filesystem"), "{line}");
/// assert!(line.contains("--port ${PORT:-8080}"), "{line}");
/// assert!(line.ends_with("--advertise https://x-app.gigalixirapp.com --node-seed-env MAJORDOMUS_MESH_NODE_SEED"), "{line}");
/// ```
pub fn gigalixir_procfile(d: &Deployment, source: &str) -> String {
    let mut out = header(
        "#",
        source,
        "The Gigalixir Procfile of this repository's deployment.",
    );
    out.push_str(&format!(
        "web: ./{exe} serve --repo . --discovery filesystem --host {host} --port ${{PORT:-{port}}} --idle 0{mesh}\n",
        exe = slug_executable(d),
        host = d.listen.interface.host(),
        port = d.listen.port,
        mesh = d
            .provider
            .as_ref()
            .map(|p| format!(" --advertise {} --node-seed-env {NODE_SEED_ENV}", p.url.trim_end_matches('/')))
            .unwrap_or_default(),
    ));
    out
}

/// One hook of the inline buildpack: the `#!` line first, because the kernel reads it there,
/// and the provenance header on the line after (`generate::opens_with_banner` reserves the
/// first line of a script for it).
fn hook(source: &str, what: &str, body: &str) -> String {
    format!("#!/bin/sh\n{}{body}", header("#", source, what))
}

/// The detect hook: it accepts the slug and names it.
///
/// ```
/// use majordomus_cli::deploy::render::gigalixir_detect;
///
/// let hook = gigalixir_detect(".ai/repo/deployments/x.yaml");
/// assert!(hook.starts_with("#!/bin/sh\n"), "the kernel reads the interpreter on line one");
/// assert!(hook.ends_with("echo majordomus\n"));
/// ```
pub fn gigalixir_detect(source: &str) -> String {
    hook(
        source,
        "The inline buildpack's detect hook: it accepts the slug and names it.",
        "echo majordomus\n",
    )
}

/// The compile hook: the executable is prebuilt at the commit the slug was read from, so
/// this compiles nothing; it makes the executable runnable and proves it starts. `$1` is the
/// build directory the buildpack hands it.
///
/// ```
/// # let d: majordomus_cli::deploy::Deployment = serde_json::from_value(serde_json::json!({
/// #     "schema": "deployment/v1", "kind": "deployment", "id": "x", "title": "x",
/// #     "application": "x",
/// #     "build": { "package": "majordomus-cli", "binary": "majordomus", "profile": "release",
/// #                "inputs": ["share"] },
/// #     "listen": { "port": 8080, "interface": "all" },
/// #     "health": { "liveness": "/api/v1/live", "readiness": "/api/v1/ready" },
/// #     "resources": { "cpu_kind": "shared", "cpus": 1, "memory_mb": 256 },
/// #     "machines": { "count": 1, "min_running": 0, "autostart": true, "autostop": true },
/// #     "region": "fra",
/// #     "provider": { "name": "gigalixir", "app": "x-app", "stack": "gigalixir-24",
/// #                   "url": "https://x-app.gigalixirapp.com" },
/// # })).unwrap();
/// use majordomus_cli::deploy::render::gigalixir_compile;
///
/// let hook = gigalixir_compile(&d, ".ai/repo/deployments/x.yaml");
/// assert!(hook.starts_with("#!/bin/sh\n"));
/// assert!(hook.contains("chmod 755 \"$1/.majordomus/bin/majordomus\""));
/// assert!(hook.contains("\"$1/.majordomus/bin/majordomus\" --version"));
/// assert!(!hook.contains("cargo"), "the executable is prebuilt; nothing compiles here");
/// ```
pub fn gigalixir_compile(d: &Deployment, source: &str) -> String {
    let exe = slug_executable(d);
    hook(
        source,
        "The inline buildpack's compile hook: it compiles nothing and proves the executable runs.",
        &format!("chmod 755 \"$1/{exe}\"\n\"$1/{exe}\" --version\n"),
    )
}

/// The release hook: no add-ons and no process beyond the Procfile's.
///
/// ```
/// use majordomus_cli::deploy::render::gigalixir_release;
///
/// let hook = gigalixir_release(".ai/repo/deployments/x.yaml");
/// assert!(hook.starts_with("#!/bin/sh\n"));
/// assert!(hook.ends_with("echo \"--- {}\"\n"), "an empty release: no add-ons, no processes");
/// ```
pub fn gigalixir_release(source: &str) -> String {
    hook(
        source,
        "The inline buildpack's release hook: no add-ons, no process beyond the Procfile.",
        "echo \"--- {}\"\n",
    )
}

/// The four Gigalixir artifacts of one deployment.
///
/// ```
/// # let d: majordomus_cli::deploy::Deployment = serde_json::from_value(serde_json::json!({
/// #     "schema": "deployment/v1", "kind": "deployment", "id": "x", "title": "x",
/// #     "application": "x",
/// #     "build": { "package": "majordomus-cli", "binary": "majordomus", "profile": "release",
/// #                "inputs": ["share"] },
/// #     "listen": { "port": 8080, "interface": "all" },
/// #     "health": { "liveness": "/api/v1/live", "readiness": "/api/v1/ready" },
/// #     "resources": { "cpu_kind": "shared", "cpus": 1, "memory_mb": 256 },
/// #     "machines": { "count": 1, "min_running": 0, "autostart": true, "autostop": true },
/// #     "region": "fra",
/// #     "provider": { "name": "gigalixir", "app": "x-app", "stack": "gigalixir-24",
/// #                   "url": "https://x-app.gigalixirapp.com" },
/// # })).unwrap();
/// use majordomus_cli::deploy::render::{gigalixir_artifacts, GIGALIXIR_PROCFILE, GIGALIXIR_DETECT,
///     GIGALIXIR_COMPILE, GIGALIXIR_RELEASE};
///
/// let all = gigalixir_artifacts(&d, ".ai/repo/deployments/x.yaml");
/// let paths: Vec<&str> = all.iter().map(|a| a.path.as_str()).collect();
/// assert_eq!(paths, [GIGALIXIR_PROCFILE, GIGALIXIR_DETECT, GIGALIXIR_COMPILE, GIGALIXIR_RELEASE]);
/// assert!(all.iter().all(|a| a.source == ".ai/repo/deployments/x.yaml"));
/// ```
pub fn gigalixir_artifacts(d: &Deployment, source: &str) -> Vec<Artifact> {
    [
        (GIGALIXIR_PROCFILE, gigalixir_procfile(d, source)),
        (GIGALIXIR_DETECT, gigalixir_detect(source)),
        (GIGALIXIR_COMPILE, gigalixir_compile(d, source)),
        (GIGALIXIR_RELEASE, gigalixir_release(source)),
    ]
    .into_iter()
    .map(|(path, content)| text_artifact(path, source, content))
    .collect()
}

fn text_artifact(path: &str, source: &str, content: String) -> Artifact {
    Artifact {
        path: path.into(),
        document: "deployment".into(),
        format: ArtifactFormat::Text,
        schema: None,
        source: source.into(),
        content,
    }
}

/// The directory a provider's files are rendered under, repository-relative; the slug
/// assembler strips it, so the provider reads each file at the slug's root.
pub(crate) fn provider_dir(provider: ProviderName) -> &'static str {
    match provider {
        ProviderName::Gigalixir => GIGALIXIR_DIR,
    }
}

/// The files of the provider a deployment names, or none when it names none.
pub(crate) fn provider_artifacts(d: &Deployment, source: &str) -> Vec<Artifact> {
    match d.provider.as_ref().map(|p| p.name) {
        Some(ProviderName::Gigalixir) => gigalixir_artifacts(d, source),
        None => Vec::new(),
    }
}

/// Every artifact of one deployment: the image definition and its ignore file, and the
/// files of the provider it names; `rust` is the builder's exact Rust version.
pub fn artifacts(d: &Deployment, source: &str, rust: &str) -> Vec<Artifact> {
    let mut out = vec![
        text_artifact(DOCKERFILE, source, dockerfile(d, source, rust)),
        text_artifact(DOCKERIGNORE, source, dockerignore(d, source)),
    ];
    out.extend(provider_artifacts(d, source));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deployment() -> Deployment {
        serde_json::from_value(serde_json::json!({
            "schema": "deployment/v1",
            "kind": "deployment",
            "id": "example",
            "title": "The example deployment",
            "application": "example",
            "build": {
                "package": "majordomus-cli",
                "binary": "majordomus",
                "profile": "release",
                "inputs": ["apps/majordomus-cli", "share"],
            },
            "listen": { "port": 8080, "interface": "all" },
            "health": { "liveness": "/api/v1/live", "readiness": "/api/v1/ready" },
            "resources": { "cpu_kind": "shared", "cpus": 1, "memory_mb": 256 },
            "machines": { "count": 1, "min_running": 0, "autostart": true, "autostop": true },
            "region": "fra",
        }))
        .expect("the example deployment parses")
    }

    /// The lines of one stage, from its `FROM` to the next.
    fn stage<'a>(file: &'a str, name: &str) -> Vec<&'a str> {
        let mut lines = file
            .lines()
            .skip_while(|l| !l.ends_with(&format!(" AS {name}")));
        let first = lines.next().into_iter();
        first
            .chain(lines.take_while(|l| !l.starts_with("FROM ")))
            .collect()
    }

    #[test]
    fn the_builder_pins_the_version_it_is_given_and_the_runtime_the_same_alpine() {
        let file = dockerfile(&deployment(), "x.yaml", "1.98.1");
        assert!(file.contains("FROM rust:1.98.1-alpine3.21 AS toolchain\n"));
        assert!(file.contains("\nFROM alpine:3.21\n"));
        // the version appears once: a second statement of it is the drift being removed
        assert_eq!(file.matches("1.98.1").count(), 1);
        let other = dockerfile(&deployment(), "x.yaml", "1.99.0");
        assert!(other.contains("FROM rust:1.99.0-alpine3.21 AS toolchain\n"));
    }

    #[test]
    fn the_stages_are_the_declared_ones_in_order() {
        let file = dockerfile(&deployment(), "x.yaml", "1.98.1");
        let named: Vec<&str> = file
            .lines()
            .filter_map(|l| l.strip_prefix("FROM "))
            .filter_map(|l| l.split(" AS ").nth(1))
            .collect();
        assert_eq!(named, BUILDER_STAGES);
    }

    #[test]
    fn nothing_before_the_build_stage_reads_a_source_file_or_the_commit() {
        let file = dockerfile(&deployment(), "x.yaml", "1.98.1");
        for name in ["toolchain", "recipe", "deps"] {
            for line in stage(&file, name) {
                assert!(
                    !line.contains(COMMIT_ARG) && !line.contains(DIRTY_ARG),
                    "{name} names the commit: {line}"
                );
                if let Some(copy) = line.strip_prefix("COPY ") {
                    assert!(
                        copy.starts_with("--from=recipe ")
                            || copy.starts_with(
                                "apps/majordomus-cli/Cargo.toml apps/majordomus-cli/Cargo.lock "
                            ),
                        "{name} copies more than the dependency graph: {line}"
                    );
                }
            }
        }
        let build = stage(&file, "build");
        assert!(build.contains(&"COPY apps/majordomus-cli ./apps/majordomus-cli"));
        assert!(build
            .iter()
            .any(|l| *l == format!("ARG {COMMIT_ARG}=unknown")));
    }

    /// Every literal `include_str!` or `include_bytes!` of the non-test code under `src` whose
    /// file does not resolve under one of `copied`, as `<source>: <embedded path>`. A path is
    /// resolved against the file that names it, as the compiler resolves it, and from a
    /// file's first test module on nothing is read, since the image compiles no test.
    fn embedded_outside(src: &std::path::Path, copied: &[std::path::PathBuf]) -> Vec<String> {
        let mut missing = Vec::new();
        let mut dirs = vec![src.to_path_buf()];
        while let Some(dir) = dirs.pop() {
            for entry in std::fs::read_dir(&dir).expect("the source tree reads") {
                let path = entry.expect("a source entry reads").path();
                if path.is_dir() {
                    dirs.push(path);
                    continue;
                }
                if path.extension().is_none_or(|e| e != "rs") {
                    continue;
                }
                let text = std::fs::read_to_string(&path).expect("a source file reads");
                // From the first test module on, the file compiles only under test.
                let compiled = text.split("#[cfg(test)]").next().unwrap_or_default();
                for marker in ["include_str!(\"", "include_bytes!(\""] {
                    for (at, _) in compiled.match_indices(marker) {
                        let rest = &compiled[at + marker.len()..];
                        let embedded = &rest[..rest.find('"').unwrap_or(rest.len())];
                        let carried = dir
                            .join(embedded)
                            .canonicalize()
                            .is_ok_and(|t| copied.iter().any(|c| t.starts_with(c)));
                        if !carried {
                            missing.push(format!("{}: {embedded}", path.display()));
                        }
                    }
                }
            }
        }
        missing
    }

    /// Every file the crate embeds is one the image's compiling stages copy: `deps` and
    /// `build` are given the package and its path crates, nothing else of the repository. A
    /// test module may read the repository, since the image compiles no test. Train 6
    /// embedded `share/mesh-cache-provider/` and master's image stopped compiling (run
    /// 40189935629); a pull request builds the image only when the image's own definition
    /// changes, so it is caught here (I4229, from #2523).
    #[test]
    fn the_build_stage_carries_every_file_the_package_embeds() {
        let package = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .canonicalize()
            .expect("the package directory resolves");
        let root = package
            .join("../..")
            .canonicalize()
            .expect("the repository resolves");
        let file = dockerfile(&deployment(), "x.yaml", "1.98.1");
        let copied: Vec<std::path::PathBuf> = ["deps", "build"]
            .iter()
            .flat_map(|name| stage(&file, name))
            .filter_map(|l| l.strip_prefix("COPY "))
            .filter(|l| !l.starts_with("--"))
            .filter_map(|l| l.split(' ').next())
            .map(|from| root.join(from))
            .collect();
        let missing = embedded_outside(&package.join("src"), &copied);
        assert!(
            missing.is_empty(),
            "embedded, not copied by the build stage: {missing:?}"
        );
    }

    /// The guard above fails on the defect it exists for: a source that embeds a file of the
    /// repository's `share/`, which the build stage does not copy, is named with that path,
    /// while an embed inside the package and one in a test module are not (I4229).
    #[test]
    fn an_embed_the_build_stage_does_not_copy_is_named_with_its_source_and_path() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let root = dir.path().canonicalize().expect("the directory resolves");
        let package = root.join("apps/majordomus-cli");
        std::fs::create_dir_all(package.join("src")).expect("the source tree is written");
        std::fs::create_dir_all(root.join("share")).expect("share is written");
        std::fs::write(root.join("share/advisors.yaml"), "advisors: []\n").expect("written");
        std::fs::write(package.join("src/schema.json"), "{}\n").expect("written");
        let source = package.join("src/advisors.rs");
        std::fs::write(
            &source,
            concat!(
                "const ADVISORS: &str = include_str!(\"../../../share/advisors.yaml\");\n",
                "const SCHEMA: &[u8] = include_bytes!(\"schema.json\");\n",
                "#[cfg(test)]\n",
                "const FIXTURE: &str = include_str!(\"../../../share/advisors.yaml\");\n",
            ),
        )
        .expect("written");
        let missing = embedded_outside(&package.join("src"), std::slice::from_ref(&package));
        assert_eq!(
            missing,
            [format!(
                "{}: ../../../share/advisors.yaml",
                source.display()
            )]
        );
        // The repository's share is carried once a compiling stage copies it.
        assert!(embedded_outside(&package.join("src"), &[package, root.join("share")]).is_empty());
    }

    #[test]
    fn the_compiled_dependencies_are_layer_content_and_sccache_is_the_only_mount() {
        // a registry cache export carries layers and never a cache mount's contents, which
        // is the whole reason the dependencies are compiled into a stage of their own; the
        // sccache mount sits beside that layer, and the target directory is never mounted
        let file = dockerfile(&deployment(), "x.yaml", "1.98.1");
        let mount = format!("--mount=type=cache,id={SCCACHE_MOUNT_ID},target={SCCACHE_DIR} ");
        assert_eq!(file.matches("--mount=type=cache").count(), 2);
        assert_eq!(file.matches(&mount).count(), 2);
        for name in ["deps", "build"] {
            assert!(
                stage(&file, name).join("\n").contains(&mount),
                "{name} has no sccache"
            );
        }
        assert!(stage(&file, "toolchain")
            .join("\n")
            .contains("ENV RUSTC_WRAPPER=sccache SCCACHE_DIR=/sccache"));
        // a server that idles out under one long compile makes the wrapper compile it twice
        assert!(stage(&file, "toolchain")
            .join("\n")
            .contains(" SCCACHE_IDLE_TIMEOUT=0 "));
        let deps = stage(&file, "deps").join("\n");
        assert!(deps.contains("cargo build --locked --release"));
        assert!(stage(&file, "build")
            .join("\n")
            .contains("cargo build --locked --offline --release"));
    }

    /// A path dependency is a dependency: the dependency stage compiles it, so it is in the
    /// build context and in that stage before the first `cargo build`, and the running
    /// process never reads it, so the runtime image does not carry it.
    #[test]
    fn a_path_crate_is_in_the_context_and_the_dependency_stage_and_not_the_runtime() {
        let mut d = deployment();
        d.build.crates = vec!["apps/majordomus-core".to_string()];
        let file = dockerfile(&d, "x.yaml", "1.98.1");
        let deps = stage(&file, "deps");
        let copy = deps
            .iter()
            .position(|l| *l == "COPY apps/majordomus-core ./apps/majordomus-core")
            .expect("the dependency stage carries the path crate");
        let build = deps
            .iter()
            .position(|l| l.contains("cargo build --locked"))
            .expect("the dependency stage compiles");
        assert!(
            copy < build,
            "the crate is there before cargo resolves the graph"
        );
        let runtime: Vec<&str> = file
            .lines()
            .skip_while(|l| *l != "FROM alpine:3.21")
            .collect();
        assert!(
            !runtime.iter().any(|l| l.contains("majordomus-core")),
            "{file}"
        );
        assert!(dockerignore(&d, "x.yaml")
            .lines()
            .any(|l| l == "!apps/majordomus-core"));
        // without a path crate the dependency stage copies the graph alone, as before
        assert!(!dockerfile(&deployment(), "x.yaml", "1.98.1").contains("majordomus-core"));
    }

    #[test]
    fn the_runtime_carries_the_binary_and_the_inputs_and_no_toolchain() {
        let file = dockerfile(&deployment(), "x.yaml", "1.98.1");
        let runtime: Vec<&str> = file
            .lines()
            .skip_while(|l| *l != "FROM alpine:3.21")
            .collect();
        assert!(runtime.contains(&"COPY --from=binary /out/majordomus /usr/local/bin/majordomus"));
        assert!(runtime.contains(&"COPY share /app/share"));
        assert!(runtime.contains(&"USER majordomus"));
        assert!(!runtime
            .iter()
            .any(|l| l.contains("cargo") || l.contains("apk add")));
    }

    #[test]
    fn rendering_is_deterministic() {
        let d = deployment();
        assert_eq!(
            dockerfile(&d, "x.yaml", "1.98.1"),
            dockerfile(&d, "x.yaml", "1.98.1")
        );
    }

    #[test]
    fn only_an_exact_version_is_a_builder_pin() {
        assert!(is_exact_version("1.98.1"));
        for floating in [
            "stable",
            "1.98",
            "nightly-2026-01-01",
            "1.98.1-beta",
            "",
            "1..1",
        ] {
            assert!(!is_exact_version(floating), "{floating} accepted");
        }
    }

    #[test]
    fn the_pin_is_read_from_the_toolchain_file_and_its_absence_is_refused() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let err = builder_rust(dir.path()).expect_err("no file, no pin");
        assert!(err.contains(TOOLCHAIN_FILE), "{err}");
        std::fs::write(
            dir.path().join(TOOLCHAIN_FILE),
            "[toolchain]\nchannel = \"stable\"\n",
        )
        .expect("write");
        let err = builder_rust(dir.path()).expect_err("a floating channel is no pin");
        assert!(err.contains("`stable`"), "{err}");
        std::fs::write(
            dir.path().join(TOOLCHAIN_FILE),
            "[toolchain]\nchannel = \"1.98.1\"\n",
        )
        .expect("write");
        assert_eq!(builder_rust(dir.path()).as_deref(), Ok("1.98.1"));
    }

    #[test]
    fn the_executable_is_compiled_by_default_and_a_release_brings_its_own() {
        let file = dockerfile(&deployment(), "x.yaml", "1.98.1");
        // declared before the first FROM, so a FROM may name it; the default compiles
        let first_from = file.find("\nFROM ").expect("a FROM");
        assert!(file[..first_from].contains(&format!("\nARG {BINARY_ARG}=build\n")));
        assert!(file.contains(&format!("\nFROM ${{{BINARY_ARG}}} AS binary\n")));
        let release = stage(&file, "release");
        assert_eq!(release[0], "FROM scratch AS release");
        assert!(release.contains(&"ARG TARGETARCH"));
        assert!(release
            .contains(&"COPY --from=release-binary majordomus-${TARGETARCH} /out/majordomus"));
        // nothing in the release stage compiles or reads the commit
        assert!(!release
            .iter()
            .any(|l| l.contains("cargo") || l.contains(COMMIT_ARG)));
        // and the runtime takes its executable from `binary` and from nowhere else
        assert_eq!(file.matches("COPY --from=build ").count(), 0);
        assert_eq!(file.matches("COPY --from=binary ").count(), 1);
    }

    #[test]
    fn the_unprivileged_process_owns_the_one_directory_it_writes() {
        let file = dockerfile(
            &deployment_with(&["apps/majordomus-cli", ".ai"]),
            "x.yaml",
            "1.98.1",
        );
        let runtime: Vec<&str> = file
            .lines()
            .skip_while(|l| *l != "FROM alpine:3.21")
            .collect();
        let owned = "RUN install -d -o majordomus -g majordomus /app/.ai/local";
        let at = |needle: &str| runtime.iter().position(|l| *l == needle);
        let (copy, own, user) = (at("COPY .ai /app/.ai"), at(owned), at("USER majordomus"));
        assert!(copy < own && own < user, "{copy:?} {own:?} {user:?}");
        // a deployment that serves no layer has no layer to write into
        let none = dockerfile(&deployment(), "x.yaml", "1.98.1");
        assert!(!none.contains(".ai/local"));
    }

    fn gigalixir() -> Deployment {
        let mut d = deployment();
        d.provider = Some(super::super::Provider {
            name: ProviderName::Gigalixir,
            app: "prismatic-majordomus".into(),
            stack: "gigalixir-24".into(),
            url: "https://prismatic-majordomus.gigalixirapp.com".into(),
        });
        d
    }

    #[test]
    fn a_deployment_without_a_provider_renders_no_provider_file() {
        let paths: Vec<String> = artifacts(&deployment(), "x.yaml", "1.98.1")
            .into_iter()
            .map(|a| a.path)
            .collect();
        assert_eq!(paths, [DOCKERFILE, DOCKERIGNORE]);
    }

    #[test]
    fn a_gigalixir_deployment_renders_its_four_files_under_its_directory() {
        let arts = artifacts(&gigalixir(), "x.yaml", "1.98.1");
        let paths: Vec<&str> = arts.iter().map(|a| a.path.as_str()).collect();
        assert_eq!(
            paths,
            [
                DOCKERFILE,
                DOCKERIGNORE,
                GIGALIXIR_PROCFILE,
                GIGALIXIR_DETECT,
                GIGALIXIR_COMPILE,
                GIGALIXIR_RELEASE
            ]
        );
        for a in &arts[2..] {
            assert!(a.path.starts_with(GIGALIXIR_DIR), "{}", a.path);
            assert_eq!(a.format, ArtifactFormat::Text);
            assert!(a
                .content
                .contains("GENERATED by `majordomus generate deployment` from x.yaml."));
            let banner = format!("# {}", crate::generate::HEADER);
            assert!(
                crate::generate::opens_with_banner(&a.content, &banner),
                "{} does not open with the banner",
                a.path
            );
        }
    }

    #[test]
    fn the_procfile_starts_the_executable_on_the_providers_port_falling_back_to_the_declared_one() {
        let procfile = gigalixir_procfile(&gigalixir(), "x.yaml");
        let web: Vec<&str> = procfile.lines().filter(|l| l.starts_with("web:")).collect();
        assert_eq!(
            web,
            ["web: ./.majordomus/bin/majordomus serve --repo . --discovery filesystem --host 0.0.0.0 --port ${PORT:-8080} --idle 0 --advertise https://prismatic-majordomus.gigalixirapp.com --node-seed-env MAJORDOMUS_MESH_NODE_SEED"]
        );
        // every other line is a comment the provider ignores
        assert!(procfile
            .lines()
            .all(|l| l.starts_with("web:") || l.starts_with('#') || l.is_empty()));
        let mut moved = gigalixir();
        moved.listen.port = super::super::Port::new(9090).expect("a port");
        assert!(gigalixir_procfile(&moved, "x.yaml").contains("--port ${PORT:-9090} "));
    }

    #[test]
    fn every_hook_opens_with_its_interpreter_and_carries_the_header_next() {
        let d = gigalixir();
        for (name, hook) in [
            ("detect", gigalixir_detect("x.yaml")),
            ("compile", gigalixir_compile(&d, "x.yaml")),
            ("release", gigalixir_release("x.yaml")),
        ] {
            let mut lines = hook.lines();
            assert_eq!(lines.next(), Some("#!/bin/sh"), "{name}");
            assert_eq!(
                lines.next(),
                Some(format!("# {}", crate::generate::HEADER).as_str()),
                "{name}"
            );
        }
        assert!(gigalixir_detect("x.yaml").ends_with("\necho majordomus\n"));
        assert!(gigalixir_compile(&d, "x.yaml").ends_with(
            "\nchmod 755 \"$1/.majordomus/bin/majordomus\"\n\"$1/.majordomus/bin/majordomus\" --version\n"
        ));
        assert!(gigalixir_release("x.yaml").ends_with("\necho \"--- {}\"\n"));
    }

    #[test]
    fn the_inline_buildpack_is_pinned_by_a_full_commit() {
        let (repo, commit) = buildpack(ProviderName::Gigalixir)
            .split_once('#')
            .expect("a pinned buildpack");
        assert_eq!(repo, "https://github.com/kr/heroku-buildpack-inline");
        assert_eq!(commit.len(), 40);
        assert!(commit.bytes().all(|b| b.is_ascii_hexdigit()));
    }

    fn deployment_with(inputs: &[&str]) -> Deployment {
        let mut d = deployment();
        d.build.inputs = inputs.iter().map(|s| s.to_string()).collect();
        d
    }
}
