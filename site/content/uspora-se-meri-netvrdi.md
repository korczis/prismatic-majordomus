+++
title = "Úspora se měří, netvrdí"
description = "Méně tokenů na task je hodnota, kterou podle nejčastějšího předpokladu přináší Majordomus, a procento ušetřených tokenů je to nejsnazší, co si projekt jako on může vymyslet. Jak repozitář na tu otázku odpovídá spárovanými běhy posuzovanými skrytými testy, u každého čísla uvádí, jak vzniklo, a nezveřejní žádnou úsporu, dokud to nedovolí pravidlo deklarované dřív než důkazy."
template = "page.html"
[extra]
lang = "cs"
translated_from = "a-saving-is-measured-not-claimed/"
source_digest = "5d707131b94dc156fef348a33451037489669c9853c1736db887eee33c712aac"
related = ["mlceni-neni-verdikt/", "dokumentace-kterou-lze-odmitnout/", "pochybnost-rozhodne-experiment/"]
[extra.translations]
en = "a-saving-is-measured-not-claimed/"
+++
{% raw %}

Méně tokenů na task je hodnota, kterou podle nejčastějšího předpokladu přináší Majordomus. [ADR 0082](@/adrs/adr-0082.md) (navržené), datované 24. září 2026, vychází z toho, že ji nic nezměřilo: dokument o ekonomice od své první verze odmítal uvádět jakoukoli úsporu tokenů. Podle záznamu je tento repozitář tu právě od toho, aby takovou domněnku proměnil v ověřený fakt, nebo ji zavrhl.

Procento ušetřených tokenů je podle vlastní stránky tohoto tvrzení tím nejcitovatelnějším, co by projekt mohl zveřejnit, a zároveň tím, co se vymýšlí nejsnáz. Každý dokument o směrování (routing), který se při návrhu nástroje studoval, uváděl čísla o zlepšení, za kterými nestálo žádné měření. ADR odpovídá jediným kalkulátorem, který každé číslo odvodí ze zaznamenaných běhů, u každého čísla uvede, jak vzniklo, a nezveřejní žádnou úsporu, dokud to nedovolí pravidlo deklarované dřív než důkazy.

## Stejný task, dvě ramena

Otázka je ta, kterou klade [`docs/ECONOMICS.md`](@/docs/economics.md): kolik tokenů spotřebuje programovací session u stejného tasku, ze stejného stavu repozitáře, se stejným modelem, nástroji a akceptačními testy, když je Majordomus nainstalovaný, a kolik, když není? Jednotkou jsou tokeny na úspěšně dokončený task. Běh, který ušetří tokeny tím, že selže, není úspora, a pár se proto počítá, jen když oba běhy splnily každou podmínku úspěchu.

Kontrolním ramenem (control) je testovací repozitář (fixture) v podobě, v jaké ho udržuje schopný tým: README, krátký `CLAUDE.md`, konvence a záznamy rozhodnutí v `docs/`, zelená sada testů a Claude Code spuštěný bez interakce (headless), bez serveru MCP, uživatelského nastavení a skills. Experimentálním ramenem (treatment) je týž repozitář po `majordomus init`, `majordomus update` a `majordomus capture install --provider claude-code`, commitnutý, se vším ostatním beze změny. Server MCP a hooky gitu jsou z tohoto prvního experimentálního ramene vynechané a jejich měření by bylo novým experimentálním ramenem.

## Korpus, který lze nesplnit

Tasky v `.ai/repo/benchmarks/economics/tasks/` zastupují každý jeden vzorec vývoje, se kterým se tento repozitář setkává, od chyby, jejíž oprava je zapsaná v záznamu rozhodnutí, až po funkci budovanou napříč dvěma sessions. Každý jmenuje svůj testovací repozitář, ověřovací příkaz, referenční řešení a skryté akceptační testy, které se do něj nakopírují až po poslední session. Běh uspěje, když projde vlastní sada testů repozitáře, projdou skryté testy a nebyl smazán žádný testovací soubor, se kterým repozitář začínal.

`majordomus economics references` bez účasti modelu dokazuje, že každý skrytý test na výchozím stavu tasku selže a na jeho referenčním řešení projde, takže neúspěšný běh vypovídá o agentovi, ne o testu. První živá sada, `pilot`, spouští každý task v obou ramenech a je záměrně malá: její deklarace podle pravidla zveřejnění nazývá její výsledky předběžnými.

## Dvě měření, nikdy sloučená

Každé číslo nese jednu z pěti tříd. Pozorované (observed) je to, co poskytovatel nebo jeho běhové prostředí (harness) nahlásil při běhu, beze změny; spočtené (counted) se počítá z bajtů na disku pojmenovaným tokenizérem v pevně dané verzi; odvozené (derived) je aritmetika nad oběma a nikdy není silnější než jeho nejslabší vstup. Odhadnuté (estimated) zastupuje číslo, které nikdo nezměřil, a nikdy se nezveřejňuje, a kontrafaktuální (counterfactual), tedy to, čemu by nějaký mechanismus předešel, je modelované a nikdy není úsporou.

Dvě měření odpovídají na různé otázky. Snížení celkového počtu tokenů, `effective_token_reduction`, je odvozené z pozorované spotřeby ve spárovaných živých bězích a je to jediný druh čísla, který by mohl podložit tvrzení o tom, co Majordomus ušetří. Výběr kontextu, `context_reduction_ratio`, je odvozený ze spočtených tokenů: co kompilátor kontextu vybral pro každé issue plánu tohoto repozitáře z toho, co vyhodnotil jako relevantní, bez účasti modelu. Není to úspora a obě měření se nikdy nesloučí; počet uvádí svůj tokenizér, `o200k_base`, který nepoužívá každý model, a nikdy se neporovnává s tokeny, které nahlásil poskytovatel.

