+++
title = "Jeden zdroj, každý jazyk"
description = "Druhou verzi majordomus.dev nikdo neudržuje. Jak se každý jazyk webu odvozuje z anglického buildu a z paměti přeložených segmentů, jak se drží svého originálu ve skutečném prohlížeči a kde je ručně psaný překlad připuštěn jako druhý zdroj, protože ho gate drží k prvnímu."
template = "page.html"
[extra]
lang = "cs"
translated_from = "one-source-every-language/"
source_digest = "8e31aa594f2eadb51816fe94a78d6fef610a233521aca7832f1209d67a2e310a"
related = ["napsano-jednou-odvozeno-vsude/", "dokumentace-kterou-lze-odmitnout/", "pochybnost-rozhodne-experiment/"]
[extra.translations]
en = "one-source-every-language/"
+++

Dne 19. října 2026 vlastník nařídil, aby majordomus.dev byl od té chvíle vždy plně vícejazyčný, přinejmenším česky a anglicky, s přepínačem jazyků, a aby se každý jazyk odvozoval a překládal automaticky. Slovy, která zaznamenává [ADR 0218](@/adrs/adr-0218.md) (přijaté): „Nechci udržovat dvě jazykové verze; musí to fungovat automaticky, odvozeně, inferovaně.“ Téměř každá stránka webu už tehdy vznikala z anglických záznamů, takže otázka zněla, jak může existovat druhý jazyk, aniž by ho kdokoli psal dvakrát.

## Tři cesty a ta zvolená

Rozhodnutí váží tři. Druhý strom obsahu, český soubor vedle každé stránky, by nutil každý generátor psát dvakrát a každou šablonu klíčovat své řetězce, zatímco data, která šablony čtou, by se dál vykreslovala anglicky: dvě verze, které vlastník odmítl, jen psané stroji místo lidí. Překlad během buildu, dotazem na model, by způsobil, že se dva buildy jednoho commitu liší, CI by muselo držet přístupový údaj a výpadek modelu by shodil web. Zvolená je třetí cesta: průchod hotovými stránkami a paměť přeložených segmentů.

## Průchod hotovým webem

Po Zole, po průchodu přístupnosti a po diagramech spustí `scripts/site-build` nad každou stránkou, kterou zapsal, `scripts/lib/site-i18n.mjs`. Stránka se rozřeže na segmenty: každý úsek řádkového obsahu mezi hranicemi bloků a každý atribut, se kterým se čtenář setká, třeba titulek, alternativní text nebo popis stránky. Kód, předformátovaný text, vstup z klávesnice, SVG a cokoli označeného `translate="no"` segmentem nikdy není. Kopie pod `/cs/` je anglická stránka, ve které je každý segment, který paměť zná, nahrazený, každý vnitřní odkaz míří na protějšek, jazyk stránky a kanonická adresa jsou nastavené, alternativy doplněné a upozornění, že text je strojově přeložený, je viditelné. Protože průchod čte vykreslené HTML, pokrývá šablony, generovaná data i obsah stejně a šablona pro to, aby byla přeložena, nemusí dělat nic.

## Paměť s klíčem z angličtiny

Paměť v `site/i18n/<lang>/memory/` drží jeden záznam na segment, s klíčem z hashe angličtiny, ve které je řádkové značkování nahrazené číslovanými zástupci. Stejná slova ve dvou šablonách jsou jeden záznam. Změněný anglický segment má nový klíč, takže zastaralý překlad se nikdy nepodá: segment se vrátí k angličtině, dokud není znovu přeložen. Překlad se připustí jen se strukturou svého zdroje, s každým zástupcem právě jednou a vnořeným jako v angličtině, takže překladatel může přeskládat slova, ale nikdy nemůže vypustit odkaz nebo kus kódu.

Překlad je samostatný, automatický krok mimo build. Build vypíše, co paměti chybí, a zveřejní její pokrytí na `/cs/i18n.json`. `scripts/site-translate run` se ptá modelu přes adaptéry poradců z [ADR 0098](@/adrs/adr-0098.md), tytéž, které používá záznam úvahy, a jeho export a import přenášejí dávky k překladatelům mimo proces. Glosář a styl, které překladatel dostane, leží vedle paměti v průvodci daného jazyka a adresář deklaruje svůj jazyk v souboru edice, a tak [project.english-only](@/rules/project-english-only-4.md) připouští text, který není anglický.

