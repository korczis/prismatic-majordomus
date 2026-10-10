# Verifying a release after its tag

What is checked between pushing a release tag and saying the release is out, in order, and
the traps each step has met. The pipeline and its recovery table are *Releasing* and *When a
release fails partway* in `docs/DISTRIBUTION.md`; the changelog and the version are
`docs/RELEASE.md`. This is the read-back that proves the published release is the one users
get, on the machines that run it. A release is out when every step below has an answer, not
when the tag is pushed.

## 1. The run published, and the record is proposed

The `release` run of the tag ends in `smoke`. `plan` refuses a tag whose commit has no `ci`
success (`scripts/ci/release-verdict`): a tag pushed before master's run finished is
recovered with `gh run rerun <run>` once `ci` is green, never by moving the tag.

`publish` ends by proposing `release/record-<tag>` as a pull request. When the run died after
publishing but before proposing, the release exists and is attested and nothing names it:
v0.20.0's run failed in its staging step, because `git add -A -- . ':(exclude)dist'` exits 1
once `/dist/` is ignored (since #855); pull request #907 changes that step. A rerun cannot
help, since the workflow at the tag is the broken one. Propose the record by hand, as #901 did:

```text
# in the canonical worktree of release/record-<tag>, cut from the current master
gh release download <tag> --dir dist
(cd dist && shasum -a 256 -c SHA256SUMS)
MAJORDOMUS_DIST_BIN=<an executable of this tree> scripts/release-record --tag <tag> \
  --dir dist --commit <the tagged commit> --notes-url <the release's URL>
git add .ai/repo/releases/<tag>.yaml
# derive, commit the record and its projections in one commit, push, open the pull request
```

A pull request a person opens gets its own run. The one the pipeline opens does not: its run
is dispatched, and on 2026-10-10 a dispatched `ci` success did not satisfy the branch
protection (#866). Close and reopen the pipeline's record pull request as soon as it opens.

From the tag until the record is on master, `scripts/ci/release-check` fails `structure` on
every other head: say so on the board and the mesh, and again when the record has landed.

## 2. Every archive is the one the workflow built

```text
gh release download <tag> --dir <empty directory>
cd <that directory> && shasum -a 256 -c SHA256SUMS
gh attestation verify <archive> --repo korczis/prismatic-majordomus    # for every archive
```

v0.20.0 is the first release published after the workflow began attesting its archives; an
older release answers that no attestation matches the digest, which is its age and not a
defect. One archive without an attestation in a release that has them is a defect.

## 3. The site serves it

After the record merges, Pages deploys it:

```text
scripts/pages verify --commit <the record's merge commit>
curl -fsSL https://majordomus.dev/releases/latest.json        # names <tag>
scripts/ci/install-check                                        # the advertised line, end to end
```

Until the record lands, `latest.json` names the previous release and the installer installs
that. A `pages` job of master that ran while the deploy was in flight can be red on a correct
commit; rerun it with `gh run rerun <run> --failed` after `scripts/pages verify` passes.

## 4. Every machine runs it

On each of the owner's machines (the trust row of *This repository's mesh* in
`docs/MESH.md`), install from the published site, ask the version, and drive a scratch
repository:

```text
curl -fsSL https://majordomus.dev/install.sh | sh
majordomus --version                      # the tag
cd <an empty scratch repository> && majordomus init && majordomus update && majordomus context
```

jetson (aarch64, glibc 2.27) cannot load the gnu build, and the installer picks it until pull
request #853 lands: the hub then exits with the shell tool's usage and its service restarts
without a log. Download the musl archive of the same tag, verify it against `SHA256SUMS`, run
its `libexec/majordomus-cli --version`, put its tree in place of the gnu one under the
installer's versions directory, and restart the service.

## 5. Every hub serves it

Installed is not serving. A hub keeps the executable it started with until it restarts, and
says so:

```text
curl -s -H 'Accept: application/json' http://<host>:8791/
```

`version` must be the tag and `stale` null. A `stale` reason ("the executable this process
was started from has been replaced since it started") is a hub still running the previous
release: restart it. The first request after a hub's checkout moves can take more than 20
seconds; a slow answer is not a dead hub.

## What is not automated

Steps 2, 4 and 5 are done by hand, machine by machine; `smoke` covers the hosted runners only.
A fleet rollout that installs, swaps the musl build where the C library needs it and reads
every hub back is owed as a typed capability, not as a script beside the repository.
