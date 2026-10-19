+++
title = "Vložené zadání se stává plánem"
description = "Zadání vložené do session je přání k jednomu dni, ne pokyn. Jak ho tenhle repozitář uchová přesně tak, jak přišlo, jak z toho, co chce, udělá záznamy, které může gate odmítnout, a co ten mechanismus ještě musí dokázat."
template = "page.html"
[extra]
lang = "cs"
related = ["dokumentace-kterou-lze-odmitnout/", "teze/", "vsechno-v-jednom-proudu/"]
[extra.translations]
en = "a-pasted-brief-becomes-a-plan/"
+++

Většina práce v tomhle repozitáři začíná stejně. Něco vložím do session. Někdy je to prompt pack: adresář očíslovaných markdownů, napsaný na jeden zátah, co chci postavit a v jakém pořadí. Jindy je to report: review webu nebo plánu od jiného modelu, se seznamem toho, co je podle něj špatně. Často česky, protože česky přemýšlím.

Nejsnazší, co s takovým zadáním agent může udělat, je vzít ho jako pokyny a začít stavět. A přesně to je chyba. Zadání je přání ke dni, kdy vzniklo, a psal ho někdo, kdo se ten den do stromu nedíval. Půlka toho, co žádá, už možná existuje. Něco z toho odporuje rozhodnutí, které repozitář má zapsané. A jakmile podle něj session jednou pojede, samotné zadání většinou zmizí — leželo ve stažených souborech nebo v chatu a s ním zmizí i důvod, proč nějaký subsystém vůbec existuje.

Tenhle článek proto odpovídá na úzkou otázku: co se děje mezi vložením a prvním commitem a které z těch kroků umí gate skutečně odmítnout. K zadáním z 18. a 19. října 2026 se vrátím na konci, protože práce naplánovaná podle nich na master ještě nedorazila, a i to patří k poctivé odpovědi.

## Nejdřív se zadání uloží tak, jak přišlo

Zadání, podle kterého se pracovalo, je doklad o původu, a tak se ukládá bajt po bajtu. Konvenci popisuje `campaigns/README.md`: každý pack od operátora se do `campaigns/` zkopíruje beze změny a `campaigns/MANIFEST.sha256` zaznamená bajty každého souboru. Tentýž README tučně říká, že jde o historii, ne o pokyny. Nic je nenačítá, nic je nevaliduje jako dokumenty vrstvy, a když se pack s repozitářem neshodnou, platí repozitář.

Stojí za tím pravidlo [a campaign brief this repository keeps is the one it was handed](@/rules/project-a-brief-is-kept-as-received-1.md) a `scripts/ci/campaigns-check` o něm rozhoduje tři věci: manifest pokrývá přesně ty soubory, které jsou přítomné, i s jejich bajty, a to oběma směry; každý adresář packu má v README svůj řádek; a každý řádek jmenuje adresář, který existuje. O tom, co pack *říká*, nerozhoduje nic. Pack dodatečně upravený tak, aby souhlasil s tím, co se nakonec postavilo, by se pořád četl jako zadání, podle kterého se pracovalo — a právě takovou úpravu má kontrola zachytit.

Zadání psané česky zůstává česky. Repozitář píše anglicky, jenže přepsat zadání by znamenalo zfalšovat záznam. Soubor českého packu je proto v `scripts/ci/english-only-allow.txt` deklarovaný jako fixture s tímhle zdůvodněním, místo aby se překládal.

## Pak věta o tom, co má platit

Se zadáním samotným repozitář uvažovat neumí. Prvním záznamem, se kterým už ano, je intent: typovaná věta o tom, co se má stát pravdou, zatím bez plánu.

Přesně tohle zapisuje `intents.propose`. Na vstupu dostane větu a volitelně titulek a id. Zapíše jeden nový soubor pod `.ai/repo/project/intents/` s větou doslovně a s prázdnými milníky, kritérii i invarianty, odmítne přepsat existující záznam a soubor na místo připojí tak, že se objeví celý, nebo vůbec. Takový záznam má odvozenou fázi `declared` a žádné issue mu nemůže sloužit: není milník, pod který by se práce dala zařadit, ani kritérium, kterému by sloužila, takže preflight práci, která se na něj odvolává, odmítne. Rozhodnutí je [ADR 0144](@/adrs/adr-0144.md), pořád ve stavu `proposed` — že se na něm smí stavět ještě před přijetím, je moje rozhodnutí — a drží ho test case 2978.

ADR odmítá lákavou alternativu: ať capability rovnou zapíše zástupné milníky a kritéria, aby záznam prošel validací hned první den. Do plánu by se tím dostala vymyšlená práce a nikdo by později nepoznal zástupný údaj od rozhodnutí.

Jeden důsledek mi nedošel předem. Věta se zapisuje doslovně a jazykový gate čte každý YAML, který někdo napsal. Česky vložené zadání se tedy větou intentu stát nemůže tak, jak je. Někdo — člověk nebo model — musí anglicky říct, co se má stát pravdou, a právě v tomhle přeformulování se může význam zadání poprvé posunout. Ještě se k tomu dostanu.

