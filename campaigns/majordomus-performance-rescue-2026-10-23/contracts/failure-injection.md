# Adversarial Scenario Catalog

Run via existing Rust test fixture, `scripts/ui`, browser test harness and bounded test dependencies. For each drill record current SHA, input, expected state, observed latency, freshness label, HTTP status, retry/timeout bound, resource use and recovery proof. Avoid real destructive GitHub/GigaLixir actions.

- **Stalled capability**: injected 90-second handler, page responds promptly with pending; eventual completion updates.
- **All source workers busy**: 4 slow background reading jobs, simultaneous urgent human `/ui`; prove no priority inversion and bounded response.
- **Cold restart**: empty in-memory readings, no warm-up, frontend cold cache; test first useful paint.
- **Git churn**: rapid branch/index/plan changes during reads, correct monotonic version and no source generation mismatch.
- **Old lease**: server binary outdated, missing executable, concurrent starter, crash while claim exists.
- **Large plan**: synthetic 1/10/100x objects and long Git merge history; evaluate scaling after derived memo.
- **Bad network**: GitHub rate limited/5xx, expired DNS, refused peer, mobile packet delay; no local UI or cd long block.
- **WS churn**: 50 concurrent reconnecting clients, duplicate/stale event IDs, offline longer than history; resync correctness.
- **Overload**: handler cap approached/exceeded, slow request bodies and readers; respond timely/503 as documented, no thread/fd leak.
- **Frontend**: no JS, reduced motion, mobile narrow viewport, keyboard, missing vendored chart lib; complete truthful SSR remains accessible.
- **Security**: cross-checkout cache key, browser mutation Origin refusal, CSP/asset hash and signed mesh actions preserved.

Do not run all drills directly against production. Establish local isolated fixtures and stage only safe read-only tests on deployed endpoints.
