+++
title = "mesh.review.request"
description = "Ask for a review of a branch, commit or pull request, optionally of one named runtime — whose link must carry review events (the `reviews` feature), or the request is refused as `feature_unsupported`. Who may answer is the sessions' own review mandate (mesh.session.open `reviews`): the request lists them as `eligible`, and none is the finding review_no_eligible_reviewer. Replicated to every linked runtime. Writes this runtime's journal only."
weight = 139
slug = "mesh-review-request"
[extra]
id = "mesh.review.request"
source = "apps/majordomus-cli/src/capability/builtin/mesh.rs"
+++
