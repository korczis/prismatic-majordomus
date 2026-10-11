+++
title = "mesh.review.withdraw"
description = "End a review request this runtime asked for — done, void or superseded — with an optional result note the request then carries. Any session of the asking runtime may (a reconnected asker is another session of it); no other runtime may, and it is never counted as an answer. An unanswered request also reads expired while its runtime has stopped beating. Replicated to every linked runtime. Writes this runtime's journal only."
weight = 112
slug = "mesh-review-withdraw"
[extra]
id = "mesh.review.withdraw"
source = "apps/majordomus-cli/src/capability/builtin/mesh.rs"
+++
