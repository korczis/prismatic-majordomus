<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: share/install/INSTALL.md.in (the prose) and share/distribution.yaml (every platform, name and URL);
     regenerate with `majordomus generate`
     Generator: majordomus-cli 0.2.0 -->
# Installing Majordomus

## Quick install

```bash
curl -fsSL https://korczis.github.io/prismatic-majordomus/install.sh | sh
```

Then, in the repository you want supervised:

```bash
majordomus init
```

## Install and initialise in one command

```bash
curl -fsSL https://korczis.github.io/prismatic-majordomus/install.sh | sh -s -- --init
```

`--init` runs `majordomus init` in the current directory using the executable that was just
installed, by its absolute path, so it does not depend on your `PATH` having been reloaded.
If installation succeeds and initialisation fails, the installer says so in those words and
leaves the working installation in place.

## Verify

```bash
majordomus --version
majordomus doctor
```

`doctor` reports whether the layer resolves, whether every enforcement it claims is actually
wired, and which hook lines are still missing.

## What it does to your machine

Nothing outside two places, and never as root:

```text
$HOME/.local/share/majordomus/versions/<version>/    the release, unpacked
$HOME/.local/bin/majordomus                    a launcher
$HOME/.local/bin/majordomus-mcp                a launcher for the MCP server
```

The launchers are two-line shell scripts that exec the versioned tree. No shell profile is
edited, no package manager is invoked, and `sudo` is never run: if a directory is not
writable the installer says which one and stops.

## Supported platforms

| Platform | Architecture | libc | Rust target | Status |
|---|---|---|---|---|
| macOS | ARM64 | — | `aarch64-apple-darwin` | supported |
| macOS | x86_64 | — | `x86_64-apple-darwin` | supported |
| Linux | x86_64 | glibc | `x86_64-unknown-linux-gnu` | supported |
| Linux | x86_64 | musl | `x86_64-unknown-linux-musl` | supported |
| Linux | ARM64 | glibc | `aarch64-unknown-linux-gnu` | supported |
| Linux | ARM64 | musl | `aarch64-unknown-linux-musl` | supported |
| Windows | x86_64 | — | `x86_64-pc-windows-msvc` | unavailable |


That table is generated from `share/distribution.yaml`, which is the one place a platform is
declared. The installer resolves your machine to a row of it from `uname -s`, `uname -m` and,
on Linux, the C library it finds; the release build matrix is generated from the same rows,
so a platform listed here is a platform a release actually built.

