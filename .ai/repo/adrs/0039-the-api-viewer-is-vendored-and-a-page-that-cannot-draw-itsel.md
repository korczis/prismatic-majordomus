---
schema: adr/v1
id: adr-0039
kind: adr
title: The API viewer is vendored, and a page that cannot draw itself says so
status: proposed
date: 2026-09-10
tags:
  - architecture
  - http
  - distribution
  - security
related:
  - file:.ai/repo/adrs/0031-the-design-is-one-declaration-and-every-surface-is-a-project.md
  - file:.ai/repo/adrs/0019-distribution-is-one-model-and-every-surface-that-ships-the-t.md
  - file:apps/majordomus-cli/src/http/swagger.rs
  - file:apps/majordomus-cli/src/cockpit/assets.rs
  - file:scripts/swagger-assets
  - file:scripts/release-package
  - file:.ai/repo/rules/project/no-network-no-eval.v1.md
  - file:docs/WEB.md
provenance:
  origin: extracted
  derived_from:
    - file:.ai/repo/adrs/0031-the-design-is-one-declaration-and-every-surface-is-a-project.md
    - file:apps/majordomus-cli/src/http/swagger.rs
    - file:scripts/swagger-assets
    - file:.ai/repo/rules/project/no-network-no-eval.v1.md
---

# 39. The API viewer is vendored, and a page that cannot draw itself says so

## Context

ADR 31 named this and left it open, in as many words: *the Swagger UI assets are still
fetched from the unpkg CDN, which is the one part of the HTTP projection that is not
available offline; vendoring them is a separate decision about distribution size, not about
design.* This is that decision.

Everything else the loopback server offers works with no network. The OpenAPI document is
generated from the registry in-process and served from memory; the Cockpit's stylesheet and
scripts are read off the disk of the distribution that is running. Only the viewer for the
document reached out — two `https://unpkg.com/swagger-ui-dist@5.17.14/...` URLs in the
shell.

Measured, on 2026-09-10, against this repository's shared server with every non-loopback
host blackholed:

```text
GET /swagger                 200
document.title               "Majordomus API"
document.body.innerText      ""            <- nothing. A white page.
#swagger-ui                  0 children
console                      SwaggerUIBundle is not defined
```

Two hundred, and a blank frame. Nothing on the page named the CDN, the failure, the
document that was still perfectly readable at `/openapi.json`, or a way forward. A reader
on a plane, behind a proxy, or in a container with no egress had no way to tell this from a
broken build of the tool itself.

Two things are wrong there and they are separable. One is the dependency on a third-party
origin at read time. The other is that the page had no answer for its own failure — and
that half is not fixed by removing the CDN, because a file can also fail to arrive from
this origin.

There is also a rule, `project.no-network-no-eval`: *bin/, lib/, share/ and test/ contain
no network client, no telemetry, no eval, no curl piped to a shell.* Its scan
(`test/cases/08_no_forbidden_constructs.sh`) reads `bin/majordomus`, `lib/*.sh` and
`share/providers/*` — shell, looking for shell constructs — and the CDN was in a Rust string
in `apps/majordomus-cli/src/`, so the letter of the rule was never violated and no gate ever
fired. The commitment behind it — SECURITY.md's, that this tool does not reach the
network — was. A page this repository writes, telling a reader's browser to fetch 1.5 MB of
JavaScript from a CDN, is a network client with a network client on the other end of the
wire; the scan simply could not see it. That gap is worth naming even though this decision
closes it by construction.

## Decision

**The viewer is part of the distribution.** `swagger-ui-dist` at the version
`SWAGGER_UI_VERSION` pins is vendored into `share/swagger/vendor/` — the stylesheet, the
standalone bundle, and the package's own `LICENSE` and `NOTICE` — and **committed**.
`scripts/swagger-assets` copies them from the pinned npm package, and `--check` refuses a
tree where a committed byte is not that package's, or where `package.json` and the crate's
constant name different versions. The version has one owner: the constant.

**The files are served by the crate's own asset machinery.** `share/swagger/` is read the
way `share/cockpit/` is: once, from disk, with the SHA-256 of the bytes as the cache key and
in the URL, `Cache-Control: immutable` when the URL carries the right digest, the same
traversal refusal, the same media-type allow-list. `cockpit::assets::Assets` was already all
of that with two constants baked in; it now takes the directory and the prefix as arguments
and both surfaces use it. A second copy would have been a second answer to one question.

**`/swagger/assets/` is a declared exception, and the only one.** The router refuses every
path under a single-page surface, on purpose. The viewer's own files are named in that
refusal rather than exempted from it: `/swagger/assets/...` is served, `/swagger/anything`
is still the 404 it was.

