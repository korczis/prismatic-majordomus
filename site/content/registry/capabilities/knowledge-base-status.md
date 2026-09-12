+++
title = "knowledge_base.status"
description = "The freshness half of the stopped-writer judgement (ADR 0052, applied to the knowledge deriver by ADR 0058): the newest knowledge.derived line, the newest session.closed line, how many closed episodes no derivation names, the candidate counts, and — once a derivation has run in this checkout and the switch is on — whether the newest closed episode went underived past session.freshness.stale_minutes. Episode ids are compared, never line order. Whether the close path still calls the deriver is read from the source by the shell validator, not here."
weight = 49
slug = "knowledge-base-status"
[extra]
id = "knowledge_base.status"
source = "apps/majordomus-cli/src/capability/builtin/knowledge_base.rs"
+++
