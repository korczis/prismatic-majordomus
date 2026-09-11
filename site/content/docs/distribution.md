+++
title = "Distribution"
description = "how the tool is packaged, published and installed: the one canonical model, what derives from it, the trust path, adding a platform, releasing, and recovering from a bad release"
weight = 23
[extra]
source = "docs/DISTRIBUTION.md"
+++

{% raw %}

How Majordomus is packaged, published and installed, and why the answer is one file.

The user-facing half is [`INSTALL.md`](@/docs/install.md), which is generated. This document is
for whoever changes how the tool is shipped: what the canonical model is, what derives from
it, what the trust path is, how a release happens, and how to add a platform.

## The rule

> Distribution has one canonical machine-readable model. Supported targets, release
> artifacts, installer resolution, documentation, release automation and the exposed
> UI/API metadata derive from that model or are mechanically validated against it.

The model is [`share/distribution.yaml`](../share/distribution.yaml), contract
`majordomus-distribution/v1`, schema
[`share/schemas/majordomus/distribution/distribution.v1.schema.json`](../share/schemas/majordomus/distribution/distribution.v1.schema.json). It
declares the binary, the repository releases are published from, the installer's canonical
URL and defaults, how an archive is named, and every target the project has an opinion
about — supported, experimental, or unavailable with the reason.

It is in `share/` rather than under `.ai/` for the reason `share/commands.yaml` is: it is a
statement about how the tool behaves, not context for a worker, and it travels with an
installed copy so that an installed executable can answer the same questions a checkout can.

## What derives from it

