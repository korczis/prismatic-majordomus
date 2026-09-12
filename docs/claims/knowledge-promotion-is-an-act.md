# verified is written only by promote with evidence on stdin or by a person editing the file, never by the deriver

## What it means

The deriver writes candidates and nothing else. A record becomes `verified` through `majordomus knowledge promote <id>` with evidence on standard input, which moves it to `curated/`, or through a person editing the file. `majordomus knowledge reject <id> --reason "<why>"` sets it aside as `superseded` in place with the reason recorded. Each act leaves a ledger line, `knowledge.promoted` or `knowledge.rejected`, and the deriver skips a promoted or superseded id at the next boundary so that neither act is undone.

## How it works

`promote` reads the candidate, refuses (10) when stdin is empty — an act without evidence is an assertion — or carries a transcript marker, or when the record's status is not `candidate`, or when `--class` is outside the enumeration; it exits 12 when no candidate carries the id. It rewrites the front matter with `status: verified`, the class the person chose with `--class` when given, today's date, and `derived_from` unchanged plus the promoting episode when one is open; appends the evidence under `# Evidence`; writes `curated/<id>.md` through a temporary file and a rename; removes the candidate; appends `knowledge.promoted` with the id and the path; and prints the curated path last. `reject` rewrites the candidate in place as `superseded`, inserts `superseded_by` when `--by <id>` names an existing record, appends the reason under `# Rejected`, appends `knowledge.rejected` and prints the path last. `--reason` is required and non-empty (2).

## How to see it

```bash
majordomus knowledge candidates                         # the queue: id, class, date, branch, title
printf '' | majordomus knowledge promote <id>; echo "exit $?"                    # 10: no evidence
printf 'Verified by running test/cases/64_knowledge_discovery.sh.\n' \
  | majordomus knowledge promote <id> --class convention   # last line: .ai/repo/knowledge/curated/<id>.md
majordomus history --event knowledge.promoted
majordomus knowledge reject <other-id> --reason "restates ADR 0010"   # last line: the rewritten candidate's path
majordomus knowledge derive                             # skipped ... (promoted), skipped ... (superseded)
```

## What it does not cover

It does not decide which candidates deserve promotion; the advisory rule makes the queue visible and a person empties it. A person editing a candidate's status by hand is the other act and is not refused, though the integrity check still requires provenance beside `verified`. Promotion does not stage or commit the curated record.

## Why it exists

A tool that can write `verified` can turn its own inference into repository truth, and a reader months later cannot tell which. The same argument gave `adr propose` no flag for `accepted`. `.ai/repo/adrs/0058-knowledge-is-derived-at-the-episode-boundary-and-a-person-promotes-it.md` fixes that the deriver writes candidates only, that the class is the person's to set at promotion, and that evidence is required because an act without it is an assertion.
