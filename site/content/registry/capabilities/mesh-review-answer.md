+++
title = "mesh.review.answer"
description = "Answer a review request from any runtime with approved, changes_requested or commented, and a note. The answer carries the requester's route, resolved as `mesh.review.request` resolves the addressee's, so the caller can wake the requester instead of it reading mesh state in a loop. Replicated to every linked runtime. Writes this runtime's journal only."
weight = 138
slug = "mesh-review-answer"
[extra]
id = "mesh.review.answer"
source = "apps/majordomus-cli/src/capability/builtin/mesh.rs"
+++
