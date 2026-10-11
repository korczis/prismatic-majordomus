# RELEASE 15 · 07 `release land`: one typed command, one trail

## Operating contract
As in 00. This is I2299 ("the release chain is one typed capability"), wanted by the executor on
2026-10-10. New behaviour is a typed capability of the Rust executable declared with `capability!`
and projected by `majordomus generate`, never a new shell script (ADR 0069, ADR 0004).

## Objective
One command lands a release and says, at every moment, which stage it is in, how long each stage
took against the budget, and what it is waiting for: `majordomus release land <version>` with
`--dry-run`, an audit trail, and the same answer over CLI, HTTP, MCP and the Cockpit.

## Issue 1: The stages are typed
verdict of the commit -> tag -> builds -> published -> metadata served -> smoke green -> record
proposed -> record merged -> master deployed. Each stage has its evidence (run id, digest, URL
answer), its timestamps and its budget from prompt 00.
Acceptance: unit tests per transition; a case with a scripted forge and site.

## Issue 2: It recovers instead of stopping
A failed stage names the remedy and the command that applies it, and `release land --resume`
continues from the last proved stage. The v0.20.0 path (publish died, record by hand, Pages queued,
smoke never run) is a regression fixture.
Acceptance: the fixture runs to "landed".

## Issue 3: It reports into the mesh
While a release is in flight, its stage and age are announced on the board and the mesh, and the
hold on PR pushes it implies (if any remains after prompt 01) is a typed claim, released by the
command, never a paragraph in a session's announcement.
Acceptance: case: a mesh claim taken at tag, released at "metadata served".

## Completion report
A real release landed by the command: the timeline, the budget, the trail.
