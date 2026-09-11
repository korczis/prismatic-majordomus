+++
title = "session_domain.machine"
description = "Every state an episode can be in, whether it is terminal and what it may move to; every transition — open, resume, checkpoint, detach, close, recover — with the states it runs between and whether it moves the state at all. Derived from the types, so a diagram that disagrees with the code cannot exist. What the machine deliberately does not depend on is stated rather than left to be inferred: task state, which is ADR 0041."
weight = 80
slug = "session-domain-machine"
[extra]
id = "session_domain.machine"
source = "apps/majordomus-cli/src/capability/builtin/session_domain.rs"
+++
