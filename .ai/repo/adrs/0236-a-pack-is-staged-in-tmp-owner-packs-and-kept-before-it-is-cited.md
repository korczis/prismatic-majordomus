---
schema: adr/v1
id: adr-0236
kind: adr
title: A prompt pack in progress is staged in the primary checkout's ignored tmp/owner-packs, and is kept under campaigns before any record cites it
status: accepted
date: 2026-11-24
tags:
  - provenance
  - campaigns
  - intake
related:
  - file:campaigns/README.md
  - file:docs/OWNER_PACKS.md
  - file:scripts/ci/campaigns-check
  - file:.ai/repo/ci/campaigns-staged-citations.txt
  - file:.ai/repo/ci/gates.yaml
  - file:.ai/repo/project/issues/I4714.yaml
  - file:.ai/repo/project/issues/I4715.yaml
  - file:.ai/repo/project/issues/I4716.yaml
  - file:.ai/repo/project/milestones/a-paste-is-decomposed-by-a-skill.yaml
  - file:.ai/repo/project/milestones/campaigns-are-read-as-provenance.yaml
  - file:.ai/repo/project/issues/I3169.yaml
  - file:.ai/repo/adrs/0124-an-uploaded-prompt-pack-is-a-provenance-tracked-input-artifa.md
  - file:.ai/repo/adrs/0180-a-prompt-pack-is-a-campaign-kept-as-received-indexed-as-a-kind-a.md
  - file:.ai/repo/adrs/0194-an-ingested-brief-is-an-intake-revision-and-its-requirements-are.md
  - test:test/cases/340_a_brief_is_kept_as_received.sh
  - test:test/cases/4662_a_brief_is_cited_where_it_is_kept.sh
  - rule:project.a-brief-is-kept-as-received
  - rule:project.a-brief-is-cited-where-it-is-kept
provenance:
  origin: authored
---

# 236. A prompt pack in progress is staged in the primary checkout's ignored tmp/owner-packs, and is kept under campaigns before any record cites it

## Context

The repository keeps every prompt pack that drove work byte for byte under `campaigns/`
(`campaigns/README.md`, rule `project.a-brief-is-kept-as-received`, `scripts/ci/campaigns-check`).
That covers a pack once it is kept. Nothing covered the time before: a pack is downloaded or
pasted, unpacked, drafted further, and worked from while it is still being written.

Measured on 2026-11-23 in the primary checkout at master ae13be1bd13:

- `.gitignore` ignores `/tmp/`. Under it, three directories hold packs by habit, none by
  contract: `tmp/owner-packs/` (packs of 2026-10 and later, with an `INDEX.md`, a
  `delivery-packs-INDEX.md` and a `drafts/` directory), `tmp/prompt-packs/` and `tmp/packs/`
  (packs of 2026-09). `tmp/packs/` is also where `majordomus pack build` writes its output.
- `.ai/repo/scope.yaml` excludes `**/tmp/`, so no capability of the tool sees any of them.
- `tmp/` exists in the checkout it was made in. A linked worktree under
  `prismatic-majordomus-wt/` has a `tmp/` of its own, empty, so a pack staged in the primary is
  invisible from every worktree in which work is done.
- The "taken from" column of `campaigns/README.md` names `tmp/packs/`, `tmp/prompt-packs/` and
  `tmp/owner-packs/` as the origin of kept packs.
- Two records, I2460 and I2461, cite an audit (`AUDIT-M00.md`) written beside the pack of
  2026-10-11 in `tmp/owner-packs/<pack>/`, which was never kept. A clone cannot open it.
- Two untracked intent records in the primary checkout's `.ai/repo/project/intents/` are pastes
  kept verbatim as statements, the problem the milestone `a-paste-is-decomposed-by-a-skill`
  names.

## The owner's answers

