<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `machine` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 1.0.0-alpha.8 -->
# Module `machine` — Machine

What one machine admits at once: a counting slot for heavy work (a build, a test run), kept beside the derive lock where every checkout of the machine agrees on it, and held to rule project.the-cooperation-model-is-the-owners-word clause 7 — at most the declared number of holders, and none admitted below the declared free disk — with the numbers read from the policy's machine.heavy_work.

Stability: experimental. Capabilities: 3.

## `machine.slot.acquire` — Take a machine slot for heavy work

Take one of the machine's slots for a running process, named by its pid, with a purpose a person reads. Holders whose process is proven gone — the pid no longer runs, or runs with another start time — are reclaimed first and named. Below the declared free disk it refuses at once; with every slot held it waits in arrival order, listed as a waiter, up to the bound (the input's, else the policy's), and then refuses. A refusal is an answer, not an error: it names the limit, the measured value, every holder with its pid, purpose and age, and what to do instead. A process that already holds a slot is told so and holds one, not two.

| | |
|---|---|
| kind | command |
| stability | experimental |
| MCP tool | `majordomus_machine_slot_acquire` |
| HTTP | `POST /api/v1/machine/slot/acquire` |
| CLI | `majordomus machine slot acquire` |
| cache | — |
| benchmark | waived (destructive) |
| provenance | builtin majordomus_cli::capability::builtin::machine_slot |
| tags | machine, build, coordination |

| input | type | required | description |
|---|---|---|---|
| `pid` | integer | yes | The process that will hold the slot: the build's own shell, not the caller of this
capability. It must be running; the slot is reclaimed when it stops. |
| `purpose` | string | yes | What the holder is about to do, in a person's words: it is what every waiter and
every refusal names. |
| `wait_seconds` | integer or null | no | How long to wait for a slot, in seconds; the policy's `machine.heavy_work.wait_seconds`
when absent, and no wait when that is absent too. |
| `volume` | string or null | no | A path the work writes to, whose volume the disk floor is judged on; the slot
directory's own volume when absent. |

Output: `SlotAnswer`.

## `machine.slot.release` — Give a machine slot back

Give back the slot a process holds, and withdraw it from the queue if it waits; the next waiter is admitted on its next look. Releasing a slot nobody holds answers `released: false` and changes nothing.

| | |
|---|---|
| kind | command |
| stability | experimental |
| MCP tool | `majordomus_machine_slot_release` |
| HTTP | `POST /api/v1/machine/slot/release` |
| CLI | `majordomus machine slot release` |
| cache | — |
| benchmark | waived (destructive) |
| provenance | builtin majordomus_cli::capability::builtin::machine_slot |
| tags | machine, build, coordination |

| input | type | required | description |
|---|---|---|---|
| `pid` | integer | yes | The process whose slot is given back. |

Output: `SlotRelease`.

## `machine.slot.status` — Who holds the machine's slots, and who waits

The slot directory as it stands: the limits and where they are declared, the free space on the volume measured, every holder with its pid, purpose, age and whether its process still runs (a pid that runs with another start time is a reused pid, and dead), and every waiter with how long it has waited and what it waits on (`machine.slot: 5 of 5 held`) — the progress a waiting session reports. Reads only: a dead holder is reported here and reclaimed by the next acquisition.

| | |
|---|---|
| kind | query |
| stability | experimental |
| MCP tool | `majordomus_machine_slot_status` |
| HTTP | `GET /api/v1/machine/slot/status` |
| CLI | `majordomus machine slot status` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::machine_slot |
| tags | machine, build, coordination |

| input | type | required | description |
|---|---|---|---|
| `volume` | string or null | no | A path the work writes to, whose volume's free space is measured; the slot
directory's own volume when absent. |

Output: `SlotStatus`.

