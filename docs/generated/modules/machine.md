<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `machine` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.29.0 -->
# Module `machine` — This machine's build output

What this repository's builds leave on the machine they run on — private build directories, coverage targets, incremental sessions under every cargo cache home — classified and, when asked, reclaimed; and the per-user scheduler entry that runs that sweep every fifteen minutes on every machine, whether or not a build of this repository ever starts there (I2881).

Stability: behaviorally_verified. Capabilities: 2.

## `machine.schedule` — Report, install or remove the scheduler entry that sweeps this machine every fifteen minutes

The per-user scheduler entry that runs `majordomus machine sweep --reclaim` every fifteen minutes from this checkout: a launchd agent (~/Library/LaunchAgents/dev.majordomus.sweep.plist) on macOS, a systemd --user service and timer (~/.config/systemd/user/majordomus-sweep.*) on Linux. `status` reports whether its files are there; `install` writes them and loads them with launchctl or systemctl, each command's outcome reported; `remove` unloads and deletes them. The entry starts the `majordomus` on PATH unless another program is named, so an upgrade does not leave it pointing at a removed version. Refused on a platform with neither scheduler.

| | |
|---|---|
| kind | command |
| stability | behaviorally_verified |
| MCP tool | `majordomus_machine_schedule` |
| HTTP | `POST /api/v1/machine/schedule` |
| CLI | `majordomus machine schedule` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::machine |
| tags | machine, disk, schedule, maintenance |

| input | type | required | description |
|---|---|---|---|
| `action` | SweepScheduleAction | no | `status` (the default), `install` or `remove`. |
| `floor_gib` | integer or null | no | The floor the scheduled sweep runs with, in GiB (default 30). |
| `program` | string or null | no | The executable the scheduler starts; by default the `majordomus` on PATH, so that an
upgrade does not leave the entry pointing at a removed version. |
| `activate` | boolean | no | Load or unload the entry with launchctl or systemctl after writing it (default true). |

Output: `SweepScheduleReport`.

## `machine.sweep` — Classify the build output under every cargo cache home of this machine, and reclaim what nothing uses

Walks every cargo cache home of this machine ($CARGO_HOME, ~/.cargo and each ~/.asdf/installs/rust/*, or the homes named): every directory under build-private/, every shared build directory's llvm-cov-target, and every incremental session. Each gets a verdict: unmeasurable when its age cannot be read, recent when written within the idle threshold (120 minutes, 30 for sessions), in_use when a running process names it (any coverage run, for a coverage target; every path when the process table cannot be read), held when its cargo lock cannot be taken, and otherwise reclaimable. A check by default; asked to reclaim, it asks each reclaimable directory again, renames it out of cargo's reach and removes it. Below the floor (30 GiB free by default) everything is swept at 30 minutes. Answers with the free space before and after, every candidate and the bytes reclaimed.

| | |
|---|---|
| kind | command |
| stability | behaviorally_verified |
| MCP tool | `majordomus_machine_sweep` |
| HTTP | `POST /api/v1/machine/sweep` |
| CLI | `majordomus machine sweep` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::machine |
| tags | machine, disk, build, maintenance |

| input | type | required | description |
|---|---|---|---|
| `check` | boolean | no | Classify and report, and remove nothing. The default, because the alternative deletes. |
| `idle_minutes` | integer or null | no | Minutes a private build directory or a coverage target must have been idle (default
120). Below the floor the sweep uses 30 whatever this says. |
| `floor_gib` | integer or null | no | GiB free below which the sweep is below the floor (default 30). |
| `cargo_homes` | array | no | The cargo cache homes to sweep; by default `$CARGO_HOME`, `~/.cargo` and every
`~/.asdf/installs/rust/*` that exists. |

Output: `BuildSweepReport`.