Asked through AskUserQuestion in Claude Code session `0745dcdc-4b78-4b4a-bd7a-653d1e4a7ded`
on 2026-11-23, before 04:22:47Z (the start of task `t-20261123042247-589b`), after the owner
asked whether the repository has a git-excluded directory for in-progress and intermediate
prompt packs, and if not, to make one with UI/UX/DX tooling, document it, wire it into the
derived, inferred and generic surfaces, and enforce it through `.ai` when packs are uploaded.

> **Question:** Where should the git-excluded staging directory for in-progress prompt packs
> live? Today they sit in tmp/owner-packs/ (also tmp/prompt-packs/, tmp/packs/), which is
> outside .ai/repo/scope.yaml and exists only in the primary checkout, invisible from linked
> worktrees.
>
> **Answer:** tmp/owner-packs/ (Recommended)

> **Question:** How much should the first delivery include?
>
> **Answer:** Phased: 1 then 2 (Recommended)

The recommended option of the first question read: keep the existing location (40+ packs, no
migration), declare it as a contract, let tooling resolve it to the primary checkout's
`tmp/owner-packs/` from any worktree, and read `tmp/prompt-packs/` and `tmp/packs/` as legacy.
The second: a first pull request with the contract, ADR, rule, documentation, the
capability and enforcement, and a second with the `/ui` screen and its browser test.

The owner accepted this decision through AskUserQuestion in the same session on 2026-11-24,
before 18:56:11Z, after the first delivery (I4714, pull request #3119) was reported:

> **Question:** Fáze 1 (adresář tmp/owner-packs/, vynucení, dokumentace) je hotová v PR #3119
> a čeká jen na opravu CI v PR #3092. Tooling (I4715 výpis + doctor varování, I4716 /ui
> obrazovka) podle milníku smí přistát až po tvém přijetí ADR. Disk má teď 71 GiB, takže Rust
> build jde. Jak pokračovat?
>
> **Answer:** Přijímám 0236, stavěj I4715 (Recommended)

The option read: record the acceptance of ADR 0236 in the repository and build I4715, the
typed `campaigns.staged` capability with its warning in context and doctor; the listing does
not need the campaign kind of ADR 0180, so I3169 still waits for 0180. ADR 0180 and ADR 0194
stay proposed.

## Decision

1. **The staging place is `tmp/owner-packs/<pack>/` in the primary checkout.** It stays
   ignored. A pack there is in progress: it may be drafted, replaced or deleted, and nothing
   about it is evidence. Its `drafts/` subdirectory holds drafts that are not yet packs.
   `tmp/prompt-packs/` is read as the legacy staging place. `tmp/packs/` is not a staging
   place: it is `majordomus pack` output, and nothing here reads it as a pack.
2. **The primary checkout is derived, never configured.** It is the work tree whose `.git` is
   the repository's common directory (`git rev-parse --git-common-dir`), so a worker in any
   linked worktree reaches the same staging place without copying it.
3. **A pack is kept before it is cited.** The first record under `.ai/repo/project/` or ADR
   that is planned from a pack cites `campaigns/<pack>`, and the pack is copied there byte for
   byte first, with its manifest lines and its index row (`docs/OWNER_PACKS.md`). A citation
   of a staging place fails `scripts/ci/campaigns-check` step 5 unless
   `.ai/repo/ci/campaigns-staged-citations.txt` names it with its reason; that list only
   shrinks, and starts with I2460 and I2461. Rule: `project.a-brief-is-cited-where-it-is-kept`.
4. **The gate runs when a record changes.** The `campaigns` class of `.ai/repo/ci/gates.yaml`
   now matches `.ai/repo/project/**` and `.ai/repo/adrs/**`. Before this, an edit to a record
   did not select `campaigns-provenance`, although step 4 already decided what records cite.
