# Site acceptance (fail-closed unless explicitly historical)

1. All relevant public routes inventoried with authored/derived/historical classification and owning source.
2. Roadmap reflects real canonical milestones/issues, dependency and claim evidence; every version link resolves.
3. Public release/version badges agree with deployed release metadata, not guessed HEAD.
4. Every generated page has verifiable source/generator identity in manifest; mixed/unknown state refuses publication.
5. No manual copy of dynamic roadmap, capabilities, release inventory or progress bars.
6. Responsive and keyboard usable: graph interactive where supported, accessible fallback always works, mobile navigation and contrast tested.
7. A changed source invalidates the correct downstream outputs; unrelated inputs do not trigger whole-site rebuild without evidence.
8. Existing CI model includes measurable gates for provenance, drift, route/link integrity and site smoke.
9. Fresh clone and adopter tests exercise generation and failure paths; cached and uncached outputs are identical.
10. Canary and production deployment verified using fetched URL, content and published SHA; rollback documented.
11. Document accurate feature limitations and publishing lag; never label planned work guaranteed.
12. Report precise latency baseline/improvement, PRs, release/deploy status, blockers and tests.
