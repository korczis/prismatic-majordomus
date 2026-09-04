# Use cases and applications

Two registries describe what Majordomus is for, as data rather than as prose.

| | question it answers | file |
|---|---|---|
| **use case** | what do I *do* with this? | [`share/use-cases.yaml`](../share/use-cases.yaml) |
| **application** | does this fit *my* repository? | [`share/applications.yaml`](../share/applications.yaml) |

They sit alongside two catalogues that already existed and answer different questions. A
**why page** is a moment you recognise — *I have had this problem*. A **use case** is a
task you perform. An **application** is a context it suits. A **command page** is one
command's surface. Keeping them separate is deliberate: a reader deciding whether they
have the problem should not be handed an implementation detail, and a reader running a
task should not have to infer the sequence from a reference page.

## Why these are data

Everything a use case says is a reference to something else in the repository. Its steps
name commands, the rules it relies on name doctrines, the promises it makes name claims,
and the contexts it applies to name applications. Written as prose, none of that is
checkable and all of it rots quietly, because nothing executes documentation.

Written as data, all of it is checked. `mj_validate_catalogue` in `lib/doctor.sh`
resolves every reference against the thing that defines it — commands against the
dispatch table in `bin/majordomus`, doctrines against the resolved rule set, claims
against `docs/CLAIMS.yaml` — and `scripts/generate-site-data` repeats the resolution when
it builds the pages, so a dangling reference cannot reach the published site.

## The schema

```yaml
version: 1
use_cases:
  - id: adopt-an-existing-repository   # stable; the URL slug and the cross-reference key
    title: ...
    weight: 1                          # display order
    summary: one line
    situation: what is true before you reach for this
    steps:
      - command: init                  # must be a command the binary dispatches
        note: what this step does here
    outcome: what you are left holding
    commands: [init, update, doctor]   # every one must exist
    doctrines: [majordomus.projection-integrity]  # every one must be in the registry
    claims: [region-projection]        # every one must be in docs/CLAIMS.yaml
    responsibilities: [projection]     # every one must be a README row
    applications: [repository-with-authored-governance]   # must name this back
```

```yaml
applications:
  - id: repository-with-authored-governance
    title: ...
    summary: one line
    context: what this situation is like
    fits_when: [...]                   # required
    does_not_fit_when: [...]           # required — see below
    use_cases: [adopt-an-existing-repository]   # must name this back
    doctrines: [...]
    responsibilities: [...]
```

Note the constraint the tool's own YAML reader imposes: it is a minimal parser with no
block scalars, so multi-line prose is a single quoted string. The registry has to be
readable by the tool that enforces it.

## Both directions, and both lists

Two rules are worth stating because they are the ones that would otherwise be skipped.

**Cross-references are mutual.** A use case naming an application that does not name it
back is a failure, not an asymmetry somebody will notice later. Checked by `mj_cat_back`
in both directions, so the two files cannot drift into disagreeing about which applies to
which.

**An application must declare `does_not_fit_when`.** A catalogue that only lists fits is
marketing. Refused by the generator and by `doctor`.

## Extending it

Add an entry, run `scripts/generate-site-data`, and the route, the cross-links, the
filters and the checks appear. There is no template to edit and no list to update — the
section pages iterate the data, the filter options are built from it, and `site-check`
requires a page per entry. That is the whole extension mechanism.

If a reference does not resolve, the build refuses and names it:

```
generate-site-data: use case adopt-an-existing-repository names doctrine 'no_such_rule', which does not exist
```

## What is not checked

Resolution is not accuracy. A use case whose steps are in the wrong order, or whose
situation describes a problem nobody has, passes every check here — each reference exists.
What is enforced is that the catalogue cannot describe a tool other than this one.
Whether what it describes is worth doing is a judgement no validator makes, and this
document does not pretend otherwise.
