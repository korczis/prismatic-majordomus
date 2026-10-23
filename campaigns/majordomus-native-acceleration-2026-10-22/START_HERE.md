# Spouštěcí prompt

Následující text předej implementačnímu agentovi spolu s cestou k rozbalenému packu.

```text
Pracuješ v prismatic-majordomus. Použij přiložený pack pro nativní
akceleraci jako implementační zadání, ne jako důkaz chybějících funkcí.

Nejprve přečti MASTER_PROMPT.md, CONTRACTS.md a EXECUTION_PLAN.md.
Dodrž skutečný discovery protokol repozitáře a proveď P00.
Nevkládej celý pack do always-loaded kontextu.

Vyber nejvýše tři malé, připravené řezy s nejlepším doloženým poměrem
přínosu a celkového integračního nákladu. Preferuj existující nativní
funkce, správnou konfiguraci a opravu jednoho adaptéru před novou službou.
Výchozí kandidáti jsou P01 a P03; P02 pouze při dostupném lokálním LSP.
P07 povyš, jestliže měření prokáže výrazný overfetch.
Doložená nefunkční blocking kontrola má bezpečnostní přednost.

Pro každý řez načti jen jeho prompt, relevantní zdroje a testové scénáře.
Najdi kanonického vlastníka, ověř oprávnění a skutečnou podporu,
změř baseline, implementuj, otestuj negativní případ, regeneruj pouze
relevantní projekce a vyhodnoť přínos. Neskonči jen obecným návrhem.

Nevytvářej druhý registry, planner, event bus, ledger nebo collector.
Neobcházej rules, trust review, sandbox, rozpočet ani finish contract.
Nepřepínej subscription na API a nespouštěj placené benchmarky bez
existujícího výslovného oprávnění. Bez něj pokračuj offline testy.

Dokončené řezy převezmi přes P09. Zbytek označ přesným důvodem,
nikoli jako hotový. Při přerušení zanech minimální handover.
Pro rutinní technické volby využij repo kontrakty; u chybějícího
oprávnění nevymýšlej obcházení a pokračuj nezávislou oprávněnou prací.
```
