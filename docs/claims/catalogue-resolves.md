# A use case cannot name something the tool does not have

## What it means

`share/use-cases.yaml` and `share/applications.yaml` describe what Majordomus is for. Every command a use case walks through, every rule it relies on, and every promise it makes is a reference to something else in the repository — and every one of those references is checked. So is the relationship between the two files: a use case naming an application that does not name it back is a failure, not an asymmetry somebody will notice later.

## How it works

`mj_validate_catalogue` in `lib/doctor.sh` flattens both registries with the tool's own YAML reader and resolves each reference against the thing that defines it: commands against `docs/CLI.md`, doctrines against the resolved rule set through `mj_doc_index`, claims against `docs/CLAIMS.yaml`. Cross-references between the two catalogues are checked in both directions by `mj_cat_has` and `mj_cat_back`. An application that declares no `does_not_fit_when` is refused outright.

The doctrine `majordomus.catalogue-integrity` is blocking and enforced by `doctor` and `watch`, so the same rule answers "is this catalogue sound" and "has it drifted" without a second implementation. `scripts/generate-site-data` repeats the resolution when it builds the pages, and refuses to emit if anything dangles — a broken reference cannot reach the published site.

## How to see it

```bash
majordomus doctor
# OK catalogue  6 use case(s), 4 application(s) — every command, doctrine, claim and cross-reference resolves, both directions
```

Change a doctrine id in `share/use-cases.yaml` to one that does not exist and `doctor` names the use case, the missing id, and the command that lists the real ones.

## What it does not cover

Resolution is not accuracy. A use case whose steps are in the wrong order, or whose situation describes a problem nobody has, passes every check here — each reference exists. What is enforced is that the catalogue cannot describe a tool other than this one; whether what it describes is worth doing is a judgement no validator makes.

## Why it exists

Documentation about a tool drifts faster than the tool, and drifts silently, because nothing executes it. Every failure this project cares about has the same shape — something declared, nothing checking that the declaration still corresponds to reality — and prose is the largest surface where that is usually accepted. Treating a use case as a set of references rather than as text makes it checkable, which is the only reason to write it as data at all.
