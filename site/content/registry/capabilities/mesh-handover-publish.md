+++
title = "mesh.handover.publish"
description = "Publish a handover record of this checkout — the newest, or the one named under .ai/local/state/handovers/ — to every linked runtime: its task, branch, head and time from its front matter, the issue and milestone given, and its Markdown body, bounded and identified by the body's digest. No path of this machine travels, and no file outside the handovers directory is ever read."
weight = 63
slug = "mesh-handover-publish"
[extra]
id = "mesh.handover.publish"
source = "apps/majordomus-cli/src/capability/builtin/mesh.rs"
+++
