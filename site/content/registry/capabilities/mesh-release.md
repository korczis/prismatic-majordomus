+++
title = "mesh.release"
description = "Release a claim this runtime's current run holds, by its key. A claim written elsewhere is refused as `not_own`: only its holder releases it, and a dead holder's claim expires instead. Writes this runtime's journal only."
weight = 71
slug = "mesh-release"
[extra]
id = "mesh.release"
source = "apps/majordomus-cli/src/capability/builtin/mesh.rs"
+++