5. **The tooling is typed capabilities, not shell (ADR 0069), and joins the plan that already
   exists.** The milestone `campaigns-are-read-as-provenance` (ADR 0180, proposed) already plans
   a campaign kind (I3167), citations (I3168) and one safe command that adds a pack (I3169),
   under the ingestion model of ADR 0124 and the noun of ADR 0194 (both proposed): a pack is
   untrusted input, and capturing it executes nothing. This decision adds the place before
   capture and does not plan a second way in:
   - **Listing (I4715, new).** A read-only capability lists every pack in the staging places
     with a derived state: `staged` (not kept), `kept` (every staged file has the bytes the
     manifest records), `diverged` (kept, and the staged copy differs from it), `planned` (kept
     and cited by a record), `unplanned` (kept and named in
     `.ai/repo/ci/campaigns-unplanned.txt`), each with its file count, first heading, newest
     modification time and the records that cite it. `majordomus generate` projects it to the
     CLI, MCP, HTTP, the generated skill and the capability documentation. `majordomus context`
     and `doctor` report a pack that has stayed `staged` past a bound the policy declares,
     because a brief on one disk is the loss the rule names.
   - **Keeping (I3169, extended).** The `campaign add` command I3169 plans also takes a pack by
     its name in the staging place, resolved from any worktree, besides a path to a ZIP or a
     directory. It copies byte for byte, appends the manifest lines and the index member, and
     refuses an unsafe archive before a write.
   The milestone's risk stands for I3169: the campaign ADR is accepted before its code lands.
   I4715 lands under this decision alone, as the owner chose on acceptance: it reads a
   git-ignored directory and ingests nothing, so it is not the ingestion surface ADR 0124
   point 3 holds back, and its noun is the name the tracked `campaigns/` directory and
   `scripts/ci/campaigns-check` already carry. If the owner chooses `intake` (ADR 0194) over
   `campaign` (ADR 0180), the capability is renamed with the kind.
6. **The screen is the second delivery.** I4716 adds a `/ui` screen on the loopback server
   that renders `campaigns.staged`: staged, kept, planned, each pack linked to its records
   and each record back to its pack, built on Flowbite Pro with the `--mj-*` tokens, held by
   a Playwright test in a real browser. Staged packs are never published: the public site
   shows only kept packs.

## Alternatives rejected

- **A new root directory `packs/`.** Clearer name, but forty packs would move and every
  "taken from" row would name a place that no longer exists. Still one checkout's.
- **The git common directory (`.git/majordomus/packs`).** Shared by every worktree natively
  and never trackable, but hidden from the editor and the file browser the owner unpacks
  downloads into.
- **Tracking packs as soon as they arrive.** A pack in progress is edited, and an edited
  tracked pack is what `project.a-brief-is-kept-as-received` refuses. Keeping happens once,
  when work is planned from the pack.
- **Extending `campaigns-check` with a listing mode.** New repository automation is a typed
  capability (ADR 0069); the shell gate gains only the refusal it already owns.
- **A `promote` operation of its own.** I3169 already plans the one command that keeps a pack,
  with the archive limits an untrusted pack needs; a second command would be a second way in.

## Consequences

- A worker in a linked worktree finds an in-progress pack through the listing once I4715
  lands, instead of asking which checkout holds it.
- A record can no longer be sourced from bytes only one disk holds; the two that are, are
  listed and can only be removed.
- Until I4715 lands, the staging place is a documented convention with a gate on its one
  failure that matters, and the listing and promotion are by hand as `campaigns/README.md`
  describes.
- The phased delivery the owner chose runs in three steps rather than two: the first pull
  request carries the contract, this ADR, the rule, the gate and the documentation (I4714);
  the capability (I4715 with I3169) follows once the campaign ADR is accepted, as the
  milestone's risk requires; the screen (I4716) follows the capability it renders.

## Verification

- `scripts/ci/campaigns-check` passes on this tree, and fails a fixture whose issue or ADR
  cites a staging place: `test/cases/4662_a_brief_is_cited_where_it_is_kept.sh`.
- The check at origin/master 93739746f8d passes the same fixture (measured on 2026-11-24), so
  the case refuses the build without step 5.
- `test/cases/340_a_brief_is_kept_as_received.sh` still passes.
