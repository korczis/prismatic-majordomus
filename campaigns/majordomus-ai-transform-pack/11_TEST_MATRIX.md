# Required test matrix

Extend existing behavioral cases rather than replacing them with unit-only
coverage.

## Preserve current suites

Existing cases around these areas remain relevant:

```text
init
doctor
update
start/check
handover
finish
watch
region projections
wiring dispatcher
profiles
doctrine enforcement/wiring
end-to-end
checkpoint
decision/question
history
context
prompt/search
continuity
CI wiring
foreign task
catalogue
no hardcoded values
project model/status/DAG
plan
GitHub projection
cross-surface
mutation
roadmap
blocker across handover
derived artifacts
sessions
knowledge discovery
```

Path assertions must be updated deliberately, not mass-replaced without reading
the intended contract.

## Add: tool installation independence

Cases:

1. executable from repository source checkout,
2. executable reached via PATH from another directory,
3. optional `.majordomus/` tool installation if practical,
4. tool distribution read-only,
5. repository writes only under `.ai/` plus declared projections.

## Add: fresh init layout

Assert:

```text
.ai/README.md exists
.ai/manifest.yaml exists
.ai/repo/policy.yaml exists
.ai/repo/profiles exist
.ai/repo/rules/vendor/majordomus manifest exists
.ai/local is ignored
.majordomus project-data directory is absent
```

## Add: local state

Assert:

- start writes under `.ai/local/state`,
- state files are ignored,
- check/finish read the new state,
- context/session/checkpoint/handover use new state,
- local state does not appear in `git status`,
- local state is not copied into site data,
- fresh clone has no task/session state.

## Add: worktrees

Create multiple temporary real Git worktrees.

Assert:

- each has independent `.ai/local/`,
- active task state does not leak by Git checkout,
- overlap reporting still sees another worktree claim by inspecting worktrees,
- stale branch/head semantics still work.

## Add: `.ai` discovery

Assert:

- registered rule source is discovered,
- unregistered random `.ai/repo/foo.md` is not implicitly normative,
- `.ai/local/**` is never implicitly discovered,
- order is deterministic.

## Add: rule DAG

Mutation tests:

- missing dependency,
- dependency cycle,
- duplicate ID/version,
- malformed frontmatter,
- unknown mandatory field where schema rejects it,
- missing `x-majordomus.validator`,
- validator missing from code,
- command omitted from dispatcher,
- blocking return swallowed,
- test path missing,
- CI not running required evidence test.

`doctor` must go red for each broken chain.

## Add: vendor rules

Assert:

- baseline is vendored on init,
- vendor manifest matches contents,
- hand edit in vendor is detected/refused by update/doctor as designed,
- newer executable does not silently alter repo baseline,
- explicit vendor update changes tracked content,
- project rules survive vendor update byte-for-byte.

## Add: bootstrap

Assert:

- AGENTS mentions README and `.ai/README.md`,
- README mentions AGENTS,
- provider bootstraps contain no monolithic duplicated rule body,
- every provider resolves the same effective rule set,
- always-loaded line budget remains enforced on generated bootstrap content where applicable.

## Add: migration

Fixtures:

1. pure legacy project `.majordomus/`,
2. already-migrated `.ai/`,
3. optional new tool `.majordomus/`,
4. ambiguous `.majordomus/` containing both old project policy and tool distribution,
5. legacy repo with unknown extra files,
6. legacy repo with dirty operational state.

Assert:

- dry-run/preview,
- no data loss,
- correct refusal,
- idempotence,
- post-migration doctor.

## Projection provenance

Whichever design is chosen, test:

- unmodified generated target,
- policy changed then update,
- target hand edited then policy changed,
- region host edited outside managed region,
- region edited inside managed region,
- fresh clone then canonical input changed,
- force path where supported.

Do not accept weaker hand-edit protection than current behavior.
