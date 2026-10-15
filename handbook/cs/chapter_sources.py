"""What each chapter of the handbook rests on in the repository.

One line per chapter: the documents, product features, capability modules, commands, decisions
and rules the chapter is about. assemble.py resolves every reference against the repository's
own generated graph (docs/generated/graph.json) and refuses to build when one of them is no
longer there, so a chapter cannot keep describing something the repository has renamed or
removed. What the chapter's own text links to is found automatically and added to this.

Reference syntax: kind:label, where label is the graph's label for that kind
(doc:docs/MESH.md, feature:mesh, module:mesh, command:finish, adr:0067, rule:project.x, claim:x).
"""

SOURCES = {
    # Úvod
    "uvod": "doc:docs/CONCEPTS.md doc:docs/HOW_IT_WORKS.md doc:docs/PRODUCT.md feature:declare-once feature:interfaces",
    "problemy": "doc:docs/WHY.md feature:why module:why",
    "stavba": "doc:docs/HOW_IT_WORKS.md doc:docs/SURFACES.md feature:interfaces module:graph",
    "stav": "doc:docs/CONCEPTS.md doc:docs/SCHEMAS.md module:objects",
    "projekce": "doc:docs/CAPABILITIES.md doc:docs/SURFACES.md feature:declare-once feature:interfaces module:capabilities adr:0002 adr:0004",
    "mj-way": "doc:docs/DOCTRINE.md doc:docs/CAPABILITIES.md doc:docs/DELIVERY.md doc:docs/HANDBOOK.md feature:declare-once feature:doctrine feature:finish-contract adr:0004 rule:project.derived-files-regenerated rule:project.rule-is-a-doctrine rule:project.no-claim-without-test rule:project.work-is-claimed-before-it-is-built rule:project.worktree-topology rule:project.land-and-publish rule:project.the-handbook-is-built-from-the-tree",
    "nasazeni": "doc:docs/ADOPTION.md doc:docs/ADOPTION_FIRST_RUN.md doc:docs/INSTALL.md feature:install command:init command:update command:doctor",
    "osobni": "doc:docs/PERSONAL_MODE.md doc:docs/ADOPTION.md feature:install command:init command:update command:doctor",
    # Harness
    "h-co-je": "doc:docs/HOW_IT_WORKS.md doc:docs/CONCEPTS.md feature:policy",
    "h-proc": "doc:docs/WHY.md doc:docs/EDITORS.md feature:why",
    "h-mapa": "doc:docs/HOW_IT_WORKS.md feature:finish-contract feature:ci command:start command:check command:finish",
    "h-prubeh": "doc:docs/HOW_IT_WORKS.md doc:docs/BRIEF.md module:lifecycle module:brief command:context",
    "h-bootstrap": "doc:docs/EDITORS.md feature:policy module:editors command:update",
    "h-hooky": "doc:docs/EDITORS.md module:lifecycle module:episodes command:capture command:prompt",
    "h-server": "doc:docs/MCP.md feature:coordination module:server module:peers module:mcp adr:0003 adr:0044",
    "h-scope": "doc:docs/SCOPE.md module:repository command:start command:check",
    "h-profily": "doc:docs/CONTEXT.md doc:docs/MODELS.md feature:models feature:policy",
    "h-zavazky": "doc:docs/DELIVERY.md feature:finish-contract module:obligations command:start",
    "h-finish": "doc:docs/DELIVERY.md feature:finish-contract module:obligations command:finish",
    "h-reasoning": "doc:docs/REASONING.md feature:reasoning module:reasoning adr:0098",
    "h-git": "doc:docs/COMMIT.md feature:commit module:commit command:doctor command:finish",
    "h-ci": "doc:docs/CI.md feature:ci module:gates",
    "h-doctor": "doc:docs/PREFLIGHT.md module:health command:doctor",
    "h-vynuceni": "doc:docs/DOCTRINE.md feature:doctrine module:rules command:rules",
    "h-mereni": "doc:docs/ECONOMICS.md doc:docs/PERFORMANCE.md feature:economics feature:benchmarks module:economics module:perf",
    # Paměť a pravidla
    "p-mapa": "doc:docs/CONCEPTS.md doc:docs/SCHEMAS.md module:directories",
    "p-zaznamy": "doc:docs/CONCEPTS.md doc:docs/CONTINUITY.md command:checkpoint command:handover command:decision command:question",
    "p-pamet": "doc:docs/KNOWLEDGE.md doc:docs/CONTINUITY.md feature:knowledge feature:continuity",
    "p-session": "doc:docs/CONTINUITY.md module:session_domain module:episodes command:session",
    "p-briefing": "doc:docs/BRIEF.md doc:docs/CONTEXT.md feature:context module:brief module:devcontext command:context",
    "p-rozhodnuti": "doc:docs/CONCEPTS.md command:decision command:adr",
    "p-znalosti": "doc:docs/KNOWLEDGE.md feature:knowledge module:knowledge_base command:knowledge",
    "p-pravidla": "doc:docs/DOCTRINE.md feature:doctrine module:rules command:rules command:doctrine",
    "p-skills": "feature:skills module:skills command:skills command:prompt",
    "p-stroje": "doc:docs/CONTINUITY.md feature:continuity module:continuity",
    # Koordinace a mesh
    "koordinace": "doc:docs/MCP.md doc:docs/MESH.md doc:docs/WORKTREES.md feature:coordination module:peers module:mesh",
    "m-vrstvy": "doc:docs/MESH.md feature:mesh module:mesh adr:0067",
    "m-identita": "doc:docs/MESH.md feature:mesh module:mesh",
    "m-cesta": "doc:docs/MESH.md feature:mesh module:mesh",
    "m-handshake": "doc:docs/MESH.md feature:mesh module:mesh",
    "m-journal": "doc:docs/MESH.md feature:mesh module:mesh",
    "m-claims": "doc:docs/MESH.md feature:mesh feature:coordination module:mesh",
    "m-handovery": "doc:docs/MESH.md doc:docs/CONTINUITY.md feature:mesh module:mesh command:handover",
    "m-stroje": "doc:docs/MESH.md doc:docs/DEVELOPMENT_RUNTIME.md feature:mesh module:mesh module:server",
    "m-bezpecnost": "doc:docs/MESH.md feature:mesh module:mesh claim:no-network",
    "m-diagnostika": "doc:docs/MESH.md doc:docs/MONITOR.md feature:mesh module:mesh module:monitor",
    "m-overeni": "doc:docs/MESH.md doc:docs/DOGFOODING.md feature:mesh module:mesh",
    # Plán, důkazy a provoz
    "x-plan": "doc:docs/PLANNING.md doc:docs/ROADMAP.md feature:planning module:plan command:plan",
    "x-intent": "doc:docs/PLANNING.md module:intents module:intent_realization",
    "x-worktree": "doc:docs/WORKTREES.md feature:worktrees module:worktree",
    "x-env": "doc:docs/ENVIRONMENT.md module:environment",
    "x-evidence": "doc:docs/EVIDENCE.md feature:evidence feature:provenance module:evidence command:evidence",
    "x-claims": "doc:docs/USE_CASES.md doc:docs/CATALOGUE.md feature:use-cases feature:evidence command:usecase",
    "x-registry": "doc:docs/CAPABILITIES.md feature:declare-once module:capabilities module:commands adr:0002",
    "x-cockpit": "doc:docs/COCKPIT.md doc:docs/UI.md doc:docs/MAJORDOMUS_UI.md feature:cockpit feature:ui module:dashboard module:web",
    "x-exec": "doc:docs/EXECUTIONS.md module:executions",
    "x-release": "doc:docs/RELEASE.md doc:docs/DISTRIBUTION.md feature:deployments module:release module:distribution command:version",
    "x-lessons": "doc:docs/DOGFOODING.md doc:docs/BACKLOG.md feature:knowledge",
    # Hranice
    "m-zaruky": "doc:docs/MESH.md feature:mesh module:mesh",
    "p-hranice": "doc:docs/KNOWLEDGE.md doc:docs/CONTINUITY.md feature:knowledge feature:continuity",
    "h-hranice": "doc:docs/HOW_IT_WORKS.md doc:docs/SCOPE.md claim:no-network",
    "velka-org": "doc:docs/ADOPTION.md doc:docs/INTEGRATION.md doc:docs/MESH.md feature:coordination feature:mesh",
    "h-prenos": "doc:docs/ADOPTION.md doc:docs/DOCTRINE.md feature:doctrine feature:policy",
    # Repozitář v číslech
    "cisla-tempo": "doc:docs/RELEASE.md module:release command:history",
    "cisla-commity": "doc:docs/COMMIT.md feature:commit module:commit",
    "cisla-adr": "doc:docs/CONCEPTS.md command:adr",
    "cisla-pravidla": "doc:docs/DOCTRINE.md doc:docs/CI.md feature:doctrine feature:ci module:rules module:gates",
    # Reference
    "prikazy": "doc:docs/CLI.md doc:docs/COMMANDS.md module:commands command:help",
    "p-prikazy": "doc:docs/CLI.md doc:docs/KNOWLEDGE.md doc:docs/CONTINUITY.md command:knowledge command:session command:handover",
    "m-prikazy": "doc:docs/CLI.md doc:docs/MESH.md module:mesh",
}
