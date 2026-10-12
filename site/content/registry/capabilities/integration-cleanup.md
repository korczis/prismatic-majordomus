+++
title = "integration.cleanup"
description = "The cleanup plan, decided offline from the recorded observation: every open pull request cleanup would close — redundant (its work is on master already) or superseded (by a declared successor that landed) — and every one it leaves for a person — possibly redundant (weak evidence) or obsolete (a person marked it, owner decision D3) — each with its disposition and the reasons that decided it, in rank order; beside it, the branches merged pull requests left on origin as `majordomus prs cleanup` last read them, with when and how long ago. A read: it closes, deletes and asks the forge for nothing — `majordomus prs cleanup` reads the forge and `--apply` closes. `observed: false` with the reason when this checkout has recorded no observation."
weight = 76
slug = "integration-cleanup"
[extra]
id = "integration.cleanup"
source = "apps/majordomus-cli/src/capability/builtin/integration.rs"
+++
