+++
title = "Sázka na vrstvu, ne na model"
description = "Proč Majordomus není další AI agent: modely se stanou zaměnitelnými, koordinaci si jejich výrobci vezmou sami a co zbývá, je nestranný doklad, že práce je hotová."
template = "page.html"
[extra]
lang = "cs"
+++

Modely budou zaměnitelné. Nevím, který bude za rok nejlepší, a stavět produkt na tom, že to vím, by byla loterie. Sázím na něco nudnějšího: že hodnota vznikne ve vrstvě, která práci modelů umí řídit a hlavně posoudit.

## Enginy vznikly ze her

Rád se to přirovnává k herním enginům. Každé studio si kdysi psalo vlastní infrastrukturu, pak vznikl trh hotových enginů a dnes si vlastní engine píše málokdo. To je pravda, jenže neúplná. Enginy, které vyhrály, nevznikly jako infrastruktura hledající zákazníka. Oddělily se od konkrétní hry, která je napřed prověřila.

Moje hra je tenhle repozitář. Majordomus vznikl destilací z prostředí, kde agenti pracovali ve velkém a selhávali z nudných důvodů. Čísla jsou v README: instrukční soubory dvou nástrojů bez společného obsahu, kontroly vedené jako blokující, které nic nespouští, desítky worktrees s duplikovanými patchi, z nichž se skoro nic nezmergovalo. Od té doby řídí sám sebe. Změny tady procházejí stejným `finish`, jaký nabízím ostatním.

## Koordinace se ubránit nedá

Koordinace agentů je nejviditelnější část celé věci a zároveň nejslabší. Výrobci modelů ji přidávají do vlastních nástrojů: subagenty, týmy agentů, běhy v cloudu. Kdo staví jen orchestraci, staví na pozemku, který mu nepatří.

Co si výrobce modelu vzít nemůže, je nestrannost. Těžko uvěřím, že laboratoř férově posoudí práci konkurenčního modelu, a už vůbec ne práci toho vlastního. Doklad o hotové práci má hodnotu právě proto, že nepatří tomu, kdo ji udělal.

## Co tu znamená důkaz

„Důkaz správnosti“ u nedeterministického výstupu je mávání rukama, dokud někdo neřekne, co přesně jím myslí. Tady je to kontrakt. Když worker prohlásí práci za hotovou, `majordomus finish` projde každý jeho řádek: otevřené blokery, předávací poznámku, jestli změny zůstaly v deklarovaném rozsahu, jestli zaznamenaný stav odpovídá HEAD a jestli projde ověřovací příkaz. Když jediný řádek selže, nezapíše nic a řekne, jak selhání zopakovat.

Záznam z [výzvy](@/challenge.md), anonymizovaný a jinak nedotčený. Rozsah je čistý, ale vlastní test repozitáře padá, takže tvrzení workeru, že je hotovo, neprojde:

```
$ majordomus finish --outcome completed --verify-command 'sh test/parse.sh'
OK   blockers    t-<id> — none open
OK   note        t-<id> — <time>--main--<head>--<hash>.md
OK   scope       t-<id> — 1 touched file(s), all within scope
OK   state       t-<id> — exact (head <head>)
FAIL verification t-<id> — sh test/parse.sh — exit 1, <s>s  [reproduce: sh test/parse.sh]
finish: refused, 1 unmet
```

Neříká to, že kód je správně. Říká to, které konkrétní podmínky „hotovo“ splněné jsou a které ne, a dává příkaz, kterým to kdokoli ověří znovu. Na víc si netroufám a víc nepotřebuju.

Stejný tvar drží i web, který čteš. Když se tahle stránka builduje, má {{ <cite_figure name="claims_guaranteed" /> }} tvrzení stav guaranteed z celkových {{ <cite_figure name="claims_total" /> }} a každé z nich má test, který by spadl. Jak to funguje, rozepisuju v článku [Dokumentace, kterou lze odmítnout](@/dokumentace-kterou-lze-odmitnout.md).

## Dnešní bolest, ne zítřejší

Stovky agentů v jednom repozitáři dnes skoro nikdo nemá. Otázky, které si týmy kladou už teď, jsou přízemnější: [kdo to udělal a podle jakých pravidel](@/why/who-did-this-and-under-what-policy.md), [za co šly peníze](@/why/spend-not-tied-to-outcomes.md), [jestli je hotovo jen proto, že to řekl model](@/why/done-because-the-model-said-so.md). Na ty odpovídám nejdřív. Škála přijde, nebo nepřijde; nástroj, který neumí odpovědět za jednoho agenta, ji stejně neunese.

Sázím taky na to, že v regulovaných oborech se otázka „kdo to udělal a pod jakou politikou“ bude klást čím dál častěji. Je to sázka, ne tvrzení. Majordomus nikoho nedělá compliant.

## Formát, ne platforma

`.ai/` jsou obyčejné soubory v gitu. Majordomus běží lokálně a nikdy nevolá model. Je to záměr. Platforma chce, aby ses do ní přestěhoval; formát chce, aby ho mohl číst kdokoli: Claude, Codex, Gemini i nástroj, který ještě neexistuje. Má-li tahle vrstva přežít výměnu modelů, nesmí být sama dalším místem, odkud se nedá odejít.

## Co netvrdím

Netvrdím, že Majordomus ověří správnost kódu, že nahradí review, ani že unese stovky agentů. To poslední jsem nezměřil. Co nedělá, stojí v [omezeních](@/limitations.md); co je zaručené, co jen doporučené, co plánované a co odmítnuté, je v `docs/SITE_CLAIMS.md`. Pokud tahle stránka tvrdí víc než ony, platí ony.
