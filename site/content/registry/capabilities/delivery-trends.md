+++
title = "delivery.trends"
description = "The series of CI runs the pages publication recorded in this checkout at site/data/trends.json, oldest first: for each run its wall time, the time its jobs waited for a runner, its jobs, and the coverage it measured (lines, functions and regions as covered over total with the percentage they give); and the median run and queue time of every job over the series, the slowest first. A read of one file: nothing is fetched, derived or written, and no committed file depends on it. A figure the file does not give is absent, never zero, and a run that measured no coverage has none. `recorded: false` with the reason when the file is absent, is not JSON, or is not the trends/v1 series."
weight = 26
slug = "delivery-trends"
[extra]
id = "delivery.trends"
source = "apps/majordomus-cli/src/capability/builtin/delivery.rs"
+++
