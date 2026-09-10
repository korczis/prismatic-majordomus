---
schema: change/v1
id: release-identity
kind: change
title: a release is one identity, derived from the public contract and projected everywhere
type: added
impact: additive
released_in: "0.3.2"
scopes: [cli, api, mcp, cockpit, docs, site, installer]
contract:
  - "capability:release.version"
  - "capability:release.status"
  - "capability:release.explain"
  - "capability:release.diff"
  - "capability:release.changelog"
  - "capability:release.manifest"
  - "capability:release.plan"
  - "capability:release.check"
  - "document-kind:majordomus.change/v1"
adrs: [adr-0028]
---

## Summary

Version, changelog, release, deployment and the version a person sees were five separate
things that agreed by hand. The crate's version and the shell tool's were two literals kept
in step by a script somebody had to remember to run; the website's navbar took its number
from one of them and the API reference from the other; there was no changelog at all, so a
release's notes were whatever GitHub generated from commit subjects; and nothing anywhere
decided whether a version was *allowed* — a breaking change could ship as a patch and
nothing would say so.

They are now one model. The crate's version is the only place a version is written. The
public contract — every capability with its input and output schemas and its projections,
every runnable command with its arguments, every document schema a repository's own files
are validated against, every platform a release publishes for — is normalised into
`docs/generated/contract.json` at every commit, and a release's compatibility is the diff
between that document and the same document at the last published release. The minimum
version follows from the compatibility by a stated policy, and a release below it is
refused rather than reported.

The changelog is a projection of records under `.ai/repo/changes/`, and so are the release
notes, the release manifests, the Cockpit's release page and the API's answer. The version
the shell tool prints is generated into `share/version.txt` from the crate's version. The
Cockpit's version display — which existed, and showed one number — now shows which of the
four versions disagree, and says so where a person is already looking.

Every one of these is read through one engine and exposed as one set of capabilities, so
the command line, the HTTP API, MCP and the Cockpit are four readings of one answer rather
than four implementations of one idea.
