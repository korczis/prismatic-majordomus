+++
title = "Případ užití se spouští, ne popisuje"
description = "Seznam toho, co nástroj umí, je nejsnazší stránka k napsání a první, která zastará. Jak Majordomus drží každý případ užití jako jeden soubor, jehož scénář běží proti skutečnému nástroji v jednorázovém repozitáři, jak na webu ukazuje normalizovaný výstup toho běhu a proč nikdo nemůže zralost případu užití napsat ručně."
template = "page.html"
[extra]
lang = "cs"
translated_from = "a-use-case-is-run-not-described/"
source_digest = "2309215af73340b7e0d7897d34d516712a824557f05d69502e36fba2b2ef40a9"
related = ["dokumentace-kterou-lze-odmitnout/", "pravidlo-se-dokazuje-odmitnutim/", "napsano-jednou-odvozeno-vsude/"]
[extra.translations]
en = "a-use-case-is-run-not-described/"
+++

Případ užití je podle [`docs/USE_CASES.md`](@/docs/use-cases.md) úkol, který někdo s Majordomem vykonává, zapsaný jednou jako data a prokázaný proti skutečnému nástroji. Není to tutoriál, seznam funkcí ani stránka. Manuál svůj princip vyslovuje jednou: co lze odvodit, to se odvodí, co odvodit nelze, má jeden kanonický domov, a co se tvrdí jako zaručené, je podložené spustitelným důkazem. Seznam případů užití na tomto webu stojí na tom principu a tento článek sleduje jak.

## Jeden soubor, všechno ostatní odvozené

Případ užití je jeden soubor v `.ai/repo/use-cases/`. Jeho front matter jmenuje příkazy, nástroje MCP, pravidla, tvrzení, odpovědnosti a aplikace, kterých se dotýká, a jeho tělo má tři oddíly: situaci, scénář a výsledek. Stránka, kategorie, do které patří, odkazy na ni i z ní, přehled, které schopnosti mají případ užití, a důkazy, které stránka ukazuje, se odvozují. Není žádný registr k úpravě, žádná šablona, na kterou by se sahalo, a žádný počet, který by se zvyšoval.

Schéma odmítne klíč, který nedeklaruje. `majordomus usecase validate` odmítne odkaz, který se nedohledá, krok scénáře, který spouští příkaz, jejž případ užití neuvádí, tělo bez jeho tří oddílů i aktivní případ užití, který míří na `guaranteed` a nemá scénář.

## Scénář je důkaz

Scénář je jeden blok YAML mezi situací a výsledkem, takže čtenář potká situaci, důkaz a výsledek v pořadí, v jakém se dějí. `setup` jmenuje připravený stav, tedy skript fixture sdílený s ukázkami příkazů, takže případ užití začíná tam, kde začíná stránka příkazu. Každý krok je jedno skutečné volání `bin/majordomus` v jednorázovém repozitáři, zadané jako vektor argumentů a nikdy jako řetězec pro shell, a očekává návratový kód a tam, kde to uvádí, vzory ve výstupu nebo v souboru.

Krok může patřit i workerovi: jeden řádek shellu mezi dvěma voláními, zaznamenaný jako čin workera, ne nástroje. Jeden scénář tak může vyprávět celý příběh. Worker sejde z cesty, nástroj odmítne, worker to vezme zpět a nástroj přijme. Takovým scénářem je [výzva](@/challenge.md) na tomto webu a její stránka přehrává zaznamenaný běh.

## Fixture, nebo živě

Scénář říká, o kterém repozitáři je. Scénář nad fixture, výchozí volba, běží v jednorázovém repozitáři, který CI umí vytvořit znovu. Živý scénář klade otázky repozitáři, ve kterém byl příkaz spuštěn. Nic nepřipravuje a smí spouštět jen vektory argumentů, které `share/commands.yaml` deklaruje jako pouze čtecí, takže nemůže změnit to, na co se ptá. Může nést i krok závazku, který nic nespouští a tvrdí, že něco, co určitá třída práce dluží, bylo splněno, a to přes stejný úsudek, jaký čte `majordomus check`.

Verdikt živého scénáře je `unmet`, ne `fail`. Selhávající scénář nad fixture je vada nástroje, kdežto nesplněný závazek v živém scénáři je nedodělaná práce, a to nejsou stejné zprávy. Živé důkazy popisují jeden strom na jednom stroji v jedné minutě, takže se nikdy necommitují: odvozený soubor, jehož obsah závisí na tom, kdo ho odvodil, zakazuje [ADR 0005](@/adrs/adr-0005.md), přijaté. Ze stejného důvodu `usecase coverage` počítá živý scénář jako ten, který příkaz jmenuje, nikdy jako ten, který ho pokrývá: záruka, kterou CI nemůže spustit znovu, není záruka. Rozhodnutím je [ADR 0038](@/adrs/adr-0038.md) (navržené).

