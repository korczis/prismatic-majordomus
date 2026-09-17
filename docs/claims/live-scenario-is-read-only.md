# A live scenario may only run commands share/commands.yaml declares read-only, names no setup, and asserts an obligation only where something owes one

## What it means

A use case's scenario declares where it runs. `mode: fixture`, the default, prepares a
disposable repository and runs the tool against it. `mode: live` asks its questions of the
repository the command was invoked in — and because that repository is somebody's work,
the tool holds a live scenario to what a question may do:

- every step's command must be `class: read-only` in `share/commands.yaml`;
- a live scenario names no setup, because it prepares nothing;
- an obligation step — a step that asserts an obligation is discharged rather than running
  anything — is valid only in a live scenario, because a disposable fixture owes nothing;
- a step is a command or an obligation, never both.

Each of these is decided from a declaration that already exists, so safety is a property
of the declaration rather than of the author's care. `usecase validate` refuses any other
shape with exit 10, naming the step, the command and the class that decided it.

## How it works

`mj_uc_validate` in `lib/usecase.sh` reads `scenario.mode`, refuses a mode that is neither
`fixture` nor `live`, and branches: a fixture scenario must name a setup that exists in the
fixture directory, a live one must name none. For each step it reads `run.0` and
`obligation`. An obligation step must carry no command, must be in a live scenario, and
must name a token `share/obligations.yaml` declares. A command step in a live scenario is
looked up in the command registry through `mj_cmdreg_class`, and anything other than
`read-only` — including an undeclared command — is refused.

The rest of the mechanism follows from this one: because a live scenario cannot mutate,
`mj_uc_run_live` needs no sandbox, and the mutating half of a workflow stays with `start`,
`check` and `finish`, which supervise it (ADR 38).

## How to see it

```bash
majordomus usecase validate                    # exit 10 names the step, the command and its class
majordomus usecase show know-whether-this-work-is-finished
bash test/run.sh 359_live_scenarios
```

`test/cases/359_live_scenarios.sh` plants each refused shape in turn — a live scenario with
a setup, a live step running a `state-mutating` command, an obligation step in a fixture, an
unknown obligation token, and a step that both runs and asserts — and requires exit 10 with
the message that names the cause.
