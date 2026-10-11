+++
title = "mesh.review.withdraw"
description = "End a review request the calling session asked for — done, void or superseded — with an optional result note the request then carries. Only the asking session may; it is never counted as an answer. An unanswered request also ends on its own when its session closes or its runtime stops beating. Replicated to every linked runtime. Writes this runtime's journal only."
weight = 111
slug = "mesh-review-withdraw"
[extra]
id = "mesh.review.withdraw"
source = "apps/majordomus-cli/src/capability/builtin/mesh.rs"
+++
