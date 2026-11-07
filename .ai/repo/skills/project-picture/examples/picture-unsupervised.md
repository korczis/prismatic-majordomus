# Picture of a repository without a Majordomus layer

Intent: "What is the state of acme-tools?" The repository has no `.ai/manifest.yaml`.

The worker skips stages 3–5 and records them as one coverage row: "T1: not supervised". It
does not run `majordomus init` and does not suggest running it unasked. The picture rests on
the tree (stage 2) and on whatever forges, trackers and conversations answer (stages 6–9).
Precedence still holds: the tree outranks everything else.

The decisions, open questions and rationale normally read from the `.ai/` layer come here
from ADR-like documents in the tree, issue threads and conversations. Each one is tagged
with that weaker source. Stage 11 proposes no Majordomus records. A missing layer is not
something to fix inside this skill.
