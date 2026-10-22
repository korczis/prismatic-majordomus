+++
title = "Pravidlo se dokazuje odmítnutím"
description = "Blokující pravidlo říká, že gate odmítne práci, která ho porušuje, a to je tvrzení o světě. Co dnes Majordomus přijímá jako jeho důkaz, tedy případ, který je pojmenovaný, leží ve stromu a jehož běh se zaznamenal, kde ten důkaz končí a jaké odmítnutí po něm žádá plán."
template = "page.html"
[extra]
lang = "cs"
translated_from = "a-rule-is-proven-by-what-it-refuses/"
source_digest = "59b8bc6f16bf2a60cf9829fec1a5c214c3f6cad6c8f40c38809a75ae51ac5a8d"
related = ["dokumentace-kterou-lze-odmitnout/", "mlceni-neni-verdikt/", "automatizace-je-schopnost/", "znalost-odvodi-nastroj-povysi-clovek/", "pripad-uziti-se-spousti-ne-popisuje/"]
[extra.translations]
en = "a-rule-is-proven-by-what-it-refuses/"
+++

Pravidlo tohoto repozitáře je verzovaný dokument, jehož front matter nese id, znění a třídu. Třída má dvě hodnoty. `blocking` znamená, že gate odmítne práci, která pravidlo porušuje, a `advisory`, že se od čtenáře čeká, že ho četl. [ADR 0048](@/adrs/adr-0048.md) (navržené) z toho vyvozuje důsledek: blokující třída je tvrzení o světě a buď platí, nebo ne. Tento článek sleduje, co repozitář přijímá jako důkaz takového tvrzení, kde ten důkaz končí a co po něm žádá plán.

## Odznak za klíč YAML

Do 11. září 2026 to nic nerozhodovalo. Každé rozhraní kladlo jedinou otázku, zda pravidlo nese blok `x-majordomus`, a podle odpovědi vykreslilo odznak; jeden řádek `product.rs` zněl `enforced: o.metadata.get("x-majordomus").is_some()`. Pravidlo mohlo v oddílu psaném pro člověka jmenovat `test/cases/28_no_hardcoded_values.sh`, ten případ mohl dostat jiné jméno a pravidlo se dál všude četlo jako vynucené. Modul pravidel ve spustitelném programu to dnes nazývá tím, čím to bylo: zelený odznak za přítomnost klíče YAML.

## Tři způsoby, jak říct, kdo pravidlo vynucuje

ADR 0048 dovoluje pravidlu deklarovat jeden ze tří režimů a deklarací je tvar jeho bloku vynucení, takže slovo režimu nejde napsat špatně. Pravidlo v režimu `dispatched` jmenuje `validator`, kterého volá `lib/doctrine.sh` u příkazů, jež jmenuje jeho `enforced_by`. Pravidlo v režimu `gated` jmenuje `tests` a žádný validátor a spouští je gate v CI nebo behaviorální sada. Pravidlo v režimu `reviewed` jmenuje `reviewed_because`, tedy důvod, proč ho nic spustitelného nedokáže vyjádřit, a vynucuje ho člověk.

Důvod je celá deklarace a vedle něj záměrně není žádný příznak: příznak lze nastavit, ale důvod nelze nastavit, aniž by ho někdo napsal. [project.clean-room](@/rules/project-clean-room-1.md) je blokující a v režimu `reviewed`, protože se týká původu toho, co bylo napsáno, a ten žádný program ve stromu přečíst neumí. Spustitelný důkaz vždy vyhrává: validátor udělá z pravidla `dispatched`, test z něj udělá `gated` a teprve potom se čte důvod. Pravidlo v režimu `reviewed` se na každém rozhraní počítá zvlášť, nikdy jako úspěch a nikdy jako selhání, protože když to číslo roste, správa pravidel slábne.

## Co rozhoduje gate

`scripts/ci/rule-proof-check` čte každé pravidlo v oddílu pravidel, takže pravidlo přidané zítra se zítra i změří; objektový graf repozitáře při sestavení této stránky indexuje {{ <cite_figure name="rule_objects" /> }} dokumentů pravidel. Gate hlásí dva nálezy a ty selhávají záměrně různě. `dangling` znamená, že blok jmenuje cestu, která ve stromu není. Selže vždy a nikdy se neomlouvá, protože důkaz, který neexistuje, je horší než žádný: pravidlo se čte jako dokázané. `unproven` znamená, že pravidlo nejmenuje validátor, test ani důvod. Ten dluh drží seznam baseline, který se smí jen zkracovat, a když tento článek vznikal, nebylo na něm žádné pravidlo.

