# The install command this project advertises is run, from the published site, and proved to work

## What it means

Every document here shows one line to a stranger:

```bash
curl -fsSL https://korczis.github.io/prismatic-majordomus/install.sh | sh
```

That line is fetched and run — as it is written, pipe included — on Linux and on macOS,
against what GitHub Pages and GitHub Releases are serving at that moment, on every push to
the default branch and on the weekly schedule. The tool it installs must report the version
the metadata resolved, serve MCP with no toolchain present, and initialise a repository.
While any of that is untrue, the default branch is red.

## How it works

`scripts/ci/install-check` is the gate `installer-live` in `.ai/repo/ci/gates.yaml`, job
`install`. It states no address of its own: the installer's URL, the metadata's URL and the
command line itself are read from `site/data/registry/distribution.json`, the generated
projection of `share/distribution.yaml`.

No path class selects it, which is deliberate. Every other gate answers a question about
this tree, and a changed path is evidence about which of those answers could have moved.
This one answers a question about a deployment, and no change to a tree can move it — so it
runs in the full plan: a push to the default branch, the schedule, a dispatch, and a pull
request labelled `ci:full`.

It installs into a home directory of its own under `TMPDIR` and removes it, so it neither
reads nor writes the machine it runs on.

## How to see it

```bash
scripts/ci/install-check                    # against the published site, right now
bash test/run.sh 97_install_gate            # the gate itself, against a local fixture
```

## What it does not cover

It proves the advertised command works on the two platforms it runs on. The other four
supported targets are proved once each, at publication, by the release pipeline's smoke
phase — cross-built targets have no runner to install on.

It is a check on a deployment, so it cannot be green before the first release is published,
and it says exactly that rather than passing quietly.

## Why it exists

Because the failure it catches happened, and nothing caught it. A tagged release failed in
its build phase on one platform, so nothing was published; the cause was fixed on the
default branch the same evening and the tag was never cut again. For a day and a half the
advertised command answered every reader with "no stable release is published yet", while
every gate over the tree was green — correctly, because no gate over a tree can see the
site. A promise made in the present tense needs a check in the present tense.
