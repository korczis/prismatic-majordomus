+++
title = "release.status"
description = "The whole release state: the four versions, the release the contract was measured against, the compatibility of the change, the minimum version it requires, the version this tree would publish, how far along the release is, and every diagnostic. This is the model the Cockpit's version display, the API and the release commands all read; none of them computes a release fact of its own."
weight = 42
slug = "release-status"
[extra]
id = "release.status"
source = "apps/majordomus-cli/src/capability/builtin/release.rs"
+++
