+++
title = "Entering the repository reports only what evidence proves"
description = "Entering the repository reports only what evidence proves"
weight = 82
[extra]
kind = "rule"
slug = "project-entry-reports-only-evidence-1"
identity = "project.entry-reports-only-evidence@1"
status = "active"
source = ".ai/repo/rules/project/entry-reports-only-evidence.v1.md"
+++
{% raw %}

## Rationale

On 2026-09-15 the banner drawn on entering this repository put a `✓` beside the Cockpit,
the API and MCP while the shared server behind them was serving version 0.6.1 to an
executable at 0.7.0. The mark was decided by a TCP connection. Nothing was broken in the
code that drew it: it reported what it measured. What it measured was not the claim a person
reads a check mark as — *this is running, for this checkout, at this version*.

The same shape recurs across the repository (`docs/ENTRY.md`, `docs/EVIDENCE.md`): a rule file
present is read as a rule enforced, a recorded test run as proof of whatever tree is checked
out now, a deployment script as a deployment. Each is a declaration standing in for evidence.
An entry surface is the worst place for that substitution, because it is the one surface every
worker reads, and reads before anything else.

## Required behaviour

- Every status an entry surface shows — the banner, `majordomus env preflight`, the HTTP route
  `/api/v1/environment/preflight`, the MCP tool `majordomus_preflight` and resource
  `majordomus://environment/preflight`, the Cockpit overview — is a rendering of one value,
  `environment::preflight::Preflight`, derived by `environment::preflight::derive`. No surface
  computes a status of its own.
- A verdict that asserts something is in force (`verified`, `active`, `fresh`) carries at least
  one piece of evidence naming what was read and what it said. `Check::new` refuses a bare one
  by turning it into `unknown`, so the rule is held in the type rather than in each renderer.
- A success mark is drawn only for such a verdict. A server that answers from another version is
  `degraded`; a lease naming a server that does not answer is `failed`; no lease is
  `unavailable`; a reading that did not ask is `unknown`.
- Rules discovered are not rules enforced: `governance.rules` counts the corpus and
  `verification.enforcement` is `verified` only when every rule owing an executable proof is
  proven against the tree.
- Evidence about another commit or tree is `stale`, never `verified`: recorded test runs are
  current only by the evidence module's own comparison, rule tallies only at the commit they
  were counted at, the deployment only when the deployment ref names HEAD as its source.
- What the repository records no evidence for — coverage, a generation check — is reported as
  `unavailable` or `unknown`, not omitted and not assumed.
- `.envrc` makes one call to the bootstrap command and holds no repository-domain logic
  (`project.envrc-is-an-adapter`). Client bootstrap files (`.mcp.json`, `.gemini/settings.json`,
  `.codex/config.toml`) start the launcher and decide nothing.

## Failure behaviour

A surface that draws a success without evidence, or a derivation that reports evidence about
another tree as current, fails `apps/majordomus-cli/tests/preflight.rs`. A surface that renders a
different verdict from the capability fails the cross-surface test in the same file. An entry that
stops rendering the preflight, or starts marking a service answering while its server is not
verified, fails `test/cases/358_entry_reports_only_evidence.sh`.

## Verification

`apps/majordomus-cli/tests/preflight.rs` holds each verdict to its evidence over hand-built
observations, reads a real fixture's deployment ref and ledger, and compares the command line,
the API, the MCP tool and resource and the Cockpit page over one served fixture.
`test/cases/358_entry_reports_only_evidence.sh` enters a fixture through the bootstrap command and
checks what is drawn against the preflight's own JSON.
{% endraw %}
