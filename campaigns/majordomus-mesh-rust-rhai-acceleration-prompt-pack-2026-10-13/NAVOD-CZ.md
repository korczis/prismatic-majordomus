# Návod česky

**Nejjednodušší start:** rozbal ZIP, zkopíruj celý `prompts/00-MESH-COMMANDER.md` do hlavní Claude Code session v aktuálním repozitáři Majordomusu. Tento prompt nejprve načte `.ai/`, ověří aktuální stav a podle dostupného meshe rozdělí práci.

**Čtyři terminály:** 1) Commander+integrator, 2) CI/evidence, 3) Rust/Rhai/plugin kernel, 4) grafy/UI + derivace. Rozdělení je pouze počáteční doporučení, skutečné mandáty a scopes si musí workeři zjistit ze stavu meshe. Specializované prompty lze vložit do dalších terminálů až po potvrzení nekolidujících scopes.

**Pořadí:** zelený master → malý bezpečný merge + deploy → canonical Rust dispatch → omezený Rhai → externí plugin → automatická projekce → grafový drilldown → port shellu → beta důkaz. Paralelní práce je povolena, ne však paralelní konflikt nad stejnými cestami.

**Release:** cílit na 10–20 minut mezi smysluplnými ověřenými vydáními. Když CI nebo release verdict není zelený, nic neobcházet. Verze bez ověřeného public deploymentu není hotové doručení.

**Zásada MJ:** jeden typed canonical capability source, Rust jako authority, Rhai kompozice, permissioned pluginy, CLI/MCP/API/peers/UI/docs/just/shell pouze projekce. Rozhodně ne přepsat shell na Rhai skripty volající bash.
