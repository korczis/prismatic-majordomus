# Profiles: the trace from the file to finish

A profile is one working mode: how much context a task is assembled with, how much effort
it is worth, how the worker reports, and what it must produce before it may finish
([`.ai/repo/profiles/README.md`](../.ai/repo/profiles/README.md)). This document follows a
profile from its YAML file through every program that reads it, and marks each link with
the code that does the reading and the test that proves it, or says that no test does.
It is the audit that milestone `a-profile-is-resolved-once-and-enforced` is built on
(issue I4500). The decision it led to is
[ADR 0232](../.ai/repo/adrs/0232-one-resolver-in-the-profile-module-owns-the-effective-profile.md):
one resolver owns the effective profile, and every reader below becomes a consumer of it.

Everything here was measured at d7fd9a36170. A line number is a claim about that commit.
When a row's code moves, the row moves with it.

## Standing

Each link has one of four standings:

| Standing | Meaning |
|---|---|
| **proven** | the named test fails when the link is removed |
| **advisory** | the link exists and acts, but nothing refuses when it is ignored, or no test holds it |
| **duplicated** | a second reader computes what another reader already computes, and the two can disagree |
| **missing** | an input or a consumer the model needs, and no code joins them |

## The path of a profile

```
.ai/repo/profiles/<name>.yaml           (seeded from share/skeleton/profiles by init/migrate)
  -> validation         doctor: parse, allowlist, name = file, default exists, limits
                        index: JSON schema majordomus.profile/v1, per object
  -> selection          start: --profile, else policy profiles.default   (issue's profile ignored)
  -> task record        current.yaml profile: <name>; ledger task.started {profile}
  -> worker context     context: TASK, PROFILE, toggles; check --explain: the flattened file
  -> enforcement        check: checkpoint freshness (advisory class)
                        finish: verify command, regression test, decision record (blocking)
  -> records            checkpoint, handover, session, session-context: the name only
  -> other readers      reasoning mode, proceed effort, completion gates, plan, Cockpit, site
```

There is no resolver. Every reader opens the file, or the object the index made of it, by
itself.

## The trace

Path is the file and line at d7fd9a36170. "Reads" is what it takes from a profile.
"Does" is what it does with it. "Consumer" is who sees the result.

### Validation and seeding

