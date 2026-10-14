# Majordomus IDD Completion Prompt Pack

Tento pack je určený k dotažení **Intent Driven Development** v Majordomusu z dnešních existujících primitives do jednoho uzavřeného, testovaného a veřejně ověřeného lifecycle.

Nejde o rewrite. Snapshot repozitáře z 11. 9. 2026 už obsahoval executable milestones/issues, derived status, DAG/waves, evidence gates, sessions, handovers, peer context, doctrines, canonical projections a deployment/test infrastrukturu. Hlavní práce je **konvergence kolem explicitního canonical Intentu + satisfaction loop**.

## Doporučené pořadí

1. `00_MASTER_ORCHESTRATOR.md` — hlavní prompt; může řídit celý zásah sám.
2. `01_ARCHITECTURE_AUDIT_AND_MIGRATION_MAP.md` — pokud chceš nejdřív tvrdý audit bez implementační slepoty.
3. `02_CANONICAL_INTENT_MODEL.md` — first-class Intent + provenance + lineage.
4. `03_INTENT_COMPILER_AND_PLANNING.md` — current→desired gap, decomposition, milestones/issues/DAG/waves.
5. `04_EXECUTION_MESH_HANDOVER.md` — sessions, peers, handovers, provider-neutral continuity.
6. `05_SATISFACTION_EVIDENCE_DRIFT.md` — evidence, re-evaluation, drift, proof bundle.
7. `06_COCKPIT_AND_SURFACES.md` — CLI/API/OpenAPI/MCP/Cockpit/docs z jednoho modelu.
8. `07_ENFORCEMENT_AND_E2E.md` — rules/doctrines/gates, cross-provider E2E a anti-regression.
9. `08_DEPLOY_VERIFY_CLOSE.md` — merge, deploy, public verification, closure.
10. `09_ADVERSARIAL_REVIEW.md` — red-team audit po implementaci; nesmí jen potvrdit předchozí práci.
11. `10_KEEP_IT_TRUE.md` — navazující prompt na udržení IDD bez regresí.

Každý prompt je záměrně dostatečně samostatný, aby mohl být vložen do nové Claude Code / Codex / jiné session. Soubor `_SHARED_CONTRACT.md` obsahuje společnou architektonickou smlouvu.

## Praktické použití

Pro jednu silnou session začni `00_MASTER_ORCHESTRATOR.md`. Pokud práce poběží paralelně přes více peers, použij 01–07 jako oddělené mandáty, ale **nejdřív nechej orchestrátor vytvořit/ověřit scopes a dependency order**, jinak si peers vyrobí přesně ten typ kolize, který Majordomus tvrdí, že řeší.

Po implementaci vždy spusť `09_ADVERSARIAL_REVIEW.md` v čerstvé session/provideru. Ten prompt má za úkol hledat důkazy proti tvrzení „IDD je hotové“.

## Kritický acceptance test

IDD v1 považuj za skutečné teprve tehdy, když jeden E2E scénář prokáže:

```text
Human declares Intent
→ canonical Intent persists
→ current reality is discovered
→ required work is derived/linked
→ work executes across at least two sessions/providers
→ handover preserves lineage and unresolved obligations
→ tests/runtime produce evidence
→ one condition fails and Intent remains UNSATISFIED
→ remaining work is derived/reopened
→ fix changes observed reality
→ verification passes
→ Intent becomes SATISFIED
→ Cockpit/API/CLI/MCP/docs expose the same lineage/proof
→ deployment/public projection is verified where applicable
```

Pokud chybí tento closed loop, máte jednotlivé primitives, ne dokončené IDD.
