# Release checkpoints and gates

## Critical path
A00 → A01 → A02 → A03 → A04 → A05 → A06/A07/A08 → A09 → A10/A11 → A12 → A13 → A14 → A15.
The manifest is the authority for dependencies; this line only sketches the flow.

## Recurring release gates
- **Truth:** source reference and public scope valid; generated data current; claim status backed by evidence.
- **Correctness:** `.ai` machine validators, domain/schema/product checks, homepage check, site build and site check, tests appropriate to atom.
- **UX:** interaction semantics, keyboard/mobile and no-JS baseline; static assets within declared budgets.
- **Safety:** no local/private paths, credentials, hidden `.ai` data, or fake runtime demos.
- **Delivery:** PR/branch protections pass; version bump follows policy; canonical Pages publish and external HTTP verification of merged commit.

## Decision states
`planned → implemented → locally_proved → merged → release_prepared → publication_pending → published_verified` or `blocked` with provenance. Never jump from `locally_proved` to `published_verified`.

## Example release evidence (template, never prefilled)
```yaml
atom: A06
source_sha: <actual SHA>
version_decision: <bump / no bump with rule>
local_gates:
  - command: <actual command>
    exit_code: <actual integer>
pr: <actual link>
merged_sha: <actual SHA>
pages:
  status: <pending / verified / blocked>
  public_url: <resolved canonical URL>
  observed_sha: <actual SHA, never guessed>
  smoke: <pass/fail/pending>
blockers: []
```
