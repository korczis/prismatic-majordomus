# MAJORDOMUS NEXUS — Claude Code Master Prompt + Prompt Pack

**Verze packu:** 1.0.0  
**Vygenerováno:** 2026-11-07  
**Účel:** Implementovat a doručit jednotný, persistentní, evidencí řízený intent gateway přes stávající Majordomus, bez paralelní orchestrace a bez opakovaných lidských rozhodnutí.

## Co dostáváš

- `MASTER_CLAUDE_CODE_PROMPT.md`: úplný autonomní implementation mandate. Spustit jako první.
- `pack.json`: machine-readable DAG, dependencies a doporučené integration lanes.
- `prompts/00_...md` až `prompts/18_...md`: **19 samostatných, úzce vymezených kroků** s technickým zadáním a akceptací.
- `contracts/`: společné invariants: autonomní konzultace AI peers, bezpečné dokončení, typed modely a řízení kontextu.
- `references/`: co bylo **ověřeno v historickém ZIP snapshotu** a co je nutné znovu ověřit v aktuálním repozitáři.
- `examples/`: E2E scénáře a dodávkové evidence.
- `CHECKSUMS.sha256`: integrita vygenerovaných souborů.

## Rychlý start

1. Otevři **aktuální** checkout `prismatic-majordomus` v Claude Code s fungujícím přístupem k jeho existujícím nástrojům. Výchozí větev/HEAD musí Claude zjistit sám.
2. Dej Claude Code k dispozici tento rozbalený pack v adresáři, který může číst, aniž by obcházel scope/trust pravidla repozitáře. Pokud jej pouze připojíš jako soubory, použij přímo `MASTER_CLAUDE_CODE_PROMPT.md` a ostatní soubory podle potřeby. Nevkládej jej automaticky do `.ai/local/`, které má zvláštní pravidla pro soukromý runtime state.
3. Pošli mu obsah `START_HERE.md` jako první pokyn. Alternativa: vlož celý `MASTER_CLAUDE_CODE_PROMPT.md` přímo do session. Master funguje i samostatně, když pack není dostupný.
4. Sleduj jen skutečné external blockers (oprávnění, chráněná větev, credentials, bezpečnostní volby). Ostatní má Claude vyřešit z `.ai`, source, Git/history, testů a přípustných konzultací s existujícími peers.

**Rozbalení ZIPu není deployment.** Pack je exekuční zadání pro Claude Code, nikoliv implementace změn Majordomusu. Skutečnou integraci/master/deploy musí Claude provést a prokázat v repozitáři.

## Doporučený režim práce

- Jeden master mission / canonical intent pro celou práci; každé číslo je checkpoint, ne nutně PR ani issue.
- **Foundation** `00–04`: ověření současného stavu, canonical contracts, capture a hooks; první použitelný vertical slice.
- **Intelligence** `05–10`: context compiler, intent continuity, Proceed/IDD/Mesh, durable recovery, governance.
- **Experience** `11–14`: persistentní intent composer, derived CLI/API/MCP/Cockpit, živý UI, economics.
- **Hardening** `15–18`: fault injection, security/performance, delivery/merge/release, E2E a cleanup.

Cíl jsou **4 kohezentní integration lanes, přibližně 1 PR na lane**, nikoliv 19 PR. Aktuální branch protection a team concurrency mohou vynutit jiný počet. Každý prompt se má odbavit jako nejmenší bezpečný, verifikovatelný krok, ideálně přibližně do 30 minut cílené práce; pokud nestačí, automaticky jej rozděl na interní checkpointy, nikoliv na nové otevřené PRs. Čas není důvod obejít gate.

## Pravidla autonomy

1. Před novým rozhodnutím načti aktuální `.ai` a cílené Git/source/test evidence. Nevytvářej vymyšlené znalosti.
2. Při skutečné nejasnosti použij existující Majordomus mesh/peers a přiděl odlišné role: navrhovatel, kritik a verifikátor. Pokud peer není dostupný, řekni `unavailable`, nevymýšlej jeho radu.
3. Než vybereš alternativu, proveď adversarial review a zkus ji zneplatnit testem. Peer majority není důkaz.
4. Ptej se uživatele jen když skutečně chybí oprávnění, volba s nevratnými důsledky nebo kritický bezpečnostní/finanční decision. Zbylé reverzibilní práce mohou pokračovat.
5. Změny do `.ai/**` materializuj jen pokud odpovídají skutečné validované změně, přes canonical rules a generátory. Prompty a telemetry patří do příslušného private runtime storage, ne do globálního KB.
6. Zachovej stávající task lifecycle, command/capability registry, worktree topology, GitHub gates, docs a semantic release. No cheating, no optimistic green.
7. `done`, `merged`, `deployed` jsou různé faktické stavy. U každého dej SHA/CI/probe evidence.

## Důležité omezení zdrojů

Uživatelský ZIP obsahuje stav **z 2026-09-11**. Z něj plyne, že část potřebné infrastruktury už existovala: Claude hooks/capture, `.ai` manifest, skills/workflows, MCP peer server, canonical capabilities, task lifecycle, worktrees, claims, CI a pages. Proto prompt pack nezačíná greenfield architekturou. **Žádný popis historického snapshotu neprokazuje stav masteru 2026-11-07.** GitHub konektor nebyl během generování packu schopen spolehlivě potvrdit aktuální stav; Claude ho musí ověřit v aktuálním checkoutu.

Oficiální hook reference: https://code.claude.com/docs/en/hooks . Pro mod/plugin support a settings precedence ověř aktuální installed verzi a dokumentaci před změnou souborů.

## Ready/Done/Blocked

- `READY`: všechny dependency prompty mají ověřený výsledek na odpovídajícím pracovním SHA a výstup lze dále použít.
- `DONE`: code/docs/tests/evidence lokálně prokázány; completion contract splněn, změna zatím může čekat na lane integration.
- `INTEGRATED`: související PR je skutečně merged do správné base (ověřená SHA).
- `DEPLOYED`: aktuální runtime/site ukazuje konkrétní revision a probe prošel.
- `BLOCKED`: přesný externí důvod a konkrétní chybějící oprávnění nebo check; ne jen věta „potřebuju další informace“.

## Výstupy, které vyžadovat na konci

1. Ke kterému intentu Nexus navázal několik promptů, bez duplicitních issues.
2. Jaké `.ai` rules/ADR/KB/Git evidence skutečně ovlivnily rozhodnutí.
3. Jaký existing Proceed/IDD/Mesh/execution runtime byl reálně použit, ne duplikován.
4. Jak funguje durable restart/compact/replay a jak jsou chráněné side effects.
5. Jak odpovídají stejné ID/status/evidence CLI/API/MCP/Cockpit/docs.
6. Verifikované SHA/PR/CI/merge/deploy/probes, verze a zbylé skutečné blockers.
