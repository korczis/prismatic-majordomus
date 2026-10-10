+++
title = "intent_opposition.record"
description = "Derive the opposition to one intent's plan and stamp its critique with what was reviewed: `reviewed_revision`, the plan's revision as derived at that moment and never one a caller supplies; `reviewed_at`, the commit; and `reviewed_with`, the tool and its version. An intent with no critique gets a record with no findings, which needs `reviewed_by`. The stamp is three top-level lines and every other line of the record is left as it is: findings are a reviewer's to write. A critique whose own findings do not hold — an unknown class or resolution, a duplicate, a rejection without a reason, a resolution planned into an issue that does not exist, serves another intent or is cancelled — is refused and nothing is written. The event `opposition.recorded` is appended to the ledger with the intent, the revision and the disposition derived then; the disposition is not written to the record. With `check`, the answer is what would be stamped and nothing is written."
weight = 78
slug = "intent-opposition-record"
[extra]
id = "intent_opposition.record"
source = "apps/majordomus-cli/src/capability/builtin/intent_opposition.rs"
+++
