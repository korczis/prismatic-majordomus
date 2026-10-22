+++
title = "Znalost odvodí nástroj, povýší ji člověk"
description = "Co se session naučila, obvykle zůstane v konverzaci, ve které se to naučila. Jak Majordomus v okamžiku, kdy epizoda končí, promění rozhodnutí, odpovědi a výsledky, které zaznamenala, v kandidátní znalost bez modelu a bez čtení konverzace, a proč kandidáta může ověřit jen člověk."
template = "page.html"
[extra]
lang = "cs"
translated_from = "knowledge-is-derived-a-person-promotes-it/"
source_digest = "54cbabb8fd1a4ed9a367307c1f473fc326696c5faea62d3f830d542ae76d62a4"
related = ["session-skonci-prace-ne/", "pravidlo-se-dokazuje-odmitnutim/", "napsano-jednou-odvozeno-vsude/"]
[extra.translations]
en = "knowledge-is-derived-a-person-promotes-it/"
+++

Epizoda práce zaznamenává rozhodnutí, řeší otázky a dokončuje tasky a každá z těch věcí je typovaný řádek v ledgeru jednoho stroje. Ledger je lokální a má strop, takže rozhodnutí je v `majordomus history` vidět jen do chvíle, než ho strop odstraní. Každý řádek je důkazem něčeho pravdivého o repozitáři, a dokud nevznikl subsystém znalostí, nic z toho se nedostalo do sledovaného stromu, do jiného checkoutu ani na rozhraní, které plní registr. [ADR 0091](@/adrs/adr-0091.md) (navržené) rozhoduje, jak se tam dostane: znalost se odvodí na hranici epizody z ledgeru a z gitu a povýší ji člověk.

## Jeden druh, žádná paměťová služba

Znalostní báze je druh `knowledge` v `.ai/repo/knowledge/` a nic jiného: žádný nový druh, žádné druhé úložiště, žádná paměťová služba a žádný agent ([ADR 0010](@/adrs/adr-0010.md), navržené, a [project.no-new-nouns](@/rules/project-no-new-nouns-1.md)). Záznam je jedno tvrzení o repozitáři na jednom řádku. Jeho třída říká, co se tvrdí (fakt, konvence, omezení, paměť nebo poučení), jeho stav, kdo za ním stojí (odvozovač, člověk, nebo už nikdo), jeho epistemika, jak je známé (pozorované, odvozené nebo rozhodnuté), a jeho provenience důkazy v podobě typovaných odkazů, které se dají dohledat: session, rozhodnutí, task, commit, issue, soubor nebo test.

Záznam nikdy neshrnuje konverzaci a nikdy necituje prompt ani tah konverzace. Kontrola, která drží [project.never-store-transcripts](@/rules/project-never-store-transcripts-1.md), platí pro jeho název, popis i tělo. Záznam o rozhodnutí, pravidlo nebo manuál je samostatný zdroj a záznam se k němu vztahuje, nikdy ho neopakuje.

## Co se odvozuje a z čeho

Odvozovač implementuje jedinou tabulku a nic nad ni a [`docs/KNOWLEDGE.md`](@/docs/knowledge.md) ji uvádí. Zaznamenané rozhodnutí se stane konvencí (`convention`), kterou epizoda rozhodla. Vyřešená otázka se stane faktem (`fact`), uvedeným spolu s otázkou, na kterou odpovídá. Task, který skončil zablokovaný, selhal nebo nenašel shodu, se stane poučením (`lesson`), odvozeným z prvního řádku důvodu v jeho poznámce. Task dokončený s ověřovacím příkazem se stane faktem: ověřením, které prošlo. Commit epizody, který se dotkl pravidla nebo záznamu o rozhodnutí, přidá tento commit a tento soubor ke každému záznamu, který epizoda napsala. Dokončený task bez ověření nedá nic a stejně tak další krok z handoveru, protože ten je stavem pokračování, ne znalostí.

Každé odvozené rozhodnutí je konvence. Rozhodovat podle jeho znění, zda zavazuje, by znamenalo odvozovat druh z prózy, a ten úsudek patří člověku, který ho povyšuje. Id záznamu je id epizody následované digestem jeho důkazů, datum je den důkazu, nikdy hodiny, a do souboru nevstupuje žádná náhodná hodnota. Stejný ledger a stejný git proto dají stejné bajty a dva worktree nikdy nezapíšou pod jedno jméno různý obsah. Řádek ledgeru, který nese značku přepisu konverzace, nedá žádný záznam a hlásí se jako přeskočený. Nic nevolá síť, model ani `eval`.

Od [ADR 0118](@/adrs/adr-0118.md) (navržené) může worker zanechat i to, na co cestou narazil: `majordomus knowledge observe` zaznamená tření, obcházku, defekt, drift nebo opakování týkající se nějakého subjektu a hranice z toho udělá poučení (`lesson`), pozorované a podle subjektu nasměrované na platformu, vynucování, generátor, dokumentaci nebo projekt. Slova se nikdy nečtou a směrování nic nerozhoduje.

## Okamžik, kdy to přestává být dosažitelné

