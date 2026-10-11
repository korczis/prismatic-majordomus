+++
title = "intents.binding"
description = "What a task asks before it starts (ADR 0111): given the issue the work executes, the intent it serves, the paths it will touch, or an exemption class with its reason, one standing — `bound` when the work serves a criterion of a live intent through links the preflight accepts, `maintenance` when its issues sit under milestones no live intent names, `exempt` when the class is one the policy declares under `intent.exemptions` and a reason was given, `refused` otherwise, each refusal with a cause: the preflight's own seven, and nothing_named, unknown_intent, intent_retired, intent_has_no_open_work, issue_outside_intent, ambiguous_intent (paths alone reached more than one intent), unknown_exemption, exemption_without_reason, exemption_names_work. The answer carries what was named, the issues and intents resolved with what each intent asks of the worker, a note when the paths lie outside the named issue's scope, and two pins a later reader compares: `plan_revision`, which moves when the intent, a link or the critique is edited, and `evidence_standing`, which moves when a served criterion's evidence changes state. Nothing is stored."
weight = 83
slug = "intents-binding"
[extra]
id = "intents.binding"
source = "apps/majordomus-cli/src/capability/builtin/intents.rs"
+++