Třída není výjimka. Pravidlo třídy `advisory` dostane stejnou otázku, protože třída říká, co porušení stojí, ne zda někdo pozná, že k němu došlo. Jediná výjimka je úzká: pravidlo převzaté ze standardního balíčku a označené `principle`, které balíček připíná hashem obsahu a nazývá ho kulturní větou bez mechanického vyjádření. Kontrola při každém běhu vypíše, kolik pravidel vyjímá, a CI ji spouští jako gate `rule-proof` ve strukturní úloze.

## Deset stavů a tři, které nesmí splynout

Gate rozhoduje, zda pravidlo jmenuje svůj důkaz. Zda důkaz proběhl, je druhá otázka a odpovídá na ni modul pravidel spustitelného programu: každé pravidlo spojí se stromem a s ledgerem zaznamenaných běhů, který definuje [ADR 0041](@/adrs/adr-0041.md) (navržené): důkaz je zaznamenané provedení a nezměněné vstupy nejsou dokázání. [`docs/DOCTRINE.md`](@/docs/doctrine.md) uvádí deset stavů, které modul hlásí, od `proven`, úspěšného běhu, od kterého se nic nezměnilo, přes `stale`, `failing` a `not run` až po `dangling` a `unproven`.

Tři z nich nesmí nikdy splynout. `proven` je úspěšný zaznamenaný běh; `gated` je mechanismus, který porušení odmítá a který spouští gate v CI, aniž by se pro něj zaznamenal verdikt; `reviewed` je člověk. Stav pravidla je nejslabší z jeho částí, s jedinou předností: padající případ udělá z pravidla `failing` bez ohledu na to, co říkají jeho ostatní případy. Jinak jeden případ ve stavu `dangling` udělá `dangling` z celého pravidla, ať projde kolik chce jeho ostatních případů, protože ta část, která se nedohledá, je právě ta, které by čtenář věřil. [ADR 0061](@/adrs/adr-0061.md) (navržené) by verdikt gatu zaznamenávalo jako provedení, aby `gated` mohl dosáhnout `proven`.

## Kde leží spodní hranice

Záruka tohoto gatu, [Every rule names what proves it, and every path it names is in the tree](@/guarantees/rule-proof-is-named-and-resolves.md), říká, co nepokrývá: zda je důkaz k něčemu. Gate rozhoduje, že pravidlo něco jmenuje a že to, co jmenuje, je ve stromu, a vyhověl by mu i případ, který by tvrdil jen `true`.

Vlastní případ gatu ukazuje, jak nízko ta hranice leží. Případ 125 nejdřív sestaví zkušební repozitář, a aby převzatá pravidla v něm kvůli němu neselhala, vytvoří prázdný soubor na každé cestě k případu, kterou jmenují. Gate každý z nich přijme, protože se ptá, zda cesta existuje.

Spodní hranice shellu byla skutečným příkladem. [project.portable-shell](@/rules/project-portable-shell-1.md) je blokující: bash 3.2 a userland BSD, žádná asociativní pole, žádný `mapfile`, žádné přepínače jen pro GNU. Do října 2026 jmenovalo jediný případ, `test/cases/08_no_forbidden_constructs.sh`, který hledá `eval` a síťové klienty, konstrukce zakázané v `SECURITY.md`, a žádnou z těch, které zakazuje pravidlo. Každou kontrolu, která se ptala, zda pravidlo jmenuje svůj důkaz, uspokojil případ, který z něj nečetl nic, zatímco `test/mesh-lab/run` deklaroval dvě asociativní pole. Případ 3480 (I3862) teď čte každou sledovanou shellovou jednotku a hledá v ní asociativní pole, `mapfile` a přepínače jen pro GNU, které vyjmenovává. Předtím svůj skener prověří na fixture, jejíž odpověď je známá, takže skener, který by je přestal nacházet, nemůže projít jako čistý strom.

## Pojmenované neznamená odmítnuté

