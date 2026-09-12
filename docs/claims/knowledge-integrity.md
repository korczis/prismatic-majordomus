# Every knowledge record is schema-valid, uniquely identified, evidenced when verified, resolves every reference it names, and carries no conversation

## What it means

A knowledge record under `candidates/` or `curated/` is refused when it is not what the schema says a record is: an unknown key, a class or status outside the enumeration, an id that differs from its file name or that another record also claims, a `verified` record with no provenance, an `extracted` record naming no evidence, a `derived_from` or `relations` target that resolves to nothing, a `superseded` record that names no replacement and records no rejection, a candidate that claims to be verified, or a title, description or body line that carries a conversation. `check`, `doctor` and `majordomus knowledge check` all refuse it with a `FAIL knowledge` finding and exit 10.

## How it works

`mj_validate_knowledge_integrity` in `lib/knowledge.sh` flattens both directories once, checks each record's front matter against the required keys, the enumerations and the generated allow-list `share/allow/knowledge.txt`, and collects every reference into one list. References are resolved where their type says the target lives: a file or test in the tree, a session in the sessions store or the ledger, a task or decision in the ledger or the state directory, a commit in git through one batched `cat-file` query, an issue in the project section, a knowledge record in either directory, a rule or decision record in the layer. `decision:none` and `task:none` are refused explicitly. The transcript regex that keeps `project.never-store-transcripts` is applied to the title, the description and the body. Duplicate ids are checked across both directories. The graph compiler treats ledger and git targets as external and never reports them as dangling; this validator is what resolves them.

## How to see it

```bash
majordomus knowledge check                              # one line per finding; last line counts records and failures
printf -- '---\nschema: knowledge/v1\nid: bad\nkind: knowledge\nclass: fact\ntitle: "x"\ndescription: "y"\nstatus: verified\nepistemics: observed\ndate: 2026-09-12\nprovenance:\n  origin: extracted\n  derived_from:\n    - session:no-such-episode\n---\n\n# x\n' > .ai/repo/knowledge/candidates/bad.md
git add .ai/repo/knowledge/candidates/bad.md
majordomus knowledge check; echo "exit $?"               # FAIL knowledge ... claims verified under candidates/; ... does not resolve; exit 10
majordomus doctor | grep 'FAIL +knowledge'
```

## What it does not cover

It does not judge whether an assertion is true, useful or worth curating; that is the promotion act. It does not validate the README the directory carries, which is a context document and is validated as one. It does not resolve a reference against a remote: a commit git does not have locally is missing here even if it exists elsewhere.

## Why it exists

A record whose reference resolves to nothing is an assertion wearing a citation, and a reader months later cannot tell it from one that was true. The store has two directories and one status field, and the field is what separates them; a candidate that claims `verified` has skipped the act that gives the word its meaning. `.ai/repo/adrs/0058-knowledge-is-derived-at-the-episode-boundary-and-a-person-promotes-it.md` records the decision and `majordomus.knowledge-integrity` is the rule that dispatches the check.
