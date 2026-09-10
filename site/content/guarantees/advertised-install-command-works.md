+++
title = "The install command this project advertises is fetched and run from the published site every night and on request, and the run that asked for it is red while it does not install a working tool"
description = "Every document here shows one line to a stranger:"
weight = 19
[extra]
claim_id = "advertised-install-command-works"
status = "guaranteed"
source = "docs/claims/advertised-install-command-works.md"
+++
{% raw %}

## What it means

Every document here shows one line to a stranger:

```bash
curl -fsSL https://majordomus.dev/install.sh | sh
```

That line is fetched and run — as it is written, pipe included — on Linux and on macOS,
against what GitHub Pages and GitHub Releases are serving at that moment. It runs on the
nightly validation of the default branch, on a dispatch, and on a pull request labelled
`ci:full`; a routine push does not ask for it. The tool it installs must report the version
the metadata resolved, serve MCP with no toolchain present, and initialise a repository.
While any of that is untrue, the `ci` verdict of the run that asked for it is red: the
nightly run of the default branch, or the labelled pull request. The teeth did not go away,
they moved to the cadence of a night — which is the trade ADR 0037 names, and the reason
that run is one somebody has to read.

## How it works

`scripts/ci/install-check` is the gate `installer-live` in `.ai/repo/ci/gates.yaml`, job
`install`. It states no address of its own: the installer's URL, the metadata's URL and the
command line itself are read from `site/data/registry/distribution.json`, the generated
projection of `share/distribution.yaml`.

No path class selects it, which is deliberate. Every other gate answers a question about
this tree, and a changed path is evidence about which of those answers could have moved.
This one answers a question about a deployment, and no change to a tree can move it — so no
change to a tree plans it.

It is also `on-demand: true`, which is about a runner rather than a subject: its job
`install` is a matrix with a macOS leg, and `needs.install.result` is one value over both
legs, so the whole job is as scarce as its scarcest leg. `scripts/ci-plan` leaves an
on-demand gate out of every plan that did not ask for it — the **full** plan included — and
records per gate that it was not planned and why; `--on-demand` asks, and the nightly
schedule, a `workflow_dispatch` and a `ci:full` label pass it. ADR 0037 is the decision: on
2026-09-10 the macOS jobs queued for between three and seven hours behind a shared pool, `ci`
needed them, and so no verdict arrived at all. A gate that never completes protects nothing;
this one now completes every night.

It installs into a home directory of its own under `TMPDIR` and removes it, so it neither
reads nor writes the machine it runs on.

## How to see it

```bash
scripts/ci/install-check                    # against the published site, right now
bash test/run.sh 97_install_gate            # the gate itself, against a local fixture
scripts/ci-plan --full x | grep installer-live              # not planned, and why
scripts/ci-plan --full x --on-demand | grep installer-live  # planned, when asked
```

## What it does not cover

It proves the advertised command works on the two platforms it runs on. The other four
supported targets are proved once each, at publication, by the release pipeline's smoke
phase — cross-built targets have no runner to install on.

It is a check on a deployment, so it cannot be green before the first release is published,
and it says exactly that rather than passing quietly.

Since ADR 0037 it does not run per push, so a deployment that breaks in the morning is
caught by that night's run rather than by the next commit. Nothing on the per-push path
replaces it for the installer; what does not move is the rest of the deployment question —
`pages-live` runs on Linux, is not on demand, and still reads the published site on every
push.

## Why it exists

Because the failure it catches happened, and nothing caught it. A tagged release failed in
its build phase on one platform, so nothing was published; the cause was fixed on the
default branch the same evening and the tag was never cut again. For a day and a half the
advertised command answered every reader with "no stable release is published yet", while
every gate over the tree was green — correctly, because no gate over a tree can see the
site. A promise made in the present tense needs a check in the present tense.

That check now runs nightly rather than on every push, because per push it was not
completing at all (ADR 0037). It shortens the window from a day and a half to a night — for
as long as somebody reads the nightly run.
{% endraw %}