```text
share/distribution.yaml
  |
  |-- majordomus generate --target distribution
  |     |-- docs/generated/distribution-matrix.json   the release build matrix
  |     |-- site/static/install.sh                    the installer, with its platform table
  |     |-- docs/INSTALL.md                           the guide, with its platform table
  |     |-- site/data/registry/distribution.json      what the website renders
  |     `-- site/static/releases/*.json               from .ai/repo/releases/*.yaml
  |
  |-- the `distribution` capability module            CLI, HTTP, OpenAPI, MCP, cockpit
  `-- scripts/release-*                               packaging, verification, recording
```

Nothing in that list states a platform, a file name or a URL of its own. The scripts ask
`majordomus distribution artifact` for a name rather than composing one; the release
workflow reads the generated matrix rather than listing runners; the installer's platform
table is a generated region inside a hand-written script; the documentation's platform
table is generated into a hand-written guide.

`majordomus generate --check` — reached by `just derive-check`, by the `site` job and by
the release pipeline's first phase — refuses a tree in which any of them disagrees.

## Adding a platform

One edit, then one command:

```bash
$EDITOR share/distribution.yaml     # add a target: id, os, arch, libc, rust_target, status, build
just derive                         # regenerate every projection
bash test/run.sh 84_distribution_model
```

The installer offers it, the release builds it on the runner the model named, the guide
lists it, the website lists it, the matrix carries it, and the completeness invariant now
requires an artifact for it in every future release. Nothing else is edited.

A genuinely new *concept* — another operating system, another archive format, another libc
— is the one case that also changes Rust: the vocabularies are enums in
`apps/majordomus-cli/src/distribution/mod.rs` so that an impossible target cannot be
written down. Adding a value there is a deliberate widening of the model, not a
synchronisation chore.

## The two versions, and why there are two

One release has one version, stated in two places because two programs ship from one tree
and each must say what it is without reading the other:

```text
apps/majordomus-cli/Cargo.toml   the crate's version — the authority
bin/majordomus                   MJ_VERSION, which `majordomus version` prints
```

Neither can derive the other at run time: an installed tree has no `Cargo.toml`, and the
crate is compiled before the shell tool exists. So they are checked instead, by
`scripts/release-version --check`, in CI and again as the first thing a release does —
together with the tag, which must be `v` followed by that version. A mistyped tag costs
thirty seconds rather than a published release.

## The trust path

```text
share/distribution.yaml            the model: platforms, names, the installer's URL
        |
        v
release build (one runner per target, the model names which)
        |
        v
release-verify                     the archive is what the model says it is
        |
        v
release-record                     digests and sizes read off the files that exist
        |
        v
.ai/repo/releases/<tag>.yaml       the repository's evidence of what was published
        |
        v
majordomus generate                site/static/releases/<tag>.json and latest.json
        |
        v
GitHub Pages                       the metadata the installer reads, over HTTPS
        |
        v
install.sh                         verifies the digest before unpacking anything
        |
        v
~/.local/share/majordomus/versions/<version>/, then the launcher, by rename
```

Each arrow is checked by the step after it. The installer never reads GitHub's API or HTML;
it reads two JSON documents that this repository generates, so a change in GitHub's pages
cannot break an installation.

## What is protected, and what is trusted

Protected:

- **transport** — HTTPS only, redirects restricted to HTTPS; a non-HTTPS metadata base is
  refused unless two explicit environment variables say otherwise, which exists so the
  installer's own tests can serve fixtures;
- **integrity** — every artifact carries a SHA-256 and a size in the metadata, both checked
  before extraction; a mismatch discards the download and stops, with no "warn and
  continue";
- **provenance of the URL** — the artifact URL must be this project's own release download
  prefix, derived from the model, and is checked before anything is fetched from it;
- **the shape of every value** — every field read out of the metadata is matched against a
  literal character class before use, so nothing read over the network can reach a command
  line, a path or the shell as anything but data;
- **extraction** — absolute paths, `..` traversal, symbolic and hard links, devices and any
  entry outside the archive's own directory are refused by name, and nothing is unpacked;
- **the executable itself** — the unpacked tool is run once, from a temporary location, and
  must report the version that was resolved; this is what catches a release built from the
  wrong tree;
- **the existing installation** — the new tree is unpacked beside the old one and the
  launcher is replaced by a rename, so any failure before that leaves the previous
  installation exactly as it was;
- **privilege** — `sudo` appears nowhere; an unwritable destination is named, not elevated.

Trusted: GitHub Pages serving the metadata, GitHub Releases serving the artifacts, and the
pipeline that produced both. Checksums bind an artifact to its metadata; they do not, by
themselves, prove who wrote the metadata.

Deferred, deliberately: signed provenance (GitHub artifact attestations, Sigstore). It is
worth adding and it is not claimed here, because a security document that describes
protections it does not have is worse than one that is short.

## Releasing

```bash
# 1. bump the version, in both places, and commit
$EDITOR apps/majordomus-cli/Cargo.toml bin/majordomus
scripts/release-version --check --tag v0.3.0

# 2. tag, and push the tag
git tag v0.3.0 && git push origin v0.3.0
```

The pipeline is [`.github/workflows/release.yml`](../.github/workflows/release.yml), an
adapter over [`.ai/repo/ci/release.yaml`](../.ai/repo/ci/release.yaml) and the generated
matrix:

```text
plan     the tag, the crate and the shell tool state one version; the model and every
         recorded release hold their invariants; the matrix is emitted
build    one archive per supported target, on the runner the model names, verified where
         it was built (fail-fast: a release missing a platform is not a release)
publish  digests, the GitHub release, the record written from what was uploaded, the
         public metadata regenerated from that record, both committed to the default branch
smoke    the published installer, from its published URL, installing the release that was
         just published, on every runner whose target it was built for
```

Only `publish` has `contents: write`. Nothing else in the run can write anything.

A runner label the model names must be a standard, currently offered GitHub-hosted label.
This is not a style rule. A retired label does not fail: the job is accepted and queues for
a runner that will never arrive, so the run neither publishes nor goes red — it simply never
ends, and `fail-fast` cancels it when a sibling fails, which makes it look like collateral
damage rather than the cause. `macos-13` sat that way through two release attempts. The list
is GitHub's, at `actions/runner-images`; a `-large` or `-xlarge` suffix means a billed larger
runner rather than a standard one.

A run that fails in `build` publishes nothing, which is the intended behaviour and also the
one that is easy to walk away from: the tag still exists, pointing at the commit the build
failed on, and no release is behind it. Fixing the cause on the default branch does not fix
the tag. Finish the release — a new version, bumped and tagged, is the ordinary way; moving
a tag that has no release behind it is the other, and only before anyone can have pinned it.
Until one of those happens the advertised install command is broken for everyone, and
`installer-live` below is what says so.

### When a release fails partway

A release changes things the world can see, and it can fail after some of them have changed.
Rerunning the workflow for the same tag is the supported recovery, and it is safe at every
stage. What "safe" means is declared in [`.ai/repo/ci/release.yaml`](../.ai/repo/ci/release.yaml)
under `rerun:` and implemented in the publish job:

* **the assets on an existing release are the release.** If the tag already has a GitHub
  release, the rerun downloads its published assets and discards its own rebuild. The record
  is evidence of what a user downloads, and rebuilds are not bit-identical (see
  *Reproducibility* below), so recording this run's bytes would state digests nothing serves.
* **the metadata commit is idempotent.** A rerun that finds the record and its projections
  already committed pushes nothing and succeeds.
* **smoke is never skipped.** Both paths end in the same public installation test, so a
  rerun proves the same external contract a first run does.

Stage by stage:

<div class="overflow-x-auto" tabindex="0">

| Failed at | Visible outside? | Rerun | Cleanup |
|---|---|---|---|
| `plan` | no | yes | none — no artifact was built |
| `build` | no | yes | none — nothing was uploaded |
| `publish`, before the release exists | no | yes | none |
| `publish`, after the release exists | yes — the release and its assets | yes; the rerun adopts those assets | none |
| `publish`, after the metadata commit | yes — the record is on the default branch | yes; the commit step finds nothing to add | none |
| `pages` | yes — the record is committed but not served | `gh workflow run pages.yml --ref master` | none |
| `smoke` | yes — everything is published | fix the cause, then rerun | none; the release stands or is withdrawn below |

</div>


#### Why the release asks Pages to publish

The publish job ends by running `gh workflow run pages.yml`, and that step is not a
belt-and-braces addition: without it the release is structurally unable to reach a user.

GitHub does not start a workflow from a push made with `GITHUB_TOKEN`. It is a deliberate
loop-breaker and it cannot be disabled for a token. The commit the publish job makes — the
only thing that carries `site/static/releases/latest.json` — therefore starts no `pages`
run, so the site keeps serving a build from before the release existed, and the smoke phase
waits fifteen minutes for a file nothing is going to write.

Release `v0.3.1` spent its entire smoke phase in exactly that state: six green builds, a
published GitHub Release, a committed record, and `latest.json` returning 404 to anyone who
ran the advertised command. Every observable pointed at the packer, and the packer was fine;
the last link of the chain simply did not exist. The dispatch is that link. It runs with
`if: always()`, because a rerun after a failed deploy is precisely the case with nothing to
commit and everything to publish.

`workflow_dispatch` is the entry point a person uses, so this starts the one deploy path
rather than adding a second one (see the `site-deploy-one-path` guarantee).

The one case a rerun cannot repair is a release that exists and publishes no archive — the
assets cannot be reconstructed from a tag. The run stops and names the command that clears
the way:

```bash
gh release delete v0.3.0 --yes    # then rerun the workflow
```

Nothing here needs the tag to be moved. A tag that points at the wrong commit is a different
problem, and the answer to it is a new version, not a moved tag.

### Testing it without publishing

```bash
scripts/release-fixture --out /tmp/rel --base-url http://127.0.0.1:8099
(cd /tmp/rel && python3 -m http.server 8099 --bind 127.0.0.1) &
MAJORDOMUS_RELEASE_BASE_URL=http://127.0.0.1:8099 MAJORDOMUS_INSECURE_BASE_URL=1 \
  sh site/static/install.sh --dry-run
```

That is what `test/cases/85_installer.sh` does, and it is the whole stack: a real archive
built from the tree you are in, real metadata rendered by the renderer the site publishes,
a real HTTP download, a real digest check, a real atomic install.

### Proving the advertised command still works

A release pipeline proves the advertised command works *once*, in its smoke phase, at the
moment of publication. That is not the same promise as the one the front page makes, which
is in the present tense, and the difference has been real: a build that failed on one
platform publishes nothing, the tag stays where it is, the fix lands on the default branch
and is never tagged again — and the URL every document points at keeps serving an installer
that resolves no release. Every gate over the tree stayed green throughout, because none of
them can see the published site.

`scripts/ci/install-check` is the gate that can. It reads the addresses from
`site/data/registry/distribution.json` — states none of its own — and then, from the
published site:

```text
1  the metadata the installer resolves is served, and names a release
2  the installer is served, and this machine's /bin/sh parses it
3  the advertised line, run as it is written, pipe included, into a home of its own
4  the installed tool reports the version the metadata resolved
5  the installed MCP launcher runs with MAJORDOMUS_NO_BUILD=1 — an archive that left a
   launcher out passes every check that only reads the archive's file list, and fails here
6  the installed tool initialises a repository that has none
```

Nothing outside its temporary tree is written: the install goes to a `HOME` of the run's
own, so the prefix, the launchers and the PATH hint all land inside it.

CI runs it as `--wait 300`. Publishing a release and deploying the site are two workflows and
the second is not instant, so a push landing between them would be told the promise is broken
when it is merely a few minutes old. Run by hand the wait is zero, because a person asking
whether the command works wants the answer now. A site that cannot serve the metadata inside
the window is broken either way, and the finding stands.

It is the gate `installer-live` in [`.ai/repo/ci/gates.yaml`](../.ai/repo/ci/gates.yaml),
job `install`, on Linux and macOS. No path class selects it, deliberately: no change to a
tree can make it true or false — only a deployment can. It is also one of the model's
on-demand gates, because half of its matrix is a macOS runner and this repository waits
hours for one: the nightly schedule, a dispatch and a pull request labelled `ci:full` plan
it, a routine push does not. So a broken advertised command turns the nightly run red,
within a day of the deployment that broke it, which is the only condition under which anyone
was going to find out. To ask the question now — after a release, say — dispatch the
workflow, or run `scripts/ci/install-check` here.

Against a local fixture rather than the published site:

```bash
scripts/release-fixture --out /tmp/rel --base-url http://127.0.0.1:8099
(cd /tmp/rel && python3 -m http.server 8099 --bind 127.0.0.1) &
scripts/ci/install-check --base http://127.0.0.1:8099
```

### Recovering from a bad release

A published release is immutable: its artifacts and its versioned metadata stay exactly as
they were, because someone may have pinned them. Withdrawing one is an edit to its record:

```yaml
# .ai/repo/releases/v0.3.0.yaml
yanked: true
```

then `just derive` and commit. `latest.json` is derived on every generation as the highest
version among the stable, unwithdrawn records, so the stable pointer moves back on its own
and `--version v0.3.0` still resolves for anyone who wants exactly that. Nothing is deleted
and no URL stops working.

## Reproducibility

The inputs to an artifact are recorded rather than claimed: the tag, the target triple, the
commit and the build time go into `RELEASE.json` inside every archive, and the commit is
compiled into the executable by `build.rs`. The archive itself is packed with entries in
sorted order.

Every archive carries each path exactly once, and carries nothing but regular files and
directories: no hard link, no symlink, no device node. `scripts/release-package` checks that
of the archive it has just written and deletes it rather than return a violating one, and
`scripts/release-verify` checks it again of the archive it is handed. Both exist because
release `v0.2.0` was never published: the packer passed a complete file list to a `tar` that
also recursed into it, every path was archived twice, GNU tar wrote the second copy of each
as a hard link, and the verifier — correctly — refused all 931 of them. There is deliberately
no fallback in the packer: an archive packed differently from the one that was asked for is
not the archive anything verified. `test/cases/87b_release_archive_shape.sh` reproduces that
packer with a `tar` shim and proves the guard stops it.

Bit-identical rebuilds are not claimed: that needs a reproducible compiler invocation
(`SOURCE_DATE_EPOCH`, a pinned toolchain version, no absolute paths in debug info), and the
toolchain here is `stable` rather than a pin. What is guaranteed is that two people
building the same tag get functionally identical archives, and that the archive says which
commit and target it came from.

## Self-update

There is no `majordomus self update`, and when there is one it will not be a second update
protocol. Everything it needs already exists and is typed:

```rust
Model::load(share)          // this installation's own model, shipped in share/
Model::release_url(tag)     // where the metadata for a tag or `latest` lives
Releases::latest_stable()   // which release an unpinned update resolves to
Release::artifact(target)   // the artifact, its digest and its size
crate::TARGET               // the triple this build was made for, compiled in
```

An installed copy carries `share/distribution.yaml`, so `majordomus distribution show` and
`majordomus distribution build` answer from an installation exactly as they answer from a
checkout — which is the property a self-update would be built on. The remaining work is the
download, the verification and the swap, and the installer already does all three; the
open design question is whether to reimplement them in Rust or to have the command re-run
the published installer with `--version`, which would keep one implementation of the risky
part rather than two.

`test/cases/85_installer.sh` covers that risky part today, and whichever way the command is
built it will resolve releases through the same records and the same stable pointer as the
installer does.

## Package managers

The release model is the primary contract and it is package-manager neutral: an archive per
target with a digest, and a machine-readable index. A Homebrew formula, an APT repository,
a Nix derivation or a container image would each read that index rather than replacing it,
and none of them is implemented. Nothing in the model prevents one.

Windows is representable — the schema carries `zip`, a `binary_suffix` and a `windows`
operating system — and is declared `unavailable` with its reason: the command a person runs
is `bash`, and until that half has a Windows answer, publishing a Windows archive would
ship an executable nothing can drive.

## Where things are

<div class="overflow-x-auto" tabindex="0">

| What | Where |
|---|---|
| the model | `share/distribution.yaml` |
| its contract | `share/schemas/majordomus/distribution/distribution.v1.schema.json` |
| a release record's contract | `share/schemas/majordomus/release/release.v1.schema.json` |
| the records | `.ai/repo/releases/`, one per published release |
| the semantics | `apps/majordomus-cli/src/distribution/` |
| the capability module | `apps/majordomus-cli/src/capability/builtin/distribution.rs` |
| the installer's behaviour | `share/install/install.sh.in` |
| the guide's prose | `share/install/INSTALL.md.in` |
| packaging and verification | `scripts/release-{version,package,verify,record,fixture}` |
| the pipeline's model | `.ai/repo/ci/release.yaml` |
| the pipeline | `.github/workflows/release.yml` |
| the behavioural cases | `test/cases/84_distribution_model.sh`, `85_installer.sh`, `86_installer_platform.sh`, `87_release_pipeline.sh` |
| the rule | `.ai/repo/rules/project/distribution-canonical.v1.md` |

</div>

{% endraw %}