| Path | Reads | Does | Consumer | Standing |
|---|---|---|---|---|
| `lib/init.sh:94`, `lib/init.sh:98` | `share/skeleton/profiles/*.yaml` | copies the shipped profiles and the README into the new layer | the adopting repository | proven: `test/cases/01_init.sh`, `test/cases/16_profiles.sh` (a fresh install's profiles all run) |
| `lib/migrate.sh:113`, `lib/migrate.sh:239` | a legacy `profiles/` tree; the skeleton | moves legacy profiles into `sections.profiles`, then seeds what is missing | a migrated repository | proven: `test/cases/66_migrate_legacy.sh` |
| `lib/doctor.sh:110` to `lib/doctor.sh:123` | every `<profiles>/*.yaml`, through `mj_load_profile` | parse, unknown keys against `share/allow/profile.txt`, `name` equals the file name, `profiles.default` has a file | `doctor` | proven: `test/cases/02_doctor_basic.sh` (name mismatch, `bogus_key`, missing default) |
| `lib/doctor.sh:133` to `lib/doctor.sh:196` (`mj_profile_limits_reconcile`) | `share/profile-limits.yaml`; the declared axes of every profile; `# majordomus-limit:` markers under `lib/` | every declared axis set on every profile; every adapter carries its marker; prints each profile's limits | `doctor` | proven: `test/cases/4400_a_profile_axis_names_the_limit_it_derives.sh` |
| `share/schemas/majordomus/profile/profile.v1.schema.json` and `apps/majordomus-cli/src/generate.rs:2213` | the schema | the schema is the source of `share/allow/profile.txt` (`x-majordomus-allow: profile`) | `doctor`'s key check | proven: `bin/majordomus generate --check` refuses a stale allowlist |
| `share/kinds.yaml:88` and `apps/majordomus-cli/src/metadata/mod.rs:221` | every profile object | validates each object against `majordomus.profile/v1`, enums included; identity is `name` | the object index: MCP, HTTP, `majordomus get` | advisory for profiles: the mechanism is proven on a rule (`apps/majordomus-cli/tests/metadata_contract.rs` `a_schema_violation_names_the_field_and_the_constraint`), and on profiles only for malformed YAML and an empty class (`malformed_yaml_in_a_yaml_kind_is_named`, `a_required_class_that_discovers_nothing_is_an_error`). No test feeds a profile a bad enum |
| `.ai/repo/profiles/README.md` front matter, read by `apps/majordomus-cli/src/layout.rs` | the directory's contract | the README declares no `layout:` block, so `layout.check` holds the directory to no lifecycle. `layout.rs:628` declares one only in a unit-test fixture | `layout.check` | advisory: no contract is declared for the shipped directory |
| `share/commands.yaml:88`, `share/commands.yaml:106`, `share/commands.yaml:142`, `share/commands.yaml:160`, `share/commands.yaml:214`, `share/commands.yaml:236` | nothing; a declaration | names `.ai/repo/profiles/` among the `reads` of update, doctor, start, context, check and finish | the command reference, the site | advisory: `watch` reads a profile (`lib/watch.sh:29`) and does not declare it |

`doctor` checks keys, not values. `effort: max` on `routine` passes `doctor`, and only the
index's schema validation refuses `effort: maximal`. The two validators are separate
programs.

### Selection and the task record

| Path | Reads | Does | Consumer | Standing |
|---|---|---|---|---|
| `lib/start.sh:22`, `lib/start.sh:61`, `lib/start.sh:62` | `--profile`, else `profiles.default` of the policy; the file's existence | refuses a name with no file (exit 12) | the worker | proven: `test/cases/16_profiles.sh` (`no-such-profile`), `test/cases/04_start_check.sh:11` |
| `lib/start.sh:144` | the name | writes `profile: <name>` into `current.yaml` | every later command | proven: `test/cases/04_start_check.sh:13` (`profile=debugging`), `test/cases/16_profiles.sh` |
| `lib/start.sh:160`, `share/events.yaml:29` | the name | appends `task.started` with `profile`, which the event registry requires | ledger, `history` (`lib/history.sh:156`) | proven: `test/cases/04_start_check.sh:17`, `test/cases/22_history.sh` |
| `lib/start.sh:61` against an issue's `profile:` | nothing | `start --issue <id>` without `--profile` takes the policy default, not the issue's profile | the worker | **missing**: the issue's `profile:` never reaches the task |

### Worker context

| Path | Reads | Does | Consumer | Standing |
|---|---|---|---|---|
| `lib/check.sh:36` to `lib/check.sh:46` | the task's profile, flattened whole | `check --explain` prints every key as `key=value` under `# profile <name>` | the worker | proven: `test/cases/16_profiles.sh` (capability, effort, verbosity, checkpoint_interval per shipped profile), `test/cases/04_start_check.sh:43` |
| `lib/context.sh:165` | the task's profile | an unknown profile is named under EXCLUDED, and every toggle is treated as unknown | `context` | advisory: no test starts a task whose profile file then disappears |
| `lib/context.sh:201` | `context.task` | `false` drops the TASK section, with the reason named | `context` | advisory: no shipped profile sets it to `false`, and no test does |
| `lib/context.sh:214` | `checkpoint_interval` | printed on the TASK checkpoint line | `context` | advisory |
| `lib/context.sh:217` to `lib/context.sh:229` | `capability`, `effort`, `verbosity`, `presentation`, `reasoning`, `verification.*`, `output_contract` | the PROFILE section, the worker's statement of its constraints | the worker | proven for the section and `capability`: `test/cases/23_context.sh:27`, `test/cases/23_context.sh:33`, `test/cases/23_context.sh:34`. The other lines are printed, and no test asserts them |
| `lib/context.sh:148` (`mj_ctx_history`, limit `history_events`) | `context.recent_history_depth` | that many ledger events, or none with the reason named | `context` | proven: `test/cases/23_context.sh` (50 under debugging; `0` named under routine) |
| `lib/context.sh:313` to `lib/context.sh:322` | `context.architecture_notes`, `context.decisions` | repository-wide decisions, task decisions, or none | `context` | proven: `test/cases/23_context.sh` (debugging task-scoped, deep-work repository-wide, routine named) |
| `lib/context.sh:329` | `context.decisions` | the REASONING section rides on the decisions toggle | `context` | advisory |
| `lib/context.sh:337`, `lib/context.sh:349` | `context.current_state` | latest checkpoint and handover, unless `false` | `context` | advisory: no shipped profile sets `false` |
| `lib/context.sh:371`, `lib/context.sh:387` | `context.relevant_files` | files touched inside the scope, or the exclusion named | `context` | proven: `test/cases/23_context.sh` |
| `lib/context.sh:388` | `context.failing_output` | names that Majordomus does not capture output, and captures nothing | `context` | proven: `test/cases/23_context.sh:58`. The axis is honest about being unenforceable |
| `lib/context.sh:440` (`mj_context_pointers`, limit `knowledge_pointers`) | `context.knowledge`, `context.knowledge_max_items`, clamped by the policy's `context.max_list_items` (`lib/context.sh:453`) | a bounded slice of the knowledge graph | `context` | proven: `test/cases/3073_context_knowledge_slice.sh` (2, 500 clamped, switched off) |
| `apps/majordomus-cli/src/devcontext/select.rs:251`, `apps/majordomus-cli/src/devcontext/select.rs:622` | profile objects as governance; an object's `profile` fact | tiers profile objects; carries `profile` as a fact; applies no toggle by design (`devcontext/mod.rs:13`) | `devcontext.compile` | advisory: by its own contract it is not the session briefing |

### Enforcement

| Path | Reads | Does | Consumer | Standing |
|---|---|---|---|---|
| `lib/check.sh:265` (`mj_validate_checkpoint`, limit `checkpoint_max_age_seconds`) | `checkpoint_interval`, 900 s when it does not parse | the newest checkpoint is no older than the interval | `check`, `watch` | proven as advisory: rule `majordomus.checkpoint-freshness` is `class: advisory`. `test/cases/04_start_check.sh:119` (`WARN`, check exits 0), `test/cases/17_doctrine_enforcement.sh` (flipped to blocking, it refuses) |
| `lib/watch.sh:29` | the task's profile file's existence | drift when the task names a profile with no file | `watch` | advisory: no test |
| `lib/finish.sh:52` | the task's profile | refuses finish when the file is gone (exit 12) | `finish` | advisory: no test removes the file mid-task |
| `lib/finish.sh:196` to `lib/finish.sh:202` | `verification.verify_command_required` (`true`, `false`, `if_files_changed`) | requires `--verify-command` and runs it | `finish` | proven for `true`: `test/cases/06_finish.sh:20`, `test/cases/16_profiles.sh`. Advisory for `if_files_changed`: no test finishes a routine task with no change and no verify command |
| `lib/finish.sh:246` to `lib/finish.sh:262` (`mj_validate_profile_requirements`) | `verification.regression_test_required`, `verification.decision_record_required` | a touched test path; a `Task: <id>` entry in `decisions.md` | `finish` (rule `majordomus.profile-requirements`, blocking) | proven: `test/cases/16_profiles.sh` (each shipped profile that sets either key is refused without it) |
| `apps/majordomus-cli/src/gates/done.rs:219` to `apps/majordomus-cli/src/gates/done.rs:252`, `apps/majordomus-cli/src/gates/done.rs:506` | no profile. Its evidence string names `verification.regression_test_required` | asks "regression-tested" of every task under any profile, with its own copy of finish's test-path predicate | `gates.done`, completion gates | **duplicated**: the copy says it is finish's predicate, and it is not. Finish's (`lib/finish.sh:252`) is case-insensitive and matches `src/test_helpers.py`, `src/tests.rs` and `Tests/a.rs`, and the copy matches none of them. Its unit tests (`a_test_path_is_recognised_the_way_the_validator_recognises_one`) never compare it with the shell regex |

### Projection, records and other readers

| Path | Reads | Does | Consumer | Standing |
|---|---|---|---|---|
| `lib/common.sh:760` (`mj_policy_cat`), `apps/majordomus-cli/src/policy.rs:465` | every profile file, normal form | part of the policy hash every bootstrap is stamped with | `update`, `generate providers` | proven: `apps/majordomus-cli/tests/providers_projection.rs` `a_comment_in_the_policy_moves_no_stamp_and_both_tools_read_it_alike`, `test/cases/3018_a_comment_in_the_policy_moves_no_bootstrap.sh` |
| `lib/update.sh:206`, `apps/majordomus-cli/src/providers.rs:181` | the policy's `profiles.default`, `profiles.checkpoint_interval_default` | `{{DEFAULT_PROFILE}}`, `{{CHECKPOINT_DEFAULT}}` in the provider bootstraps | every provider's instructions | proven: `test/cases/03_update.sh`, `test/cases/93_rust_provider_projections.sh`, `providers_projection.rs` |
| `.ai/repo/policy.yaml:15` against the default profile's `checkpoint_interval` | two values | the bootstrap tells a worker the policy's interval, and `check` enforces the profile's | the worker | **duplicated**: both are `15m` today, and nothing compares them |
| `lib/update.sh:176` to `lib/update.sh:188` (`mj_build_fragments`), `apps/majordomus-cli/src/providers.rs:168` | every profile, through `mj_load_profile` | builds `PROFILE_TABLE`, which no shipped template includes and the Rust renderer refuses | nobody | advisory: dead output, a shell reader with no consumer |
| `apps/majordomus-cli/src/reasoning/mod.rs:114` to `apps/majordomus-cli/src/reasoning/mod.rs:129` | `.ai/local/state/current.yaml` and `.ai/repo/profiles/<name>.yaml`, at hard-coded paths, by line match on `reasoning:` | the profile's reasoning mode, below `MAJORDOMUS_REASONING_MODE` and CI (`reasoning/availability.rs:174`) | `reasoning`, advisors | proven: unit test `the_open_tasks_profile_names_its_mode` and the doc test of `resolve_mode`. **Duplicated** in that it bypasses the manifest's `sections.profiles` and the YAML reader |
| `apps/majordomus-cli/src/capability/builtin/proceed/step.rs:543`, `apps/majordomus-cli/src/capability/builtin/proceed/step.rs:424` | each profile object's `effort`, from the index; the issue's `profile` | a work item's `profile` and `effort` | `proceed.step`, the loop that acts on it | proven: `an_issue_is_ordered_as_devtask_implement_under_its_own_key` (step.rs tests). **Duplicated**: a second reading of `effort` |
| `apps/majordomus-cli/src/plan.rs:242`, `apps/majordomus-cli/src/devtask/task.rs:403`, `lib/project.awk:403`, `lib/plan_json.awk:78`, `lib/plan.sh:347`, `lib/plan.sh:393` | an issue's `profile:` | carried through the plan, `plan next`, `plan show`, `devtask` | planners, the Cockpit (`cockpit/plan.rs:925`) | advisory: displayed. Its name is never checked against a profile file |
| `lib/common.sh:1463` (`mj_record_attribution`), `lib/common.sh:1818` (`mj_record_front_matter`), `lib/checkpoint.sh:47`, `lib/handover.sh:45`, `lib/session.sh:621`, `lib/session_context.sh:146`, `lib/session_context.sh:280`, `lib/recover.sh:335`, `lib/derive.sh:243`, `lib/prompt.sh:112` | the task's profile name | copies the name into a record's front matter or a prompt's `{{PROFILE}}`; never opens the file | the next worker | advisory: name carriers. No test asserts the name a record carries |
| `apps/majordomus-cli/src/continuity/record.rs:109`, `apps/majordomus-cli/src/continuity/local.rs:315`, `apps/majordomus-cli/src/continuity/mod.rs:1598`, `apps/majordomus-cli/src/capability/builtin/continuity.rs:1044`, `apps/majordomus-cli/src/mesh/handover.rs:416`, `apps/majordomus-cli/src/cockpit/pages.rs:2588` | a task's profile name | carried on a handover record and suggested as `start --profile` on resume. A mesh-published handover always writes `profile: none` | continuity, the Cockpit | advisory: carried by name, with a unit test of the carrier (`continuity/local.rs:677`) and none of the value |
| `apps/majordomus-cli/src/session/episode.rs:347` | nothing, by design | refuses `profile` on an episode: it belongs to the task | episodes | proven: the unit test at that line |
| `apps/majordomus-cli/src/skill/mod.rs:972` | `<profiles>/**` as an invocation surface | a skill a profile names counts as used | `skills` | advisory: shipped profiles name no skill |
| `scripts/generate-site-data:395`, `scripts/site-check:1390` | `share/skeleton/profiles/*.yaml`, not this repository's | the site's profile pages | the website | advisory: the site shows the skeleton, which lacks `reasoning:` |

`lib/knowledge.awk:180` and `lib/knowledge.sh:460` index a profile as a knowledge node by
`name` and `description` only, and so does the object index. Several matches for "profile"
in `lib/` and the Rust source are a different concept and are not rows: `share/archive.yaml`
and `lib/archive.sh` (archive profiles), `lib/rust_bin.sh` and `lib.rs:116` (the cargo build
profile), `design/kit.rs` (UI kit profiles), and the `mesh/build_cache` modules (an AWS
credential profile).

## Compatibility

How the four shipped profiles, and the files that describe them, stand against each other
and against the schema:

| | debugging | deep-work | implementation | routine |
|---|---|---|---|---|
| `.ai/repo/profiles/` equals `share/skeleton/profiles/` | no: the repository adds `reasoning: standard` | no: adds `reasoning: strict` | no: adds `reasoning: standard` | no: adds `reasoning: fast` |
| keys within `share/allow/profile.txt` | yes | yes | yes | yes |
| `effort_escalation` set | `after_blocked_attempts: 2`, `to: xhigh` | `1`, `xhigh` | no | no |
| `verification.verify_command_required` | `true` | `true` | `true` | `if_files_changed` |
| `verification.regression_test_required` | `true` | `false` | `false` | `false` |
| `verification.decision_record_required` | `false` | `true` | `false` | `false` |
| `checkpoint_interval` | `15m` | `30m` | `15m` | `30m` |

The rest of the model:

| Surface | Today | What the resolver must keep |
|---|---|---|
| File schema | `majordomus.profile/v1`, `additionalProperties: false`, seven required keys | v1 for additive optional keys (ADR 0232); a rename or a new required key is v2 with a migration |
| Allowlist | `share/allow/profile.txt`, generated from the schema | regenerated, never hand-edited |
| Task record | `current.yaml` `profile: <name>` | the name, plus the effective profile's fingerprint |
| Ledger | `task.started` requires `profile` (`share/events.yaml:29`) | the name stays; the fingerprint is added |
| Continuity record | `TaskRef.profile: Option<String>` (`continuity/record.rs:109`); `none` on a mesh handover | unchanged; a resume suggests the name |
| Policy | `profiles.default`, `profiles.checkpoint_interval_default` | the default stays an input; the interval default becomes a diagnostic when it disagrees with the default profile |

Two axes have no consumer at all. Nothing reads `effort_escalation`, and `output_contract`
is printed by `context` (`lib/context.sh:228`) and nothing checks it. The axes
`capability`, `effort`, `verbosity` and `presentation` are printed, and `effort` is
forwarded by `proceed.step`. None of them is enforced, and `share/profile-limits.yaml`
leaves them out on purpose.

## Surprises against the issue's own record

The issue was written at 51b38035a71. Re-measured at d7fd9a36170:

- I0201 is delivered (519e783abcb). `share/profile-limits.yaml`, its `doctor`
  reconciliation and the three `# majordomus-limit:` adapters are on master, and case 4400
  proves them. The issue record still carries no completion stamp.
- `apps/majordomus-cli/src/reasoning/mod.rs` reads a profile's file directly, and so is a
  Rust reader the issue does not list. `proceed/step.rs` reads `effort` from the index,
  which is a second.
- `gates/done.rs` does not read the profile at all. It asks the regression question of
  every task, while it cites the profile key.
- The policy's checkpoint interval and the default profile's are two declarations of one
  value.

## What is not covered yet

- No test or gate fails when a new reader of a profile appears that is not a row here.
  The test is a case under `test/cases/` that searches `lib/`, `share/` and the crate
  source for `mj_load_profile`, `mj_pro `, `profile_files`, `kind == "profile"` and the
  profiles directory, and compares every hit with the paths of this document's trace
  tables. `test/` is outside I4500's declared scope, so the case belongs to the resolver's
  first issue. Until then, this table can go stale without anything noticing.
- `docs/claims/profile-axes.md` says every axis is advisory. That is stale for the
  `context.*` toggles and the `verification.*` requirements. It is outside this audit's
  scope (see ADR 0232, Consequences).
