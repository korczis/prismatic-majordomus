# Majordomus Live Intent Workbench | Claude Code prompt pack

**Datum sestavení:** 2026-11-07  
**Ověřený master:** `d7fd9a3617024a7390ac36c1888d6fcee63b600e` (před prací vždy znovu ověřit)  
**Repo:** https://github.com/korczis/prismatic-majordomus

## Účel
Dovést už existující `/ui/idd` od read-only intenty/plány k použitelnému workflow: napsat volný návrh → bezpečně uložit → získat důkazy z `.ai/`, KB/ADRs, gitu a historie → diskutovat nejasnosti → živě kompilovat prompt/pack → porovnat se stavem repo → schválit a materializovat do canonical intent/milestones/issues → vykonávat přes Majordomus Mesh.

## ASAP strategie
- **P00–P05:** první použitelný editor a prompt compiler. Při tomto checkpointu lze už skutečně pracovat, i kdyby materializace byla stále blokovaná existujícími issues.
- **P06–P09:** canonical reconciliation, shared validation, autorizovaná materializace, execution. Respektovat skutečné issues/ADRs a jejich závislosti.
- **P10–P11:** end-to-end, security, performance, docs, dogfooding, doručení na master, release/deploy proof.

## Jak spustit v Claude Code
1. Otevři checkout `prismatic-majordomus`, synchronizuj remote `master` a spusť běžné Majordomus environment/context preflight.
2. Vlož celý obsah `01-MASTER-ORCHESTRATOR.md` jako hlavní misi.
3. Jako další samostatné úkoly vlož soubory `prompts/00-P00-...md` až `prompts/11-P11-...md` v pořadí, pokud již daná schopnost není doloženě implementovaná na masteru.
4. **Každý prompt je atomický**: cílový aktivní čas do 30 minut, jeden opravdu použitelný doručovaný krok, testy, dokumentace, generování, případné verzování, CI/merge a deploy evidence. Žádné čekající mega-větve.
5. Po každém kroku ověř PR, master SHA, runtime feature a odpovídající issue; blokovaný krok nesmí být odškrtáván jako DONE.

## Předpřipravené podklady
- `02-BASELINE-AND-COLLISION-MAP.md`: konkrétní odkazy na zdrojové soubory, existující intenty, issues a dependency mapu.
- `03-ACCEPTANCE-AND-GATES.md`: E2E acceptance matrix a release podmínky.
- `PROMPTS-ALL-IN-ONE.md`: všechno v jednom souboru pro předání/archivaci.
- `MANIFEST.json` a `SHA256SUMS.txt`: seznam a kontrola integrity souborů.

## Kritické hranice
**Není to nový planning engine.** Není to nový druh entity `intake`. Není to automatický přepis `.ai/`. Není to publikování chat transcriptů. Není to remote write z neautorizovaného konektoru. Není to oprávnění přeskočit proposed ADR nebo závislost. A není to CLI-only prompt generator vydávající se za živý Workbench.

Technické zadání promptů je anglicky kvůli přesnosti agentního provedení. Všechny skutečné výsledky mají být odvozené ze stavu repozitáře v době vykonání.
