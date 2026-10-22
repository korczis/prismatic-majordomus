+++
title = "Napsáno jednou, odvozeno všude"
description = "Capability tohoto repozitáře je deklarovaná na jednom místě a do příkazové řádky, MCP, HTTP, OpenAPI i na tento web se dostává jako projekce té deklarace. Projekce jsou commitnuté, znovu generované, nikdy ručně upravované, a commit, který je nechá zaostat, je odmítnut. Proč, jak a co to stojí."
template = "page.html"
[extra]
lang = "cs"
translated_from = "written-once-derived-everywhere/"
source_digest = "b85d9b164fd3ba86d1c1ee856de9fcde101c4d5a660b232d0218de2f1e8c9649"
related = ["dokumentace-kterou-lze-odmitnout/", "plan-se-napadne-driv-nez-se-postavi/", "teze/", "zabrano-driv-nez-se-stavi/", "jedna-vetev-jedno-misto/", "automatizace-je-schopnost/", "znalost-odvodi-nastroj-povysi-clovek/", "pripad-uziti-se-spousti-ne-popisuje/", "jeden-zdroj-kazdy-jazyk/"]
[extra.translations]
en = "written-once-derived-everywhere/"
+++

Nástroj, který nabízí tutéž operaci na několika plochách, ji obvykle popisuje několikrát. Příkazová řádka má svou nápovědu, HTTP API svou routu, dokument OpenAPI své schéma, MCP server svůj seznam nástrojů a dokumentace svou vlastní stránku. Každý popis píše někdo ručně a každý se od ostatních vzdaluje vlastním tempem. Jako první si toho obvykle všimne uživatel, který se řídil dokumentací.

Tenhle repozitář se rozhodl, že fakt se napíše jednou a všechno ostatní se z něj odvodí ([ADR 0004](@/adrs/adr-0004.md), přijaté). Capability se deklaruje na jednom místě ve spustitelném programu v Rustu. Její příkaz, nástroj MCP, HTTP routa, operace v OpenAPI i její stránka na tomto webu jsou projekce té deklarace. Bootstrap, který čte každý agent, to říká rovnou. Operace se nikdy nepřidává ani nemění úpravou registru transportu, definice OpenAPI nebo tabulky v dokumentaci: změní se kanonická deklarace a projekce se vygenerují znovu. Sémantická definice opakovaná napříč projekcemi se tu nazývá chybou návrhu, ne pohodlím.

## Projekce jsou commitnuté

Generované soubory se často do repozitáře nedávají a vytvářejí se až při buildu. Tady se commitují, a to záměrně. Recenzent, session na jiném stroji nebo tento web si projekci přečte, aniž by cokoli buildoval, a změna v diffu přesně ukáže, co udělá s každou plochou. Pull request, který přidá do odpovědi jedno pole, ukáže i to, jak se pole objeví v dokumentu OpenAPI, v registru a v datech webu.

Commitování generovaných souborů má jedno známé nebezpečí: zastarávají. Pravidlo [project.derived-files-regenerated](@/rules/project-derived-files-regenerated-1.md) na to odpovídá přímo: generovaný soubor se mění změnou jeho kanonického zdroje a novým spuštěním generátoru, oboje se commituje spolu a otisková brána commit, který to nedodrží, odmítne. Vynucuje to commit hook přes `scripts/pages current`. Ten skript porovná otisk vstupů zaznamenaný u odvozených dat s otiskem commitovaného stromu, což trvá zlomek sekundy. Spustí také vlastní `generate --check` spustitelného programu, takže projekce registru porovnává tentýž kód, který je píše. Commit, jehož odvozená data zaostávají za vstupy, je odmítnut dřív, než vůbec vznikne.

## Jedno odvození, popořadě

Projekce vyrábí jediný skript, `scripts/derive`, který celý graf rozepisuje do fází:

1. `majordomus generate` zapíše, co závisí jen na kódu: dokumenty OpenAPI a registru, referenci příkazů, stránky capabilities a modulů, allow-listy a bootstrapy providerů.
2. `scripts/generate-site-data` zapíše data a obsah webu a odvozené dokumenty z kanonických souborů a z výstupu první fáze.
3. `majordomus generate` proběhne znovu nad indexem, protože dokumenty, které zapsala druhá fáze, jsou objekty, které index čte: dataset registru pro web, matici benchmarků a manifest všech artefaktů.
4. `scripts/gitattributes` zapíše z generátorů politiku mergování.

Žádná fáze nečte vlastní výstup a dvojí spuštění celého odvození nic nezmění. Právě s tím druhým během porovnává `scripts/derive-check` a průběžná integrace strom, který tahle kontrola odmítne, odmítne taky.

## Jedno po druhém a nic za zády

Odvození není zadarmo. Sestaví nebo znovu použije spustitelný program, přečte celý repozitář, zapíše mnoho souborů a trvá minuty, ne sekundy. Několik sessions na jednom stroji by jinak odvozovalo současně a navzájem se dusilo. Pravidlo [project.one-derive-at-a-time](@/rules/project-one-derive-at-a-time-1.md) z toho dělá frontu. `scripts/derive` drží zámek stroje po celý svůj běh, čeká na živého držitele, ustoupí prioritní značce, která jmenuje jinou větev, a když ho někdo zastaví, vezme s sebou celý strom svých procesů.

Druhé pravidlo zavírá tišší mezeru. Odvození promítá jen to, co sleduje git, takže nový soubor, který nikdo nepřidal, by pro ně byl neviditelný, a commit, který ho přidá, by pak byl odmítnut kvůli odvozeným datům, která nesedí. `scripts/derive` proto nesledovaný a neignorovaný soubor odmítne dřív, než začne čekat na zámek, a jmenuje ho.

## Mergování odvozených souborů

Dvě větve, které obě znovu vygenerují tentýž odvozený soubor, se střetnou na místech, která by žádný člověk neměl řešit ručně. Repozitář pro odvozené cesty deklaruje merge driver. Ten takový konflikt vyřeší ve prospěch jedné strany, řekne, že výsledek je teď zastaralý, a skutečné vyřešení nechá na příštím odvození. Merge commit se pak odvodí jednou, nad sloučenými zdroji, místo aby se řádek po řádku sešíval ze dvou zastaralých výstupů.

## Co to stojí

Cena je skutečná. Každá změna deklarace stojí jedno odvození, odvození na vytíženém stroji čeká, až na něj přijde řada, a generované soubory prodlužují diffy. Session, která zapomene přidat nový soubor, je odmítnuta, a ta, která ručně upraví generovaný soubor, je odmítnuta znovu. Odpověď repozitáře zní, že každé z těch odmítnutí je levnější než alternativa, kterou je zveřejněná plocha tvrdící něco, co kód nedělá.

Architekturu popisuje [ADR 0004](@/adrs/adr-0004.md). Model capabilities popisuje [manuál capabilities](@/docs/capabilities.md) a cestu od zdrojů až na tento web [architektura publikovaného webu](@/docs/github-pages-architecture.md).
