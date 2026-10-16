+++
title = "A release leaves the site better than it found it"
description = "A release leaves the site better than it found it"
weight = 73
[extra]
kind = "rule"
slug = "project-a-release-leaves-the-site-better-1"
identity = "project.a-release-leaves-the-site-better@1"
status = "active"
source = ".ai/repo/rules/project/a-release-leaves-the-site-better.v1.md"
+++
{% raw %}

## Rationale

On 2026-10-16 the published site drew its dependency graphs with node text that could not
be read in either theme, and called a roadmap that the repository's own records contradicted
a plan. Every release before that day had passed every gate. The gates hold the site to the
tree it is built from — the links resolve, the generated pages are current, the declared
colours are readable, the homepage follows its declaration — and each of them is a floor: a
release that met it on the day before met it again on the day after, and nothing asked
whether the site a visitor sees had become any better for the release, or whether a change
somewhere had quietly made it worse in a way no floor measures.

The owner's order of that day is the rule: every release leaves the site better, nicer,
friendlier and more derived, inferred, generic, generated and documented than it found it.
"Better" is not a word a gate can read, so this rule makes it two measurable halves — nothing
the repository measures got worse, and something the repository can name got better — and
holds every release to both at the one moment a release is decided: its tag.

## Required behaviour

**The baseline is the previous release.** It is the newest stable, unyanked release record
whose commit is an ancestor of the commit being released (`Releases::latest_stable` order,
read from `.ai/repo/releases/`), or the release a caller names. Both trees are read from git
at their commits, never from a working tree, so the verdict is a function of two recorded
commits and reads the same whenever it is taken again: the release record is the stored
evidence of which tree was released, and the measures are computed from that tree by the same
code every time, which is why they are not copied into a second file that could drift from it.

**(a) No measure regresses.** The measures are the ones the executable computes from a tree,
each with the direction in which it improves:

<div class="overflow-x-auto" tabindex="0">

| Measure | Better | Computed from |
|---|---|---|
| `contrast.light.failing`, `contrast.dark.failing` — text pairs below WCAG AA | lower | `crate::design::contrast` over `share/design/tokens.yaml` and the stylesheets that consume it (`project.design-tokens-declared-once`) |
| `contrast.light.weakest`, `contrast.dark.weakest` — the weakest text contrast | higher | the same measurement, the smallest ratio of a text pair, to two places |
| `site.pages-by-hand` — page files written by hand rather than generated | lower | the files under `site/content` and `site/content-src` that the tree's own `.gitattributes` does not mark `merge=derived` |

</div>


A measure worse at the release than at the baseline refuses the release, and the finding names
the measure, both values and the delta. A measure the released tree cannot produce refuses it
too: a measurement not taken is never a pass. A measure the baseline's tree cannot produce —
it predates what the measure reads — is stated as unmeasured and compared with nothing.

The site's other measures are binary and already held at zero on every commit a release can
be cut from: the dead and undecidable links, the unreachable pages, the stale generated data
and the prose counts typed by hand (`scripts/site-check`, `scripts/ci/link-check`, gate
`site-build`), the browser probes at every width (gate `site-probe`) and the interactive
controls (gate `interaction-probe`). A release is published only from a commit whose `ci`
verdict is success (`scripts/ci/release-verdict`), which is every one of those gates at zero,
so none of them can be worse than the baseline's zero; they are this rule's floor, and the
measures above are what can move above it.

**(b) The window carries an improvement of the site.** The window is every commit between the
baseline and the release, merges excluded. A commit is an improvement of the site when its
conventional type is one of the publication model's `release.kinds` and it changes, outside
every file `.gitattributes` marks `merge=derived`:

- a path of one of the gate classes `release.classes` names in `.ai/repo/ci/pages.yaml` — the
  site's own sources: its templates, styles and scripts, and the design declaration — whose
  paths are the CI model's (`.ai/repo/ci/gates.yaml`) and are named, never copied; or
- any path at all, while its scope is one of `release.scopes`: a canonical record, document or
  generator outside the site's classes whose page the site generates counts when its commit
  names the site as its subject.

A commit whose every changed file is generated output counts for nothing, whatever its
subject says. Editing a generated page by hand is a defect `project.derived-files-regenerated`
already refuses, and a copy kept by hand is the opposite of the derived site this rule asks
for. A window with no counting commit refuses the release, and the finding lists every commit
that touched the site or named it, with the reason each did not count.

## Failure behaviour

`majordomus release site` (the capability `release.site`, also `GET /api/v1/release/site` and
the MCP tool `majordomus_release_site`) answers the verdict: every measure at both releases
with its delta and whether it held, improved, regressed or could not be measured; every commit
of the window that touched the site or named it, with whether it counts and why; and the
findings. The command exits `0` when the release passes, `10` when it does not — each finding
`site-measure-regressed`, `site-measure-unreadable` or `site-not-improved`, naming the measure
and its delta or the commits — and `12` when the verdict cannot be taken: no recorded release
is an ancestor of the commit, a ref the clone does not have, or a publication model whose
`release` section is missing or names a class the CI model does not declare.

In `.github/workflows/release.yml` the step `the release leaves the site better than the last
one found it`, in the `plan` job, runs it on the tagged commit after the `ci` verdict and
before any target is built: a refused verdict publishes nothing. The verdict is written to
the job summary, and the JSON is kept as the run's `release-site` artifact.

## Verification

```sh
majordomus release site                          # this commit against the last release
majordomus release site --since v0.27.0 --head v0.28.0
bash test/run.sh 3170_a_release_leaves_the_site_better
```

`test/cases/3170_a_release_leaves_the_site_better.sh` builds a repository with two recorded
releases and proves each half against the real executable: a measure that regressed refuses
the release and names it with its delta; a window with no site change refuses; a window whose
only site change is a hand edit of generated output refuses, naming it; a window with an
improvement of a site source passes, and so does a canonical record whose commit names the
site; a repository with no recorded release is exit 12. `apps/majordomus-cli/src/release/site.rs`
carries the unit half: the directions, the comparison, the classification of every kind of
commit, the declaration's refusals and the measures of the shipped design.
{% endraw %}