## Až plánování dělá ze zadání něco odmítnutelného

Naplánovat návrh je samostatný krok a právě v něm je většina hodnoty. Intent se plánuje třemi druhy záznamů:

- **Kritéria s důkazem.** Každé kritérium splnění je jedna věta, kterou rozhodne jmenovaný důkaz, typicky testovací soubor nebo case.
- **Milníky a issues.** Issue patří pod milník a v `serves` jmenuje kritérium, kvůli kterému existuje.
- **Závislosti.** Issue uvádí, na čem závisí (`depends_on`), a vlny provádění se z toho grafu odvozují: nultá vlna jsou všechna issues bez závislostí a každé další leží o vrstvu za svou nejhlubší závislostí. Vlny se počítají při každém čtení a nikam se neukládají. Dvě issues ze stejné vlny, jejichž scope se překrývá, se seřadí za sebe, protože falešná souběžnost stojí konflikt objevený až po práci.

`majordomus intent validate` posuzuje tvar takového plánu. Jakmile má intent nějakou práci, odmítne kritérium, kterému neslouží žádné živé issue; intent, který zatím žádnou práci nemá, dostane místo toho jedno varování. Odmítne i issue pod milníkem intentu, které neslouží ničemu. Práce pod milníkem, který žádný intent nejmenuje, se hlásí jako údržba a nikdy jí není přiřazen vymyšlený účel. Rozhodnutí je ADR 0073, rovněž zatím `proposed`; model plánování popisuje stránka [How planning works](@/docs/planning.md).

V tomhle kroku se z věty „web by měl působit živěji“ stane něco, čemu může gate říct ne. Ne proto, že by gate tomu přání rozuměl, ale protože každý kus práce teď musí ukazovat na větu a každá věta na test.

## Před prací přichází kritika

Než se na intentu začne pracovat, projde plán oponentním čtením, které se zapíše pod `.ai/repo/project/critiques/`. Každý nález je zařazený podle otázky, kterou klade — chybějící požadavek, nedoložený předpoklad, nedostatečná nebo zbytečná práce, riziko regrese, povrch, který se musí změnit taky, ověření, které dodávka potřebuje — je buď blokující, nebo ne, a je vyřešený jako otevřený, naplánovaný do issue, které intentu slouží, nebo zamítnutý s důvodem.

Odmítnutí je konkrétní: issue sloužící intentu, které je aktivní, v ověřování nebo hotové, zatímco intent nemá žádnou kritiku nebo má otevřený blokující nález, `intent validate` neprojde. Přes sestavenou binárku to drží case 386.

Stejně konkrétní je i hranice. Kritika jmenuje svého recenzenta v poli `reviewed_by` a validace ho čte jako text. Dvě z kritik na masteru otevřeně říkají, že jde o self-review: každou napsala tatáž session, jež napsala posuzovaná issues, a kritika má větší váhu, když ji napíše jiná session nebo já. Je dobře, že to říkají, místo aby to skrývaly. Jenže nezávislost je tím pádem věta v záznamu, ne vlastnost, o které by cokoli rozhodovalo.

## Nejistota se promýšlí, nehádá

Zadání často chce něco, kde správný návrh není zřejmý. Na to je `majordomus reasoning` a příručkou je [Reasoning](@/docs/reasoning.md). Session zapíše posouzení: předmět, jak podstatná ta nejistota je, hypotézu a důkazy za ní. Posouzení označené jako material, high nebo critical bez odkazů na důkazy neprojde. Politika pak rozhodne, jestli se nezávislá revize vyplatí; `scripts/advisor-consult` se zeptá těch poradců, které vybere, a jejich odpovědi jsou důkazy, které session váží, nikdy verdikty. Neshoda se rozhoduje experimentem, ne sčítáním hlasů, a závěr se zapíše, aby si ho další session přečetla místo toho, aby se ptala znovu.

Poradci jsou volitelní. Checkout, kde žádný není, uvažuje a rozhoduje úplně stejně a řekne, že to udělal lokálně. Rozhodnutí je [ADR 0098](@/adrs/adr-0098.md), i ono zatím `proposed`.

## Práce se nabízí a nárokuje, ne bere

Jakmile issues existují, může o ně stát víc sessions na víc strojích. Rozhoduje se o tom v meshi, který popisuje [Majordomus Mesh](@/docs/mesh.md). Handover publikovaný na jednom stroji se nabídne ostatním a převezme se na jiném. Claim je scope cest, který drží session, a ve výchozím stavu je exkluzivní: `mesh.claim` odmítne exkluzivní claim, který se potká s živým exkluzivním claimem jiné session, a jmenuje ty, se kterými se potkal. Když dva runtimy nárokují stejný scope, aniž by o sobě věděly, dostanou se do journalu oba a fold na každém runtimu jmenuje téhož jediného vítěze. Claim platí, dokud jeho držitel tepe; runtime, který zmizí, přestane sám od sebe držet cokoli, a to všude.