Odvozovač běží ve dvou okamžicích: při koncové události poskytovatele a při jeho kompakci, tedy ve chvílích, kdy to, co epizoda ví, přestává být dosažitelné. Po každém uzavření, ať je to hook poskytovatele, nebo člověk, který napíše `majordomus session close`, následuje řádek `knowledge.derived`, v pořadí ledgeru ještě před samotným uzavřením. Kompakce nemá žádné uzavření, na kterém by se svezla, a tak její adaptér zavolá odvozovač sám. Obojí běží bez ohledu na to, zda je aktivní nějaký task, protože životní cyklus patří epizodě, a obojí lze v politice vypnout.

Trunk, který se mění jen přes pull requesty, nemůže commitnout, co do něj hranice zapíše. Od dodatku z 15. října 2026 (I1984) zapisuje epizoda na trunku své kandidáty a svůj záznam session do outboxu, který patří jen danému checkoutu, a další uzavření na kterékoli jiné větvi téhož klonu přenese každý soubor beze změny a pod jeho vlastním jménem do sledovaných oddílů, odkud ho na trunk přinese pull request té větve. `majordomus context` i `doctor` neprázdný outbox pojmenují a ani jeden ho nepovažuje za selhání.

## Ověřenou ji udělá člověk

Stav `verified` zapisuje úkon, nikdy odvozovač. `majordomus knowledge promote <id>` přesune kandidáta do `curated/`, nastaví třídu, kterou člověk zvolil, ponechá jeho provenienci a přidá k ní povyšující epizodu a připojí důkazy, které člověk předá na standardní vstup. Prázdný vstup se odmítne, protože úkon bez důkazů je jen tvrzení, a vstup se značkou přepisu konverzace se odmítne ze stejného důvodu, proč ji nesmí nést záznam. `majordomus knowledge reject <id> --reason` ponechá kandidáta jako nahrazený záznam s důvodem a případně se záznamem, který ho nahrazuje. Odvozovač id, které bylo povýšeno nebo odmítnuto, přeskočí, takže další hranice žádný z těch úkonů nevrátí.

Fronta se měří a stroj ji nikdy nevyprazdňuje. Briefing, kterým začíná další epizoda, pojmenuje kandidáty čekající na její větvi na review a doporučující pravidlo [majordomus.candidates-reviewed](@/rules/majordomus-candidates-reviewed-1.md) hlásí frontu nad stropem politiky, kandidáta staršího, než politika připouští, i kandidáta, kterého nesleduje správa verzí. O tak starém kandidátovi manuál říká, že nebyl zkontrolován, ale ignorován.

## Provenience přežije soubory, které cituje

Soubor nebo test, který záznam cituje, může ze stromu později zmizet. Od I1987 se takový odkaz dohledá v historii gitu, k nejnovějšímu commitu, který cestu obsahoval nebo smazal, a `majordomus knowledge check` ho pojmenuje jako historický, místo aby ho mlčky nechal projít. Cesta, kterou žádný commit v historii nikdy neobsahoval, se dál nedohledá nikam. Smazání citovaného souboru a jeho commit drží případ 3023.

## Když zapisovač ustane

Blokující pravidlo [majordomus.knowledge-observed](@/rules/majordomus-knowledge-observed-1.md) hlásí zastaveného zapisovače: nejnovější uzavřenou epizodu nejmenuje žádné odvození a je starší, než dovoluje čerstvost deklarovaná v politice, nebo je přepínač zapnutý a životní cyklus odvozovač nikdy nezavolá. Checkout, jehož ledger nedrží vůbec žádné odvození, se hlásí jako přeskočený, takže v den, kdy odvozovač přibyl, žádný checkout nezčervenal. Blokující pravidlo [majordomus.knowledge-integrity](@/rules/majordomus-knowledge-integrity-1.md) odmítne záznam, který není platný podle schématu, jednoznačně identifikovaný, podložený důkazy, je-li ověřený, úplně dohledatelný a správně nahrazený, kandidáta, který o sobě tvrdí, že je ověřený, i záznam, který nese konverzaci.

## Jak to ověřit

`majordomus knowledge candidates` vypíše frontu a záznamy o jednom subjektu seskupí s epizodami, které za nimi stojí, a `majordomus knowledge derive --dry-run` ukáže, co by otevřená epizoda zapsala. `majordomus knowledge check` zkontroluje oba adresáře a `majordomus-cli knowledge status` hlásí poslední odvození, nejnovější uzavření a úsudek o zastaveném zapisovači. `majordomus history --event knowledge.derived` ukáže každé odvození tak, jak ho zaznamenal ledger.

## Co tento článek netvrdí

Netvrdí, že znalostní báze drží všechno, co se epizoda naučila. Odvozovač zapíše jen to, co uvádí jeho tabulka, a nic, co zaznělo v konverzaci a nebylo zaznamenáno, se k němu nedostane. Kandidát také nic netvrdí, dokud ho člověk nepovýší. ADR 0010, 0091 a 0118 jsou navržená a vlastník nepřijal žádné z nich; odvozovač, příkazy a pravidla, která popisují, jsou na masteru a běží. Jak práce pokračuje přes jednotlivé session, popisuje [Session skončí, práce ne](@/session-skonci-prace-ne.md), jak se pravidlo váže k případu, který může selhat, vysvětluje [Pravidlo se dokazuje odmítnutím](@/pravidlo-se-dokazuje-odmitnutim.md) a proč odvození dá pokaždé stejné bajty, ukazuje [Napsáno jednou, odvozeno všude](@/napsano-jednou-odvozeno-vsude.md).
