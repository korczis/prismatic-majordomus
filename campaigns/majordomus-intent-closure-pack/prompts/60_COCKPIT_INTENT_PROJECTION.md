# Milestone 60: First-Class Intent UX in Cockpit

## Mission

Build a useful Cockpit projection over the canonical intent model and lifecycle now that semantics underneath are real.

Do not put business rules in frontend code. Cockpit renders and interacts with canonical API/state.

## 1. Audit existing Cockpit architecture

Find design system, route/data conventions, generated/static projections, API usage, interaction framework, accessibility/testing standards and responsive patterns.

Reuse them exactly.

## 2. Intent overview

Add a first-class intent view that can answer at a glance:

- what outcome are we pursuing;
- current intent stage;
- required invariants;
- acceptance criteria and satisfaction state;
- evidence behind each criterion;
- milestones/issues carrying each criterion;
- active sessions/workers;
- opposition status/disposition;
- gaps/blockers;
- GitHub reconciliation state;
- why completion is blocked or allowed.

## 3. Visual hierarchy

Prefer progressive disclosure over a giant data dump.

At minimum provide:

- summary/status header;
- criteria/evidence matrix;
- plan/work breakdown;
- blockers/gaps;
- opposition panel;
- provenance/explain links.

Use the project's current visualization stack when a dependency graph materially helps. Do not add libraries merely because they are fashionable.

## 4. No mocked state

All displayed data must come from canonical real state/fixtures used by tests. No hardcoded example counts, fake statuses or frontend-only sample intents in production paths.

## 5. Interactions

Where current APIs support safe mutations, allow appropriate actions such as:

- inspect/validate intent;
- trigger/re-run opposition;
- inspect evidence;
- reconcile GitHub;
- navigate to issue/session/milestone;
- inspect completion blockers.

Do not expose a button that can directly mark criteria/intent satisfied bypassing canonical policy.

## 6. URL/deep linking

Intent, criterion, evidence and related issue/session links should be stable/deep-linkable according to existing Cockpit routing conventions.

## 7. Accessibility and responsive UX

Meet existing UI conformance gates. Ensure status is not communicated only by color and dense graphs/tables degrade sensibly on small screens.

## 8. Tests

Add backend/data contract plus UI tests for:

- intent list/detail;
- satisfied/unsatisfied/stale evidence states;
- opposition blocked state;
- no-work coverage gap;
- completed intent;
- GitHub drift state;
- empty/legacy state;
- deep links;
- responsive/accessibility rules in existing test framework.

## 9. Docs

Update user-facing docs/screenshots/generated references according to repository convention. Avoid manually duplicating data inventories.

## Acceptance criteria

- Cockpit exposes the complete intent lifecycle from canonical state;
- a user can understand why an intent is or is not complete without reading raw files;
- no satisfaction semantics are implemented in frontend code;
- no mocked production data;
- UI behavior is tested and passes existing conformance gates.
