# Every bootstrap this repository generates leads a worker to one workflow for working beside other sessions; the layer discovers that workflow, and every tool, command and script it tells a worker to use exists

## What it means

A session that starts in this repository is told, in the bootstrap it always loads, that there is a workflow for working beside other sessions and where it is: `.ai/repo/workflows/working-beside-other-sessions.md`. The sentence that points there also carries the two instructions that cannot wait for the document to be opened: read the mesh as well as the board, and ask for an approval in the session that acts. The workflow itself is a document of the layer like `task-lifecycle.md`, found by the same discovery, and nothing it tells a worker to call is a name that does not exist.

## How it works

The workflow is a tracked file under the layer's workflows section, so the knowledge sources carry it in the class `workflow` and the registry serves it as a document resource. `AGENTS.md` and `CLAUDE.md` are rendered by `majordomus update` from the templates under `.ai/repo/providers/`, and the pointer is one sentence in each template. The bb bootstrap is rendered from the distribution's template, which is shipped to every adopter and cannot name a file only this repository has; it hands over to `CLAUDE.md` or `AGENTS.md`. The instruction about an approval reported by another session is clause 7 of `project.mesh-is-observation-not-authority`, which names the case among its tests.

`test/cases/1034_working_beside_other_sessions_is_written_where_it_is_read.sh` reads the checkout it lives in. It asks the shell tool and the Rust executable whether they discover the workflow beside `task-lifecycle.md`; it reads the policy's projections and requires the pointer in every template of this repository and in the file rendered from it; it requires the clause in the rule; and it takes every backticked name in the workflow and looks an MCP tool up in the capability registry, a command in the command graph, and a script or document in the tree. Two of its checks are also run against copies the case breaks, a template without the pointer and a workflow naming a tool nobody declared, and each must refuse.

## How to see it

```bash
bash test/run.sh 1034_working_beside_other_sessions_is_written_where_it_is_read
majordomus knowledge sources | grep 'workflows/working-beside-other-sessions.md'
grep -n 'working-beside-other-sessions.md' AGENTS.md CLAUDE.md
```

Then delete the paragraph that begins "Before you coordinate with another session" from `.ai/repo/providers/agents.tmpl`, run `majordomus update`, and run the case again: it fails naming the template and the workflow's path.

## What it does not cover

It does not cover what a session does after reading. Whether a session read the mesh, asked its own person, left its head to the lane, held its push or looked at the lock is conduct, decided by review and by the session's own permission layer; no gate observes it. It does not cover joining the mesh automatically when a session starts, which is separate work. And it is a claim about this repository's layer: an adopter's bootstraps are rendered from the distribution's templates and carry no pointer to a workflow the adopter does not have.

## Why it exists

On 2026-10-09 and 2026-10-10 about ten sessions on four machines shipped two releases through one integration executor and lost hours to five failures: an approval relayed between sessions and nearly acted on, messages on the mesh that the executor never read, heads brought up to master before their turn, a release tag that failed every other head until its record landed, and a lock left held by a stopped job. None was a disagreement about the work, and none was written where a session starting the next morning would read it. A lesson kept in one conversation is lost with the conversation, so the five are in the layer, the bootstrap points at them, and a case fails when the pointer, the document or a name in it goes missing.
