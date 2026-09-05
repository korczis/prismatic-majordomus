+++
title = "Registry"
description = "What the Rust executable compiled the repository into: every capability, every object of the layer, the kinds it reads, and the fingerprint that names this exact state."
template = "registry.html"
+++
The registry is the one model every interface of the Rust executable answers from. `majordomus mcp`, the HTTP routes, the OpenAPI document, the `capabilities` commands and this page are projections of the same entries, and this page is rendered from `site/data/registry/registry.json`, which `majordomus generate site` writes and `majordomus generate --check` compares in CI. Nothing below is typed by hand; a number that disagrees with the executable is a stale generated file, and the build refuses it.