**The page states the case it is in, in markup.** `#swagger-unavailable` is rendered by the
server, before any script; the widget's own arrival is what removes it. A script that has to
run in order to report that no script ran is not a report. The server knows which situation
it is in — it can see whether the files are in its `share/` — so the notice says either *the
browser did not run what we served* or *this distribution has no viewer, here is the
command*, and never guesses.

**The page carries a content-security policy.** `default-src 'none'`, everything from
`'self'`, the one inline script allowed by the digest of its own bytes, no `unsafe-eval`, no
remote origin — the Cockpit's policy, for the same reasons, now that the page has no remote
origin to allow.

## Alternatives rejected

*Build it, do not commit it — the Cockpit's own precedent for a large library.*
`scripts/cockpit-assets` copies cytoscape, three and p5 out of pinned npm packages and
leaves them uncommitted, because they are two megabytes nothing on the critical path needs.
That precedent does not transfer, for a reason that is easy to miss:
**`scripts/release-package` packs `share/` as it finds it, and the release workflow runs no
asset build.** Those three libraries are therefore in no published archive at all —
correctly, since every page that uses them renders without them. Here the file *is* the
page. Vendoring it that way would have traded a CDN for an empty `share/swagger/` in every
installed copy: the same blank page, on more machines, and now with no CDN to save it. The
other Cockpit precedent is the one that applies — `vendor/alpine.csp.min.js` is committed
because the surface depends on it — and it is a criterion about dependence, not about
kilobytes.

*Build it in the release workflow instead.* Six release runners would each need node and
`npm ci` before packaging, to save 1.6 MB in a repository whose `.git` is 1.2 GB. It puts a
network fetch on the release path, which is the exact fragility this decision removes from
the read path, and gives a release a new way to fail.

*Keep the CDN and fall back to the local copy.* Two sources for one file, a page that
behaves differently depending on what a network did, and a third-party origin still in the
policy. The fallback direction is the wrong one anyway: the local copy is the one that is
always right.

*Keep the CDN and only add the notice.* Cheaper, and it fixes the silence but not the cause:
the API console of an offline-first tool would still be the one page that needs the
internet.

*Precompress the vendored bytes and serve them with `Content-Encoding: gzip`.* 417 kB in the
tree instead of 1.6 MB, at the cost of a binary blob nobody can read or diff, an asset
server that must learn about encodings, and a file that cannot be checked against the npm
package without decompressing it first. Not worth it at this size.

*Restyle Swagger UI's component tree so that a smaller build would do.* ADR 31 declined this
deliberately and it is declined again here.

## Consequences

**The size, exactly.** The vendored set is 1,616,237 bytes: `swagger-ui-bundle.js`
1,452,753, `swagger-ui.css` 152,071, `LICENSE` 11,358, `NOTICE` 55. In the repository that
is +0.13% of a 1.2 GB `.git`. In a release archive, which is `tar.gz`, it is **431,623 bytes**
— measured, by packing this tree twice with `scripts/release-package`, once with
`share/swagger/` and once without. Against the last published archive,
`majordomus-v0.3.1-aarch64-apple-darwin.tar.gz` at 4,784,267 bytes, that is +9.0%. What a reader gets for those
bytes: an API console that works on a plane, behind a proxy, and in a container with no
egress, and a tool that no longer sends a browser to a CDN.

**The HTTP projection now contacts nothing.** Not a CDN, not a font host, nothing. That is
now true of every page this executable serves, and it is checkable by grep rather than by
reading: no `https://` appears in the shell at all, and the tests assert it.

**Upgrading Swagger UI is one edit and one command.** `SWAGGER_UI_VERSION`, the same string
in `package.json`, then `npm install && scripts/swagger-assets`. The gate fails the tree if
those ever disagree — including the half-upgraded pin that used to be possible, where the
stylesheet came from one version and the bundle from another.

**A distribution can still be built without the viewer**, and that is now a stated,
survivable state rather than a blank page: `share/swagger/` absent means `/swagger` renders
the notice, `Swagger::complete()` answers false, and the process logs a warning naming the
command that fixes it.

**What is still undone.** The vendored bytes are checked against the npm package by
`scripts/swagger-assets --check`, which needs `node_modules`; there is no check that the npm
package is what SmartBear published, because this repository has no signature-verification
apparatus and inventing one for one file would be worse than naming the gap.
`cockpit::assets` now serves two surfaces while living under `cockpit::`; the module is the
right one and its name is one rename behind, which is a mechanical follow-up and not part of
this decision.
