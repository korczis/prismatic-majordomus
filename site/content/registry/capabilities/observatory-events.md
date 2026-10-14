+++
title = "observatory.events"
description = "The events of the mesh journal above a cursor, as the Observatory's activity scene draws them: identity, kind, node, logical clock and time, newest first, at most a page. The answer's cursor is the next request's `after`; a cursor this journal never reached is expired, and the reader takes a fresh projection. Without a cursor, the newest page."
weight = 135
slug = "observatory-events"
[extra]
id = "observatory.events"
source = "apps/majordomus-cli/src/capability/builtin/observatory.rs"
+++