U zadání to hraje roli zvlášť. Vložené zadání mívá sklon skončit ve víc sessions najednou, a dvě sessions, které za jedno odpoledne postaví stejný subsystém, tenhle repozitář už zažil.

## Gate na konci cesty

Mezi session a pushem stojí jako poslední gates. Pro tohle téma je nejdůležitější `scripts/ci/english-only-check`, strukturální gate, který CI pouští při každé změně; pre-push hook pouští strukturální gates plánu ještě před pushem, v časovém limitu, který sám uvádí. Odmítne napsaný soubor s písmenem, které se vyskytuje v češtině, slovenštině nebo polštině a nikdy v angličtině, nebo s celým jiným písmem. Co rozhoduje a co ne, popisuje [stránka té záruky](@/guarantees/authored-files-are-english.md).

Kontrola je ve vlastní hlavičce poctivá i ke své hranici: testuje abecedu, ne jazyk. Česká věta napsaná bez diakritiky projde a vždycky projde. Záznamy sessions jsou vyřazené záměrně, protože citují, co zaznělo, a to, co zaznělo, není vždycky anglicky. Gate tedy udrží mimo strom běžný případ — český odstavec vložený do záznamu plánu — ale nepozná, jestli plán anglicky říká totéž, co zadání česky.

## Co se stalo 18. a 19. října

Práce naplánovaná podle toho, co ode mě v ty dva dny přišlo, leží na větvi, která **na master zatím nedorazila**. Část jejího zdroje nebyl pack vložený do session, ale konverzace z projektu v ChatGPT, přečtené přes moji vlastní přihlášenou session, a jejich surový záznam je záměrně uložený mimo všechny checkouty, ne pod `campaigns/`. Proto tenhle článek nedává žádný odkaz na ty záznamy, žádné číslo z nich a žádné tvrzení o tom, kterými kroky téhle cesty prošly nebo co z nich vzešlo. Dokud nedorazí, web je ukázat neumí a gates na masteru je nesoudily.

README kampaní už říká, co takový stav znamená: kde plán ještě není na masteru, nese ho větev. Znamená to ale i tohle: session, která si další práci vybírá podle masteru, ten plán vůbec nevidí a může naplánovat totéž znovu. Dostat plán na master je součást plánování, ne dodatek.

## Co tohle dokazuje a co zatím ne

| Krok | Záznam | Co o něm rozhoduje | O čem nerozhoduje |
|---|---|---|---|
| Uložit zadání | `campaigns/`, `MANIFEST.sha256` | `scripts/ci/campaigns-check` | jestli cokoli, co zadání chtělo, existuje |
| Vyslovit intent | `.ai/repo/project/intents/` | `intents.propose`, case 2978 | jestli věta říká, co zadání myslelo |
| Naplánovat | kritéria, milníky, issues, `serves`, `depends_on` | `majordomus intent validate` | jestli je kritérium splněné — to je otázka důkazu |
| Podrobit kritice | `.ai/repo/project/critiques/` | `intent validate`, case 386 | jestli je recenzent nezávislý na autorovi |
| Promyslet | záznamy `majordomus reasoning` | požadavek na důkazy u podstatného posouzení | jestli je závěr správný |
| Rozdělit práci | claimy a handovery v meshi | `mesh.claim` a fold journalu | jestli ten, kdo nárokuje, dotáhne práci |
| Pushnout | napsaný strom | `scripts/ci/english-only-check` | jazyk, na rozdíl od abecedy |

Čtyři věci jsou pořád dlužné, a radši je pojmenuju, než nechat tabulku naznačovat něco jiného.

Rozhodnutí, na kterých tahle cesta stojí — ADR 0144, ADR 0073 a ADR 0098 — jsou navržená, ne přijatá. Stavět na navrženém rozhodnutí se tady smí a kód i jeho testy existují; přijetí je ale moje rozhodnutí a to zatím nepadlo.

Přeformulování ze zadání na intent nikdo nekontroluje. Pack je uložený doslovně a intent je validovaný, jenže nic ty dva neporovnává. Když anglická věta potichu zúží to, co české zadání chtělo, všechny gates za ní zůstanou zelené.

Nezávislost kritiky je deklarovaná, ne rozhodnutá. Záznam říká prózou, kdo recenzoval, a víc než to, že se self-reviews na masteru jako self-review samy označují, se zatím říct nedá — a i to platí jen z konvence, ne proto, že by to cokoli kontrolovalo.

A práce naplánovaná podle zadání z 18. a 19. října musí dorazit na master. Dokud tam její záznamy nejsou a gates masteru přes ně neprošly, popisuje tenhle článek mechanismus, ne to, co z těch zadání udělal. Pokud mechanismus obstojí, pokračováním by měl být odkaz, ne odstavec.

Stejnou myšlenku, jen použitou na dokumentaci místo plánů, rozebírá článek [Dokumentace, kterou lze odmítnout](@/dokumentace-kterou-lze-odmitnout.md).
