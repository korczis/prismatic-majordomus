+++
title = "majordomus monitor steps"
description = "Wait until every step of a range of one job of a workflow run is success: exit 0 green, 10 at the first step that concluded anything else (failure, cancelled, timed_out, skipped), 11 when the timeout passes while pending, 12 when GitHub cannot be read"
weight = 174
[extra]
route = "/docs/cli/monitor/steps/"
command = "majordomus monitor steps"
source = "apps/majordomus-cli/src/cli.rs"
+++