## Důkazy, které stránka může ukázat

`majordomus usecase run` zapíše pro každý krok příkaz, jeho argumenty, návratový kód proti očekávanému, normalizovaný výstup, každé tvrzení s výsledkem a časování. Normalizovaný znamená, že cesty, domovský adresář, časová razítka, id tasků a session, hashe záznamů a doby trvání se nahradí zástupnými hodnotami a nic, co se týká chování, ne. Dva běhy jednoho scénáře na jednom nástroji jsou bajt po bajtu shodné.

Generátor webu tyto lokální důkazy nečte. Každý scénář spustí sám, stejným příkazem, a výsledek vloží do `site/data/generated/catalogue.json`, a CI odmítne data webu, která se liší od čisté regenerace. Stránka, která ukazuje, co příkaz vypsal, proto ukazuje přesně to a vložený přepis nemá kam jít.

## Zralost se pozoruje

Dvě pole píše autor: stav (aktivní, koncept nebo zastaralý) a cíl (zaručený, nebo doporučující). To, čím případ užití je, se spočítá při generování dat webu. Je `described`, když nemá scénář, `executable`, když ho má, ale v této generaci k němu nejsou úspěšné důkazy, `verified`, když scénář prošel, a `guaranteed` jen tehdy, když je ověřený, každé tvrzení, které jmenuje, je zaručené a každé pravidlo, které jmenuje, je vynucené. Nikdo do případu užití `guaranteed` nenapíše.

## Pokrytí, které může politika vyžadovat

`majordomus usecase coverage` bere každý veřejný příkaz, každé zaručené tvrzení s odpovědností a každý nástroj MCP jako cíl. Spočítá aktivní případy užití, které cíl jmenují, ty, jejichž scénář ho spouští, a ty s úspěšnými důkazy, a cíl označí jako pokrytý, částečný nebo jako mezeru. Co mezera znamená, říká politika pro každou třídu zvlášť. Tento repozitář vyžaduje každý veřejný příkaz, tvrzení a nástroje MCP nechává jako doporučující a blokující pravidlo [majordomus.use-case-coverage](@/rules/majordomus-use-case-coverage-1.md) spouští přehled pod `doctor`, `check` i `finish`. Vyžadovaná mezera odmítne dokončení a pojmenuje schopnost i příkaz, který pro ni připraví koncept.

Koncept, který zapíše `majordomus usecase scaffold --missing`, projde validací i spuštěním a nepokrývá nic, dokud člověk nebo agent nedopíše vyprávění, nezpřísní tvrzení a neudělá z něj aktivní případ užití. Kostra nepotřebuje žádný model a manuál výslovně říká, že nic, co napíše model, není důkaz.

## Jak to ověřit

`majordomus usecase run` spustí každý scénář nad fixture, `majordomus usecase run <id> --keep` ponechá repozitář k nahlédnutí a `majordomus usecase show <id>` vypíše soubor a jeho poslední důkazy. `majordomus usecase coverage --check` uplatní politiku a `majordomus usecase impact` podle souborů, kterých se změna dotkla, pojmenuje příkazy, pravidla, případy užití a behaviorální případy, které je třeba spustit. Záruky [use-case-coverage](@/guarantees/use-case-coverage.md), [use-case-evidence](@/guarantees/use-case-evidence.md) a [use-case-impact](@/guarantees/use-case-impact.md) jmenují jako svůj důkaz `test/cases/94_use_cases.sh`.

## Co tento článek netvrdí

Netvrdí, že každé chování nástroje má případ užití. Politika vyžaduje každý veřejný příkaz a u tvrzení a nástrojů MCP mezery jen hlásí. Výsledek živého scénáře také není záruka: popisuje strom, ve kterém běžel. ADR 0038 je navržené a ADR 0005 přijaté; příkazy, generátor a pravidlo, které článek popisuje, jsou na masteru a běží. Jak repozitář odmítá vlastní dokumentaci, popisuje [Dokumentace, kterou lze odmítnout](@/dokumentace-kterou-lze-odmitnout.md), jak se pravidlo váže k případu, který může odmítnout jeho porušení, vysvětluje [Pravidlo se dokazuje odmítnutím](@/pravidlo-se-dokazuje-odmitnutim.md) a proč odvozená stránka dá pokaždé stejné bajty, ukazuje [Napsáno jednou, odvozeno všude](@/napsano-jednou-odvozeno-vsude.md).
