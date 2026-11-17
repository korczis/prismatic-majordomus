---
schema: adr/v1
id: adr-0175
kind: adr
title: v1.0.0-alpha.1 declares what it can do and its limits, and every line of the declaration names its proof
status: proposed
date: 2026-10-19
tags:
  - release
  - alpha
  - claims
  - distribution
related:
  - file:.ai/repo/adrs/0029-the-changelog-is-a-projection-and-the-version-has-one-writer.md
  - file:docs/CLAIMS.yaml
  - file:docs/DISTRIBUTION.md
  - file:docs/INSTALL.md
  - file:docs/RELEASE.md
  - file:share/distribution.yaml
  - file:.ai/repo/project/issues/I2826.yaml
provenance:
  origin: authored
---

# 175. v1.0.0-alpha.1 declares what it can do and its limits, and every line of the declaration names its proof

## Context

The owner's order for the first pre-release reads: the release notes for the alpha declare
exactly what it can do and its limits. Nothing in the repository authors release notes.
The GitHub release body is GitHub's own list of merged pull requests (`gh release create
--generate-notes`, `.github/workflows/release.yml`); the release record
(`.ai/repo/releases/<tag>.yaml`) is evidence read off the artifacts, with a `notes_url` back
to that GitHub page; and `release.changelog` composes the records, the ADRs dated inside
each release's window and the conventional commits of its range (ADR 0029). The only
authored text that reaches what the tool prints for a tag is therefore a decision dated
inside the tag's window. That the published body is not the tool's changelog contradicts
ADR 0029's "every surface is derived"; I2826 owns that, after the alpha.

## Decision

**The alpha's declaration is this decision.** It is dated inside the window that ends at
`v1.0.0-alpha.1`, so `majordomus release changelog` prints it in the alpha's section. Every
line names the feature, claim, case or document that proves it, and nothing is stated that
the claims matrix (`docs/CLAIMS.yaml`) or a case does not hold. A claim that is not
`guaranteed` is a limit, not a capability.

### What v1.0.0-alpha.1 can do

- **Install and verify itself** on macOS (aarch64, x86_64) and Linux (x86_64 and aarch64,
  glibc and musl), pinned by name, because a pre-release is never the latest release:
  `curl -fsSL https://majordomus.dev/install.sh | sh -s -- --version v1.0.0-alpha.1`
  (`docs/INSTALL.md`).
  The supported targets are `share/distribution.yaml`'s; the installer verifies every
  archive before installing it (`installer-verifies-before-installing`, case
  `85_installer.sh`), a release is complete on every required target or absent
  (`release-is-complete-or-absent`), and the advertised command is derived, not typed
  (`install-command-is-derived`). Feature `install`.
- **Supervise a repository's AI layer** under `.ai/`: scoped tasks whose scope is enforced
  at check and finish (`scoped-task`, `scope-enforcement`), a finish contract that
  refuses a task with unmet obligations and records the refusal (`finish-contract`,
  `finish-refusal-is-recorded`), handovers without transcripts (`handover-record`,
  `no-transcripts`), and context documents resolved by path (`context-documents`).
  Features `coordination`, `finish-contract`, `continuity`, `context`.
- **Declare once and project everywhere**: one capability registry projected to the command
  line, HTTP with OpenAPI, MCP and the Cockpit (`capability-registry`,
  `interfaces-are-projections`, `mcp-stdio-surface`, `cockpit-is-a-projection-of-the-registry`).
  The full surface is the generated reference, `docs/generated/capabilities.md`. Features
  `declare-once`, `interfaces`, `cockpit`.
- **Hold work to evidence**: a proof is an execution, never a statement
  (`evidence-proof-is-an-execution`), evidence currency is not collapsed into a pass
  (`evidence-currency-is-not-collapsed`), a commit message is judged (`commit-message-is-judged`),
  and new code is covered (`new-code-is-covered`). Features `evidence`, `commit`, `coverage`, `ci`.
- **Plan and remember**: milestones and issues with derived status (`project-schema`,
  `project-status-derived`), decisions and the ADR catalogue (`decision-record`,
  `adr-catalogue`), an append-only ledger (`ledger-integrity`). Features `planning`,
  `knowledge`, `provenance`.
- **Cooperate between sessions and machines**: authenticated mesh links and claims that
  cross runtimes (`mesh-cooperation-links`, `mesh-claims-cross-runtime`), off unless a
  repository enables it (`mesh-off-by-default`), and worktrees whose topology is derived
  (`worktree-topology-derived`). Features `mesh`, `worktrees`.
- **Reason without depending on a model**: reasoning works with no advisor
  (`reasoning-works-with-no-advisor`), advisors share one transport contract
  (`advisor-transports-share-one-contract`), and the model catalogue is one declaration
  routed with reasons (`models-one-declaration`, `models-routing-explains-itself`). Features
  `reasoning`, `models`.
