+++
title = "A tag with a pre-release part is published as a prerelease, recorded on the prerelease channel, and never resolved as the stable release, so an unpinned installer and a node that follows stable keep the highest stable version"
description = "Publishing v1.0.0-alpha.1 does not change what curl -fsSL https://majordomus.dev/install.sh | sh installs. The alpha is a GitHub prerelease, its record says channel: prerelease, and releases/latest.json keeps naming the highest stable release. A person who wants the alpha names it with --version 1.0.0-alpha.1."
weight = 139
[extra]
claim_id = "release-prerelease-channel"
status = "guaranteed"
source = "docs/claims/release-prerelease-channel.md"
+++
{% raw %}

## What it means

Publishing `v1.0.0-alpha.1` does not change what `curl -fsSL https://majordomus.dev/install.sh | sh` installs. The alpha is a GitHub prerelease, its record says `channel: prerelease`, and `releases/latest.json` keeps naming the highest stable release. A person who wants the alpha names it with `--version 1.0.0-alpha.1`.

## How it works

Three places decide the label and the claim holds only while they agree. `scripts/release-record` derives the channel from the tag (a `-` in the version means prerelease) unless `--channel` is given, and refuses `--channel stable` on such a tag with exit 2. The publish step of `release.yml` passes `--prerelease --latest=false` for the same tags. In the crate, `Release::is_stable_candidate` is false for any version with a pre-release part, whatever its record says, so a mislabelled record can never become the stable pointer; `Release::findings` reports the mislabel. Precedence follows semantic versioning: identifiers compare one by one, numeric ones as numbers (`alpha.2` before `alpha.10`), a pre-release below its own final, build metadata ignored.

## How to see it

```bash
bash test/run.sh 3179_a_prerelease_is_never_published_as_stable
scripts/release-record --tag v1.0.0-alpha.1 --dir dist --channel stable    # exit 2: cannot be recorded on the stable channel
cargo test --manifest-path apps/majordomus-cli/Cargo.toml --lib distribution::
```

## What it does not cover

It does not publish anything and does not exercise GitHub: the workflow is checked for the flags it passes, not by a run. The version gates that parse the declared version (`Version::parse`, the compatibility gate, `release bump`) are a separate defect and still reject a pre-release part. `majordomus self update` is a design (ADR 0161) and is not implemented, so its stable-only rule is not claimed here.

## Why it exists

The audit before the first alpha found that the pipeline recorded every tag as stable and created every release without `--prerelease`, and that `latest_stable` ranks by version. `v1.0.0-alpha.1` would have outranked `v0.27.0` in `releases/latest.json` and become what every unpinned installer installs. The ordering was also string-wise, so `alpha.10` sorted below `alpha.2`.
{% endraw %}