**Windows x86_64** (`x86_64-pc-windows-msvc`) — The command a person runs is bash: bin/majordomus and lib/*.sh are POSIX shell, and the archive carries them beside the executable. Until that half has a Windows answer, publishing a Windows archive would ship an executable nothing can drive.



## Choosing where it goes

```bash
curl -fsSL https://korczis.github.io/prismatic-majordomus/install.sh | sh -s -- --install-dir "$HOME/bin"
curl -fsSL https://korczis.github.io/prismatic-majordomus/install.sh | sh -s -- --prefix "$HOME/opt/majordomus"
```

or, equivalently:

```bash
MAJORDOMUS_INSTALL_DIR="$HOME/bin" curl -fsSL https://korczis.github.io/prismatic-majordomus/install.sh | sh
```

## Pinning a version

```bash
curl -fsSL https://korczis.github.io/prismatic-majordomus/install.sh | sh -s -- --version v0.2.0
```

A pinned installation resolves `https://korczis.github.io/prismatic-majordomus/releases/v0.2.0.json`, which names the exact artifact and its
sha256 digest, and therefore installs the same bytes every time. An unpinned
installation resolves `https://korczis.github.io/prismatic-majordomus/releases/latest.json`, which is the latest stable release and moves
forward as releases are published. Use the pinned form in CI.

## Using it in CI

```bash
curl -fsSL https://korczis.github.io/prismatic-majordomus/install.sh | sh -s -- --version v0.2.0
export PATH="$HOME/.local/bin:$PATH"
majordomus --version
```

The installer needs no interaction and asks none: everything it does is decided by its
arguments and environment. Pin the version so that a new release cannot change a build that
used to pass.

## Upgrading and reinstalling

Run the same command again. The installer is idempotent:

- the same version already installed — it says so and exits 0;
- an older version installed — it upgrades, and the previous installation keeps working
  until the moment the launcher is replaced;
- a newer version installed and no version pinned — it refuses to move you backwards, and
  says how to do it anyway (`--force`, or `--version`).

## Uninstalling

```bash
rm -f $HOME/.local/bin/majordomus $HOME/.local/bin/majordomus-mcp
rm -rf $HOME/.local/share/majordomus
```

That is the whole of it. It removes the tool. It does **not** remove any repository's
`.ai/` directory: your policy, rules, decisions, plan and history are the repository's, not
the tool's, and no command of this project deletes them.

## Releases

No release is published yet. When one is, its metadata appears at `https://korczis.github.io/prismatic-majordomus/releases/latest.json` and this table lists it.


Each release publishes one archive per supported target plus a `SHA256SUMS` file, and
its metadata is served as JSON beside the installer. The installer reads that JSON and never
GitHub's API or HTML, so a change in GitHub's pages cannot break an installation.

## Security model

What is protected, and how:

- **The metadata is fetched over HTTPS**, with redirects restricted to HTTPS. A base URL
  that is not HTTPS is refused unless two explicit environment variables say otherwise, which
  exists so the installer's own tests can serve fixtures.
- **The artifact is verified before it is unpacked.** The metadata carries a sha256
  digest and a size; the installer checks both and, on a mismatch, discards the download and
  stops. There is no "warn and continue".
- **The artifact URL is checked against the project's own release host** before anything is
  fetched from it, and every value read out of the metadata is matched against the shape it
  must have. Nothing read over the network reaches a command line, a path or the shell.
- **The archive is inspected before extraction.** Absolute paths, `..` traversal, symbolic
  and hard links, devices, and any entry outside the archive's own directory are refused by
  name and nothing is unpacked.
- **The installation is atomic.** The new tree is unpacked beside the old one and its
  executable is run once, from a temporary location, to prove it reports the version that was
  resolved. Only then is the launcher replaced, by a rename. Any failure before that leaves
  the previous installation exactly as it was.
- **Nothing is elevated.** `sudo` appears nowhere in the installer.

What remains trusted: GitHub Pages serving the metadata, GitHub Releases serving the
artifacts, and the release pipeline that produced both. Checksums bind the artifact to the
metadata; they do not, on their own, prove who wrote the metadata. Signed provenance is a
recorded next step, not a claim made here — see `docs/DISTRIBUTION.md`.

## Reading it before running it

`curl … | sh` runs a script you have not read. If you would rather read it first:

```bash
curl -fsSL https://korczis.github.io/prismatic-majordomus/install.sh -o install-majordomus.sh
less install-majordomus.sh
sh install-majordomus.sh
```

The installer is one file with no dependencies beyond `curl` or `wget`, `tar`, and one of
`sha256sum`, `shasum` or `openssl`. Its top half is generated from the distribution model and
its bottom half is behaviour; both are reviewed in this repository and linted by
`shellcheck` in CI.

## Troubleshooting

**`majordomus: command not found` after installing.** The launcher directory is not on your
`PATH`. The installer prints the exact line to add; it does not edit your shell profile.

**"does not provide a prebuilt binary for".** Your machine resolved to no supported target.
The message names what was detected and lists what is supported. There is no automatic
fallback to building from source: that would need the toolchain this installer promises not
to require.

**"the downloaded archive does not match its digest".** Stop. Nothing was installed. Run it
again; if it happens twice, the artifact or the metadata is wrong and forcing it would be
the wrong response.

**Behind a proxy.** `curl` and `wget` read the usual `https_proxy` environment; the
installer adds nothing of its own.

**Seeing what it would do.**

```bash
curl -fsSL https://korczis.github.io/prismatic-majordomus/install.sh | sh -s -- --dry-run
curl -fsSL https://korczis.github.io/prismatic-majordomus/install.sh | sh -s -- --verbose
```

`--dry-run` resolves the platform, the release and every path, prints them, and changes
nothing.

## Building from source

For contributors, not for installation. The repository's own tooling is documented in
`CONTRIBUTING.md`:

```bash
git clone https://github.com/korczis/prismatic-majordomus
cd majordomus
cargo build --release --manifest-path apps/majordomus-cli/Cargo.toml
```

The shell tool needs no build at all: `bin/majordomus` runs from a checkout. Use the
installer for everything else — a source checkout is a development environment, not an
installation.

## How this works underneath

The architecture, the trust path, the release procedure and how to add a platform are in
[`DISTRIBUTION.md`](DISTRIBUTION.md).