- **Show what it read**: the UI dashboard cites its sources
  (`ui-dashboard-cites-its-sources`) and the Observatory draws nothing it did not read
  (`observatory-draws-nothing-it-did-not-read`). Features `ui`, `observatory`.

### Its limits

- **It is a pre-release.** It never replaces the stable release: a suffixed tag is published
  as a GitHub pre-release and is never the stable pointer an unpinned installer resolves
  (pull request #1313, case `3179_a_prerelease_is_never_published_as_stable`). Install it by
  pinning its version. `majordomus self update` is a design in `docs/DISTRIBUTION.md`, not an
  implementation, and there is no tested uninstall path.
- **No Windows build.** `windows-x86_64` is declared `unavailable` in
  `share/distribution.yaml`, with its reason: the command a person runs is POSIX shell.
- **Not every claim is a guarantee.** The claims the matrix marks `advisory` are reported,
  not enforced: `capability-class`, `checkpoint-interval`, `decision-attribution`,
  `effort-escalation`, `minimum-context`, `profile-axes`. The ones it marks `planned` are not
  in this release: `ci-verdict-is-required`, `cost-per-outcome`,
  `intent-closes-github-milestones`, `intent-command-deployment-evidence`, `routing`,
  `runtime-adapters`, `shared-policy`, `task-commit-attribution`, `telemetry`,
  `token-savings-measured`. Two are rejected outright: `semantic-retrieval`,
  `task-dependencies`. Whether a guaranteed claim's proof is current on a given tree is
  `majordomus evidence`'s answer, not this page's.
- **The mesh does not yet converge under every failure.** A claim released through one
  runtime was observed staying held on the runtime whose stream carries it, with links to a
  restarted peer refused as its previous instance (I2825, milestone
  `mesh-journal-converges`). An exclusive claim is local admission with announced
  conflicts, not a lock (`docs/MESH.md`).
- **Upgrading a macOS mesh hub can drop it from the tailnet.** The macOS application
  firewall admits a program by its path, and an upgrade installs the executable under a new
  versioned path, so the upgraded hub stops answering other machines until the new binary
  is admitted. This was observed when 0.28.0 was installed on a hub, and upgrading to the
  alpha does the same. Nothing runs the check after an upgrade, but the gap is visible:
  `majordomus mesh firewall` reads the new binary as `missing` and `majordomus mesh doctor`
  fails its firewall check. The step, on that Mac in the hub's checkout, is
  `sudo ~/.local/bin/majordomus mesh firewall apply`, then a restart of the hub, then a
  check from another machine that the hub's mesh endpoint answers with the new version
  (`docs/MESH.md`).
- **Browser compute is not in it.** The portable core and the browser's recomputed results
  (pull requests #1309, #1343, #1345) are outside the alpha.
- **No local inference.** Observing and routing to local model runtimes is planned for after
  the alpha; the catalogue's local vendors declare no model (`share/models.yaml`), and the
  crate calls no model.
- **Deployments are a draft feature** (`.ai/repo/features/deployments.md`).
- **Some modules are experimental.** The capability registry marks the modules delivery,
  guard, integration, mcp, mesh, models, monitor and reasoning `experimental`
  (`docs/generated/registry.json`): their capabilities work and are tested, and their
  inputs, outputs and names may still change before a stable release.
- **The executable uses the network where it is asked to.** The shell tool performs no network
  call except the declared loopback peer probe (`no-network`); the Rust executable is outside
  that scan. `majordomus serve` and `mcp` bind an HTTP server, on loopback unless `--host` or
  `MAJORDOMUS_HTTP_HOST` names another interface; the mesh links the runtimes a policy enrols;
  and the integration commands ask the forge through `gh` when they are run. None of these
  calls a model or sends telemetry.
- **A server bound beyond loopback exposes the layer to reading.** With the mesh enabled the
  server binds an interface other hosts can reach. Any host that can reach it can then read
  the repository's AI layer, its diagnostics and its peers, without authentication and
  unencrypted. A write from another host is refused (HTTP 403) unless it is a message signed
  by a linked mesh node (ADR 0126). The server warns about this at start. Bind `127.0.0.1`
  unless a reachable server is intended, and reach a remote one over a private network.

## Alternatives rejected

- **Hand-editing the GitHub release body.** It lives outside the repository, nothing checks
  it, and it would be the kept changelog ADR 0029 refuses.
- **A `CHANGELOG.md` or a release-notes file read by nothing.** Authored text no tool prints
  is the same defect with a path.
- **Switching `release.yml` to publish `majordomus release changelog` before the alpha.**
  It is the right end state and a release-path change; it is I2826, after the alpha.

## Consequences

- `majordomus release changelog` prints this declaration in the `v1.0.0-alpha.1` section,
  provided its date falls after the previous release's and on or before the alpha's: if the
  alpha moves, this date moves with it.
- The GitHub release body still lists pull requests until I2826 lands; until then this
  declaration is read through `majordomus release changelog` and the site's ADR pages.
- A line here that a later change makes false is a defect in this record, corrected by a
  superseding decision, never by an edit after the tag.