## Drženo svého originálu v prohlížeči

Blokující pravidlo [project.the-site-is-offered-in-every-language](@/rules/project-the-site-is-offered-in-every-language-1.md) říká, co kopie dluží. Každá stránka má kopii v každém deklarovaném jazyce a žádná kopie se nepíše ručně. V každé kopii každý odkaz vede na stránku, kterou build zapsal, a zůstává v jazyce té kopie. Přepínač i alternativy vedou na každou kopii a zpět. Při každé šířce, kterou design deklaruje, má horní lišta kopie výšku a řádky lišty anglické stránky. Ta poslední věta má svou historii: první česká kopie vyšla s horní lištou zalomenou na druhý řádek, protože české popisky byly delší a řada tlačítek se přeložila jako jedna věta, a nic kopii s originálem neporovnávalo.

Teď to dělá gate `site-languages`. Načte v Chromiu každou stránku každé kopie, obslouženou ze samotného buildu bez sítě, drží každou k pravidlu a pojmenuje stránku i to, co naměřil. Překlad, který by rozbil lištu, se opravuje v paměti, nikdy vyřazením stránky nebo šířky. Případ 3590 dokazuje průchod, který kopie zapisuje, včetně odmítnutých překladů a druhého průchodu, který už nic nezmění.

## Kde je článek druhým zdrojem

Dodatek z 21. října 2026 zaznamenává jedinou výjimku, kterou gaty teď vynucují. Kontrola 21 v `scripts/site-check` vyžaduje, aby se každý článek, který uvádí navigace, dal číst v každém jazyce, který web deklaruje, a aby se členové překladové skupiny navzájem jmenovali. Překlad jmenuje článek, ze kterého vznikl, a digest jeho těla a kontrola 21 digest přepočítá a odmítne překlad, kterému jeho zdroj utekl, aniž by ho kdy přepsala. První bod rozhodnutí proto teď zní, že angličtina je jediný zdroj, kromě deklarovaného překladu článku, který kontrola 21 drží u digestu jeho zdroje.

Taková stránka se do kopie svého jazyka přenese beze změny. Strojová kopie anglického článku jmenuje ručně psaný překlad jako svou kanonickou stránku, takže čtenář i vyhledávač potkají autorský text. Alternativa, vyřadit protějšky článků z kontroly a ty stránky smazat, by zachovala původní znění a odstranila by jediné místo, kde se dokazuje, že překlad drží krok se svým zdrojem. Tento článek je jedním z nich: jmenuje anglický originál a digest jeho těla.

## Co zatím nedělá

Když rozhodnutí vznikalo, vyhledávání, sitemap a vlastní řetězce skriptů ležely mimo stránky, které průchod čte. Podle měření z 22. října 2026 nese sitemap každou kopii s jejími alternativami a hlídá ho případ 3590. Vyhledávací index uvádí každou kopii, ale záznam strojové kopie si ponechává anglický titulek a popis; v jazyce kopie je jen záznam autorského překladu. Skripty české úvodní stránky žádnou anglickou frázi nenesou, a to je jediná změřená stránka. Překlad je strojový a na každé přeložené stránce to říká, s odkazem na originál. Pokrytí se měří při každém buildu a gate, který by odmítl jeho pokles, je dalším krokem rozhodnutí, zatím ne gatem. Každá změna anglické prózy stojí překlad segmentů, které změnila, a jen jich.

## Jak to ověřit

`/cs/i18n.json` na živém webu hlásí pokrytí paměti a každá stránka pod `/cs/` ukazuje své upozornění a odkazuje zpět na angličtinu. `bash test/run.sh 3590_the_site_is_derived_in_every_language` spustí případ, který drží průchod, `node scripts/lib/site-i18n-probe.mjs` spustí gate nad buildem a `scripts/site-check --articles` drží překlady článků u jejich zdrojů.

ADR 0218 je přijaté a jeho dodatek je jeho součástí. Jak se jedna deklarace dostane do každého rozhraní, popisuje [Napsáno jednou, odvozeno všude](@/napsano-jednou-odvozeno-vsude.md), jak repozitář odmítá vlastní dokumentaci, vysvětluje [Dokumentace, kterou lze odmítnout](@/dokumentace-kterou-lze-odmitnout.md) a jak se otázka klade poradcům, kteří také překládají, ukazuje [Pochybnost rozhodne experiment](@/pochybnost-rozhodne-experiment.md).
