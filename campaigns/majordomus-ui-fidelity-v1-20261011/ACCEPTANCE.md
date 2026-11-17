# Final acceptance gates

1. High-level `majordomus ui` entry works with current shared server; no separate backend or fabricated UI state.
2. Reference image fidelity: information hierarchy, dense cards, rail, layout and palette resemble screenshot at desktop widths. Dynamic values must be from source or explicitly unavailable.
3. All six KPI cards, graph, three chart panels, PR rows, mesh list, roadmap and activity feed either show real data with provenance or honest empty/unsupported/error state.
4. Graph nodes/edges match canonical graph; chart data matches table representation, metric units/window/sample counts are explicit.
5. All navigation paths resolve, actions go through canonical typed executor and authorization; error, latency, denial and disconnect tested.
6. No-JS, mobile, keyboard, focus, contrast, reduced motion and light/dark mode comply with existing `.ai` and WCAG expectations.
7. No added browser console errors, CSP violations, secrets leaks or dependency risk; asset and performance budgets measured.
8. `.ai` rule/CI gate migration is actually enforced by reproducible negative tests. Docs, CLI help/completion and GitHub Pages projections updated and freshness-checked.
9. Validate same behavior on Majordomus repo and one consumer repo, where no data is available show proper absent state.
10. Version bump follows canonical version policy; merged, released, deployed and verified evidence documented, not assumed.
