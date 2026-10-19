+++
title = "Všechno v jednom proudu"
description = "Zhruba tucet sessions na pěti strojích pracovalo na jednom repozitáři a žádná neviděla, co dělají ostatní, dokud si nenapsala vlastní polling smyčku. Co změnil jeden proud signálů, ze kterého je šum sbalený pryč, a co naměřil hned první den."
template = "page.html"
[extra]
lang = "cs"
translated_from = "everything-followed/"
source_digest = "2fd4eb0ff28e2bb5767724e5d664f44bbef19fa23cc485a399606c43305fb1ed"
related = ["teze/", "dokumentace-kterou-lze-odmitnout/", "vlozene-zadani-se-stava-planem/"]
[extra.translations]
en = "everything-followed/"
+++
{% raw %}

Večer 10. října 2026 na tomhle repozitáři pracoval zhruba tucet AI sessions, rozložených na pět strojů. Sdílely mesh: každá session, každý claim na cestu, každý handover a každá žádost o review byly podepsané události, které se během jednoho heartbeatu replikovaly na všechny stroje. Teoreticky tedy každá session mohla vidět všechno.

Prakticky to nedělala žádná. Žádosti o review zůstávaly hodiny bez odpovědi. Handover publikovaný pro jeden stroj se našel náhodou. Dvě sessions si ve stejné minutě vzaly stejné číslo issue. Všechny ty informace v journalu byly. Jen ho nikdo nečetl — a ukázalo se, že to bylo rozumné rozhodnutí.

## Journal byl hlavně šum

Server každého checkoutu při každém heartbeatu znovu ohlásí všechny klienty, kteří jsou k němu připojení, jako mesh sessions. Právě díky tomu je session z jednoho stroje vidět na jiném. Znamená to ale také, že většina journalu je tatáž věta, řečená pořád znovu.

Ve vzorku 1000 po sobě jdoucích událostí bylo 954 událostí `session_opened` pro sessions, jejichž obsah se nezměnil. Jen 46 událostí říkalo něco nového: osmnáct claimů, sedm uvolnění, šest žádostí o review a šest odpovědí, pět uzavřených sessions a čtyři handovery. Číst journal tak, jak je, znamenalo projít 954 řádků, aby se našlo těch 46.

A tak si každá session, která potřebovala vědět, co se děje, napsala vlastní smyčku. Pouštěla `jq` nad `majordomus mesh events`, držela si množinu už viděných sessions, na merge se ptala přes `gh pr list` a na ref, na který čekala, přes `git ls-remote`. Jedna z těch smyček — napsala ji session, která pak tuhle funkci postavila — je specifikace, podle které funkce vznikla.

## Jeden proud, sbalený

`majordomus monitor` je přesně ta smyčka, napsaná jednou a vlastněná repozitářem (ADR 0133). Spojuje tři zdroje: journal meshe (do kterého se promítá board), stopu executoru s jeho mergi, obnovami a selháními, a pull requesty a běhy workflow na GitHubu. Odpovídá jednou stránkou signálů a každý z nich je řádek, podle kterého může člověk nebo agent jednat.

Číst ho stojí za to hlavně kvůli tomu, co vynechá. A o tom rozhoduje sbalování.

Session se do proudu dostane, když poprvé řekne, co dělá, a pak znovu, když se to změní — ne když to jen zopakuje nebo když se jí posune head. Claim se do něj dostane, pokud tatáž session nedržela už stejný. Krok executoru, který nic nerozhodl, se do něj nedostane. Zelený běh CI na větvi pull requestu je očekávaný výsledek, takže taky ne; červený vždycky. Všechno vynechané se počítá, ne schovává, takže čtenář vidí, kolik se toho sbalilo.

Dvě další pravidla z toho dělají něco, na co se session může spolehnout.

První zní: **co jmenuje tebe, jde první**. Review, jehož předmět jmenuje tvého workera, odpověď na tvou žádost o review, handover, který tě zmiňuje, nebo neúspěšný běh na tvé větvi dostane značku `for_you` a je vypsaný přede vším ostatním.

Druhé zní: **ticho musí něco znamenat**. Zdroj, který nešlo přečíst (mesh neběží, chybí `gh`, GitHub neodpověděl včas), je vypsaný i s důvodem. Nikdy se nevrátí jako prázdný seznam, protože prázdný seznam se čte jako klidný repozitář.

## Co naměřil první den

První skutečná stránka, za poslední dvě hodiny toho večera, ukázala 50 signálů a sbalila 1371 událostí. Trvalo to tři sekundy a většinu z nich zabralo čekání na GitHub.

Sbalování nebylo při prvním spuštění hotové a proč, ukázal sám proud. Mezi 50 signály bylo dvacet šest řádků jako `board-p97 (claude-code) joined:`, po kterých o minutu později následovalo `left`. Byl to klient na jednom stroji, který se každou minutu znovu připojoval, aniž by kdy řekl, co dělá.

Pravidlo, které z toho vzešlo, je to, na kterém stojí název tohohle článku: **spojení, které o sobě nic neřekne, do proudu nepatří**. Jeho příchod se ukáže, až když poprvé ohlásí záměr, a jeho odchod jen tehdy, když se ukázal i příchod. Dnes to hlídá test a neustálé připojování a odpojování toho klienta už se k nikomu nedostane.

Při návrhu kurzoru se našlo ještě něco. Stránkování samotného journalu používalo jen Lamportovo razítko, takže dvě události ze dvou strojů se stejným razítkem mohly skončit na opačných stranách hranice stránky — a ta druhá se nikdy nepřečetla. Kurzor monitoru nese úplnou pozici každého zdroje zvlášť: u meshe razítko, stream a pořadové číslo. Stránka za kurzorem nic nezopakuje a nic nepřeskočí.

## Jak se používá

```sh
majordomus monitor --for 1b          # poslední hodina, nejdřív to, co jmenuje 1b
majordomus monitor --follow          # průběžně vypisuje, co je nového
majordomus monitor --follow --match '#885' --until merged --timeout 1h
```

Poslední řádek používají agenti nejčastěji: počká na jednu událost a skončí, takže session může čekat na merge, aniž by kvůli tomu psala smyčku.

Tentýž proud je MCP nástroj `majordomus_monitor`, `GET /api/v1/monitor` a stránka Cockpitu `/cockpit/monitor`, která po dobu, co je otevřená, přidává nové signály nahoru. Každý briefing na začátku session z něj vypíše jeden řádek a `majordomus context` mu věnuje sekci. Bootstrap pro providery, který čte každý agent, ten příkaz jmenuje, takže se o něm nová session dozví stejně jako o všem ostatním.

Monitor nic nezapisuje a nic si neukládá. Každý signál pochází ze záznamu, který už existoval, a jednat podle signálu zůstává na capability, které ten úkon patří. Mesh díky němu není chytřejší. Jen je čitelné to, co mesh věděl už předtím.

Manuál je [`docs/MONITOR.md`](@/docs/monitor.md). Rozhodnutí je [ADR 0133](@/adrs/adr-0133.md), zatím ve stavu `proposed`. Důkazem jsou cases 1050 až 1052, které spouštějí server s deklarovaným meshem a pro GitHub `gh`, jenž vypisuje fixtures; cases 1053 a 1054 dokazují stránku Cockpitu a to, co se o proudu dozví každá session: sekci v `majordomus context`, řádek v briefingu a zmínku v bootstrapu. K tomu jsou tu unit testy každého pravidla sbalování, o kterém je tu řeč.
{% endraw %}
