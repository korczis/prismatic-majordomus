+++
title = "release.obligation"
description = "The version obligation of this tree against the trunk it is integrated into (ADR 0106): the larger of what the public contract requires since the last release (ADR 0051) and the completion cadence the policy declares (`release.cadence`), which a change set owes over the trunk's own version when it carries work. Every changed path is classified by what makes it machine output — a projection the trunk's .gitattributes marks derived, a release record, the manifest and lock differing only by the version — and anything else is work; a change set that carries no work owes no cadence, so the release pipeline's follow-ups never raise the version they record. The verdict is a predicate, not a count: satisfied when the declared version reaches the minimum computed against the trunk as it is now, owed when it does not, behind when the trunk already declares more, and unverified — never a pass — when the trunk cannot be read. `release advance` satisfies it through the one writer; `finish --outcome completed` asks it; the `version-obligation` gate refuses a merge that does not hold it."
weight = 140
slug = "release-obligation"
[extra]
id = "release.obligation"
source = "apps/majordomus-cli/src/capability/builtin/release.rs"
+++
