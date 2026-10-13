+++
title = "HTTP API: postmortems"
description = "What turned the trunk's CI red and why: one record per cause under .ai/repo/post-mortems/, naming the runs it answers, the cause as a class and a sentence, the components it lives in, the signals a failing log shows, what fixed it and what now prevents it. The records are listed, judged against the rules their schema cannot say, compared with the trunk's failed runs to find the run nobody has answered, and searched for the signals of a new failure. Nothing here changes the repository; the two operations that ask the forge are the command line's alone."
weight = 1040
path = "docs/api/postmortems"
template = "api-tag.html"
[extra]
api_tag = "postmortems"
api_slug = "postmortems"
source = "docs/generated/openapi.json"
+++
