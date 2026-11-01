+++
title = "distribution.build"
description = "What this executable is: the version of the crate it was built from, the Rust target triple, the profile, and the commit — all compiled in at build time, so an installed binary answers without a repository, a toolchain or git — and the facts of the platform hosting it (application, region, Machine, image), each read from the environment by name from a fixed allowlist and null when absent. No other environment variable is read, so a token or secret the platform injects cannot reach the answer."
weight = 47
slug = "distribution-build"
[extra]
id = "distribution.build"
source = "apps/majordomus-cli/src/capability/builtin/distribution.rs"
+++