Nic z toho neukazuje, že je porušení odmítnuto. Důkaz přijme jakýkoli pojmenovaný, přítomný a úspěšný případ, takže pravidlo podložené jen případem, který prověřuje bezproblémový průchod, se čte jako dokázané. Audit 8. října 2026 našel druhou polovinu: doctor dokazoval zapojení grepem na řetězec dispatche, a když se do `check.sh` napsalo `|| exit 0`, doctor dál procházel. Případ 18 je důkazem, že ověřovač zapojení funguje. Rozbije každý článek řetězu v jednorázové kopii nástroje a vyžaduje, aby doctor zčervenal, ale žádný validátor odmítající porušení neukazuje.

Záměr (intent) vlastníka, na kterém stojí milník [a-claim-stands-only-on-its-evidence](@/plan/a-claim-stands-only-on-its-evidence.md), nastavuje laťku: blokující pravidlo se hlásí jako dokázané jen podle zaznamenaného běhu případu, ve kterém je jeho porušení odmítnuto. [ADR 0188](@/adrs/adr-0188.md), navržené 18. října 2026 a vlastníkem nepřijaté, rozhoduje jak. Případ jmenuje pravidlo, jehož porušení odmítá, na vlastním řádku hlavičky, `# majordomus-refutes: <rule-id>`. Starší hlavička `# majordomus-negative:` si ponechá svůj význam, způsob selhání příkazu, gatu, skriptu, workflow nebo schopnosti, a nikdy se nečte jako důkaz pravidla. Validátor v režimu `dispatched` v CI prokáže, že odmítne deklarovanou fixture porušení, a doctor si ponechá levnou kontrolu zapojení a řekne, že nic jiného než zapojení nekontroluje.

Rozhodnutí nesou tři issues plánu. [I3110](@/plan/I3110.md) přidá do důkazu každého blokujícího pravidla `refusal_shown` a zavede ráčnu (ratchet) na seznam blokujících pravidel bez odmítajícího případu. [I3111](@/plan/I3111.md) dá každému blokujícímu pravidlu v režimu `dispatched` minimální fixture porušení, spustí proti ní skutečný vstupní bod v jednorázovém repozitáři a selže, když ji validátor přijme. [I3112](@/plan/I3112.md) nechá úlohu validate spočítat blokující pravidla, která nedokazuje žádný zaznamenaný běh její vlastní hlavy, a zařídí, aby stránky pravidel v UI i na webu přestaly kreslit pravidlo, které neproběhlo, je zastaralé nebo neznámé, jako úspěch. Když tento článek vznikal, plán vedl I3110 jako připravené a zbylá dvě jako blokovaná, obě závislá na něm, a žádný případ nenesl hlavičku refutes, ani případ 3480. ADR 0188 očekává, že většina blokujících pravidel začne na seznamu pravidel, jejichž odmítnutí není ukázáno, a ten seznam se smí jen zkracovat.

## Jak to ověřit

`scripts/ci/rule-proof-check` hlásí oba nálezy a selže na novém dluhu, `--strict` baseline ignoruje. `majordomus-cli rules` hlásí stav každého pravidla vedle toho, co jmenuje, a `bash test/run.sh 125_rule_proof` spustí vlastní případ gatu. `majordomus intent explain a-claim-stands-only-on-its-evidence` ukáže kritérium `blocking-rule-shows-refusal` a jeho plánovaný důkaz a `git grep -l 'majordomus-refutes:' test/cases` vypíše případy, které odmítnutí deklarují.

## Co tento článek netvrdí

Netvrdí, že by u některého blokujícího pravidla bylo ukázáno, že odmítá své porušení: podle rozhodnutí, které plán navrhuje, zatím u žádného. ADR 0041, 0048, 0061 a 0188 jsou navržená a vlastník nepřijal žádné z nich; gate a graf důkazů, které popisují, jsou na masteru a běží. Jak repozitář odmítá vlastní dokumentaci, popisuje [Dokumentace, kterou lze odmítnout](@/dokumentace-kterou-lze-odmitnout.md), proč kontrola, která se nikdy neozvala, není úspěch, vysvětluje [Mlčení není verdikt](@/mlceni-neni-verdikt.md) a odkud se bere spodní hranice shellu, ukazuje [Automatizace je schopnost](@/automatizace-je-schopnost.md).
