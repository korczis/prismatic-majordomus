+++
title = "episodes.detach"
description = "Close an execution episode deliberately: the work is done, not merely interrupted, and the repository's record says so. Without 'external_id' it is whichever episode the calling connection holds. A client that simply goes away does not need this — its episode detaches and the reaper closes it as interrupted — and the difference between those two records is the one thing about an ended episode that changes what somebody does next."
weight = 27
slug = "episodes-detach"
[extra]
id = "episodes.detach"
source = "apps/majordomus-cli/src/capability/builtin/episodes.rs"
+++
