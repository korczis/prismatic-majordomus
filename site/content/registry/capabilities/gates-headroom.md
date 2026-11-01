+++
title = "gates.headroom"
description = "Every structure job the CI model assigns gates to (`structure`, `structure-<x>`), with its `timeout-minutes` from .github/workflows/validate.yml, the 90th percentile (nearest rank) of its wall time over the committed window .ai/repo/ci/structure-job-durations.tsv, and that p90 as a share of the bound. A job past half its bound is `exceeds` and names its three most expensive gates from .ai/repo/ci/structure-durations.tsv, so the remedy is a split or a cheaper gate and never a larger bound (I4381). A missing window, a job with no measured run or no bound is `unmeasured`, never a pass; `ok` is true only when every job fits and every input was read."
weight = 75
slug = "gates-headroom"
[extra]
id = "gates.headroom"
source = "apps/majordomus-cli/src/capability/builtin/gates.rs"
+++
