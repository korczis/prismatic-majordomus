+++
title = "continuity.resume"
description = "Plans exactly as continuity.plan does and acts only on a ready or ready_with_warnings plan: writes the handover into this checkout's handovers (where handover --resolve and the session briefing find it), appends the task's decisions to the decision log once each, and records the record as the one this checkout continues, so that its next publication extends the same line. Runs nothing the record says; the start command for the task is returned as a recommendation."
weight = 16
slug = "continuity-resume"
[extra]
id = "continuity.resume"
source = "apps/majordomus-cli/src/capability/builtin/continuity.rs"
+++
