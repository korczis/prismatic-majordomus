+++
title = "The site offers the release that was published, on every surface at once"
description = "The site offers the release that was published, on every surface at once"
weight = 177
[extra]
kind = "rule"
slug = "project-the-site-offers-the-release-it-published-1"
identity = "project.the-site-offers-the-release-it-published@1"
status = "active"
source = ".ai/repo/rules/project/the-site-offers-the-release-it-published.v1.md"
+++
{% raw %}

## Rationale

On 2026-10-12 v0.21.0 was tagged at 09:43Z, the release pipeline published its six archives
and, at 10:23Z, put `releases/latest.json` naming 0.21.0 on the published site, which is what
the installer resolves. The homepage beside the install command went on offering 0.20.0, and
linking its release notes, for as long as the release's record took to reach master — the
record arrives through a pull request the release opens after publishing and which needs a
full validation of its own, an hour or two. Two surfaces of one site answered "which release
is the latest" from two sources, and after every release they disagreed for hours. Nothing
noticed, because every check read one surface at a time: the installer gate read
`latest.json`, the release smoke waited for `latest.json`, `site-check` held the pages to the
records of the tree they were built from, and each of those was true.

ADR 0131 had already made the metadata leave ahead of the record and made a deploy of master
keep it. What it did not do is let the pages follow it: they rendered the record's projection
(`site/data/registry/distribution.json`, and a `release` field the site data carried), so a
publication moved one surface and left the rest.

## Required behaviour

1. **One derivation.** `scripts/site-build` derives the release the site offers as the newer of
   two readings of one fact: the newest release record of the tree it builds (its projection
   `site/data/registry/distribution.json`) and the `releases/latest.json` the published branch
   already serves, read from the git ref (`origin/gh-pages`) and never fetched over the network
   at render time. It writes the result, with which reading won and the commit of the published
   branch it read, into `build.json` as `release`; `/build.json` serves it. When the published
   release is the newer, its two documents are copied into the build, so the metadata the build
   serves is the release its pages offer.
2. **Every surface reads it.** A page that names the latest release reads `offered` (set once in
   `base.html` from `build.release`) and nothing else. The site data carries no release of its
   own: `project.json` states the tree's version, which is a different fact.
3. **The record stays the archival projection.** The release record carries what is known only
   after publishing — digests, attestations, the publication time — so it cannot land before the
   tag, and the tag does not wait for it. The changelog and the install guide's history are its
   projections and follow it; they state releases that exist and never offer one ahead of it.
4. **Refused before it is pushed.** `scripts/site-check` refuses a build whose homepage offer
   (`data-release` on the trust strip, and on the navbar and hero badges), `/build.json` and
   served `/releases/latest.json` name different releases, and a tree whose two committed
   projections of the latest release — `latest` in `distribution.json` and
   `site/static/releases/latest.json` — disagree: the pre-merge half, decidable from the tree.
   The record's reading is the highest stable record that is not withdrawn
   (`Releases::latest_stable`); no surface lists the record files to find "the newest".
5. **Measured after it is deployed.** `scripts/pages release-live` reads the live homepage's
   offer, `/build.json`, `/releases/latest.json` and the newest GitHub release. One version is a
   pass. A disagreement younger than `deploy.release_window` (`.ai/repo/ci/pages.yaml`) after
   GitHub's release is a publication in flight and passes, saying so; older, it fails. A site
   offering a version GitHub has not published fails at once. A reading it cannot take is exit
   12, never a pass.

## Failure behaviour

- `scripts/site-build` exits `10` when the published branch serves a `releases/latest.json`
  that is not a `release/v1` document naming its own tag, or a tag file that differs from it.
- `scripts/site-check` fails (section 17b, `release`) for any disagreement in item 4.
- `scripts/pages release-live` exits `10` past the window or for an offer ahead of GitHub, `12`
  when a reading is missing. In `.github/workflows/pages.yml` it is the step `the release it
  offers`, after the site serves the deployed commit; in `scripts/ci/pages-check` it is section
  5 of the `pages-live` gate, which `finish` asks (`at-finish`) and the scheduled pages run asks
  every thirty minutes, deploying when it fails.
- `scripts/site-deploy` warns when it keeps the published metadata over an older build: the
  pages of that build offer the older release until the next build.

## Verification

`test/cases/1087_the_site_offers_the_release_it_published.sh`: the derivation picks the
published release over an older record and the record over an older or absent publication,
copies the published documents, and refuses a malformed one; every template reads the one
derivation; `scripts/pages release-live` against a `file://` site and a `gh` on `PATH` passes
one release, passes a fresh disagreement inside the window, and fails when each of the three
surfaces in turn names another release past it, when the site offers what GitHub has not
published, and returns 12 when GitHub cannot be read. `test/cases/352` holds the offer to a
release that exists; `test/cases/1041` holds the metadata path of ADR 0131.
{% endraw %}
