+++
title = "Launch kit"
description = "what to paste when somebody asks what Majordomus is: the link to send, the install command, one-sentence and longer descriptions, three proof points with the page behind each, and what not to claim"
weight = 21
[extra]
source = "docs/LAUNCH.md"
+++

{% raw %}

What to paste when somebody asks what Majordomus is. Every sentence here is a claim the
repository already holds, and every link resolves to a page generated from it; nothing here is
a figure, because figures are computed and go stale in prose. When you need a number, take it
from the page that computes it.

## The link to send

**<https://majordomus.dev/challenge/>** — a coding agent strays outside its scope, says it is
done, hands over and comes back; Majordomus refuses it and accepts the work only when the
repository's own test passes. Every line on that page is a recorded run of the real tool, and
the build refuses to publish it if the run stops holding.

- Install: `curl -fsSL https://majordomus.dev/install.sh | sh`
- Source: <https://github.com/korczis/prismatic-majordomus>
- Replay the challenge locally: `majordomus usecase run catch-an-agent-that-says-it-is-done`

## One sentence

AI coding agents do the work; Majordomus is a local control layer that keeps the rules, the
task's scope, the handover and the definition of done in the repository, and refuses to call
work finished until the repository's own checks pass.

## About fifty words

Majordomus supervises AI coding agents from inside the repository. One policy generates the
instruction file each agent reads. A task claims the paths it may touch; a file outside them
fails the check. Handovers are structured records, not transcripts. And `finish` evaluates a
contract line by line, writing nothing while any line fails.

## About a hundred and fifty words

Coding agents are good at doing the work and bad at knowing when it is finished. They edit
files nobody asked for, report success over a failing test, and forget the task when the
session ends. A better prompt does not fix that, because the prompt is the part being ignored.

Majordomus is a supervisory control layer that lives in the repository. One canonical policy
generates the instruction file each agent reads, so the rules stop diverging between tools. A
task claims the paths it may touch; `check` and `finish` fail on anything outside them, and
`start` reports when another worker already claims the same paths. A handover is a structured
record the next session gets back from `context`. `finish` evaluates a contract line by line
and records an outcome only when every line passes, including the repository's own
verification. It runs locally, in portable shell, and never calls a model.

## Three proof points

1. **It refuses, and says why.** The challenge's recorded run shows a scope refusal and a
   finish refused over a failing test, each with the command that reproduces it:
   <https://majordomus.dev/challenge/>.
2. **Every public claim names its test.** Guarantees, advisory behaviour, planned work and
   what is refused are separated, each with the implementation and the test behind it:
   <https://majordomus.dev/guarantees/>.
3. **The site is held to the repository.** Pages, counts and transcripts are generated from
   the tree and refused when they drift; the tool supervises its own development:
   <https://majordomus.dev/docs/dogfooding/>.

## What not to say

- Not a number of users, stars, customers or benchmark wins; the repository measures none.
- Not that it runs or coordinates models; it never invokes one.
- Not that overlap between workers is blocked; it is reported. Scope, the finish contract and
  a failing verification are what refuse.
- Not that a planned capability exists; <https://majordomus.dev/roadmap/> says what is not built.

## Assets

- Social card: `share/design/brand/social-card.png`, served as `/images/social-card.png`
- Logo: `share/design/brand/logo.svg`
- The replay itself: `site/data/generated/challenge.json`, regenerated with the site
{% endraw %}
