# 14-day evidence-driven dogfooding protocol

Precondition: instrumentation quality proven before comparison. Days 1-2: source discovery, baseline instruments and stable pricing basis; days 3-6: observation, reliability and source coverage; day 7: quality/coverage gate; days 8-11: randomized or blocked-assignment treatment for one intervention (e.g. narrower context retrieval); days 12-13: compare accepted outcomes and cost, bootstrap uncertainty; day 14: publish evidence and revert/adopt decision.

**Unit:** eligible IDD execution/outcome; predefine inclusion/exclusion. Stratify by complexity, model, provider, task family and repo. Avoid before-after comparisons with systematically different work mixes. Primary endpoint: normalized cost per independently accepted outcome. Guardrails: verification pass rate, defects/regressions, elapsed cycle time and accepted scope. Secondary: cache ratio, overfetch, compaction frequency, retry cost, abandonment.

Distinguish direct provider bill, subscription allocation, local hardware cost, API-equivalent model estimates. No provider invoice => no claim of billed savings. Compute confidence intervals from task-level bootstrap with cluster grouping for repeated sessions. If sample inadequate, mark exploratory. Never tune intervention using validation data without recording it.

Produce human-readable narrative, graph, dataset manifest, reusable query, data coverage report, methodology, confounds, negative results and proof links. Zero invented sample points.