Pro každý platný pár platí `reduction = 1 - treatment / control` nad celkovými tokeny: kladná hodnota znamená, že Majordomus spotřeboval méně tokenů, záporná, že víc, a žádné rozhraní znaménko neotočí, protože ADR vyžaduje, aby se task, na kterém Majordomus stojí víc, ukázal, a ne zprůměroval. Napříč páry kalkulátor uvádí rozdělení a percentilový bootstrapový interval mediánu se zaznamenaným seedem, který převzorkovává celé tasky, protože opakování jednoho tasku nejsou nezávislá.

## Pravidlo přichází před důkazy

Metodika ještě před jakýmikoli důkazy deklaruje nejmenší počet platných párů, kategorií tasků s dostatkem párů v každé z nich a opakování každého tasku v každém rameni, nejnižší podíl platných párů a nejširší interval, který přijme, a vyžaduje aktuální důkazy z čistého stromu. Tvrzení o celkovém počtu tokenů je dovolené, jen když platí každá hranice, a jen pro primární metriku nad všemi důkazy, bez filtru. Pod hranicí pravidla každé rozhraní uvádí, že žádné ověřené tvrzení o úspoře celkového počtu tokenů není k dispozici, a vedle toho předběžné pozorování označené jako předběžné.

Žádný běh se nevyřadí, pokud ho metodika nejmenuje i s důvodem, a primární metrika se pak ukazuje s ním i bez něj. Zaznamenaný běh se nikdy nepřepíše, `--force` se odmítne a pár, který není platný, se hlásí i s důvodem, nikdy se tiše nezahodí.

To, co se dostane ke čtenáři, hlídá `majordomus economics check`, který CI spouští v každém plánu jako strukturní gate `economics-claims`. Odmítne procento nebo násobek napsaný do ručně psané prózy ve stejné větě jako slovo ekonomického slovníku a navázané tvrzení, které má stav `guaranteed`, zatímco důkazy jeho metriky neobstojí. Čte text, ne význam, takže tvrzení formulované bez vyčíslení projde.

## Důkazy zastarávají

Každý záznam nese otisk souborů, na kterých jeho sada závisí, u živé sady metodiky, korpusu, testovacího repozitáře a těch částí Majordomus, které určují, co session v experimentálním rameni dostane. Otisk se přepočítává při každém čtení, a když se kterýkoli z těch souborů změnil, důkazy se hlásí jako zastaralé a tvrzení na ně navázané ztrácí záruku, dokud se sada znovu nespustí. Důkazy zaznamenané pod jinou verzí metodiky jsou nekompatibilní a nikdy se s ostatními neslučují.

## Co platí a co se neměří

Deterministická sada pro kontext byla poprvé zaznamenána 26. září 2026; její hodnota je na stránce [Token economics](@/economics.md) se stavem measured a třídou derived from counted. Když tento článek vznikal, v říjnu 2026, nebyl zaznamenaný žádný živý běh, a vygenerovaný verdikt proto zněl, že žádné ověřené tvrzení o úspoře celkového počtu tokenů není k dispozici; táž stránka ukazuje verdikt, jak stojí teď. Hypotézy, které měl benchmark otestovat, byly zapsány dřív než jakékoli důkazy a vedou se jako neotestované; jedna z nich říká, že krátké izolované tasky získají méně a mohou stát víc.

Čtenář si to všechno může zopakovat. `majordomus economics references` a `majordomus economics measure` nepotřebují žádný model, `majordomus economics run --suite pilot` potřebuje přihlášení do Claude Code a čerpá spotřebu u poskytovatele, `majordomus economics summary` přepočítá každé číslo ze zaznamenaných běhů a `majordomus economics explain effective_token_reduction` ukáže, na čem jedna metrika stojí. Tytéž odpovědi poskytuje `GET /api/v1/economics`, nástroje MCP `majordomus_economics*` i stránka Economics v Cockpitu, všechno projekce jediného kalkulátoru.

Testovací repozitář je záměrně malý, takže velký repozitář, kde na výběru kontextu záleží nejvíc, se neměří; neměří se ani jiná běhová prostředí a modely, server MCP, koordinace mezi paralelními workery ani vliv kontraktu dokončení (finish) na přepracování. Náklady jsou vlastním odhadem běhového prostředí v době běhu, ne fakturou. Experimentální rameno je Majordomus tak, jak ho dostane uživatel, který si nainstaluje jen shellový nástroj, takže to, co session utratí za příkazy, které jí bootstrap doporučuje, ale tato instalace je spustit nedokáže, se měří a nekoriguje.

ADR 0082 je navržené; tvrzení, že se výběr kontextu počítá a že se nepodložené číslo o úspoře odmítá, jsou zaručená a tvrzení o změřené změně celkového počtu tokenů je plánované. Proč kontrola, která se nikdy neozvala, není úspěch, popisuje [Mlčení není verdikt](@/mlceni-neni-verdikt.md), jak repozitář odmítá vlastní dokumentaci, vysvětluje [Dokumentace, kterou lze odmítnout](@/dokumentace-kterou-lze-odmitnout.md), jak se neshoda rozhoduje experimentem místo počítání, ukazuje [Pochybnost rozhodne experiment](@/pochybnost-rozhodne-experiment.md) a metodu popisuje [`docs/ECONOMICS.md`](@/docs/economics.md).
{% endraw %}
