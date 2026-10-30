+++
title = "Zabráno dřív, než se staví"
description = "Několik AI sessions na jednom repozitáři dřív nebo později postaví tutéž věc dvakrát nebo si vezme totéž číslo. Proč nestačilo oznamovat, co kdo dělá, na co se session zeptá každé větve, než začne stavět, a proč se identifikátor zjišťuje průzkumem, a ne odhadem."
template = "page.html"
[extra]
lang = "cs"
translated_from = "claimed-before-it-is-built/"
source_digest = "f37dcdd45b804033b067a37ca28ed1dc313e24cdaf07aeb9bbd450bd8a0c0477"
related = ["kresla-ne-jmena/", "vsechno-v-jednom-proudu/", "napsano-jednou-odvozeno-vsude/", "session-skonci-prace-ne/", "jedna-vetev-jedno-misto/", "mnoho-stroju-jeden-denik/"]
[extra.translations]
en = "claimed-before-it-is-built/"
+++

Dne 9. září 2026 pracovalo v tomhle repozitáři osm sessions a dvě z nich nezávisle na sobě stavěly subsystém pro vydávání. Použily stejnou cestu modulu, stejnou skupinu příkazů i stejný generovaný changelog a vydržely u toho několik hodin. Obě oznámily, co dělají. Ani jedno oznámení ale neexistovalo, když ta druhá session začínala.

Odpověď byla celou dobu zaznamenaná. Větev pushnutá dřív, než kterákoli z nich začala, už nesla `apps/majordomus-cli/src/release/`. Nic se jí nezeptalo. Dva návrhy, které dojdou k jedné cestě, nejsou konflikt při merge, který se vyřeší spojením dvou diffů. Jsou volbou mezi dvěma hotovými kusy práce, a ten, který prohraje, byl napsán zbytečně.

## Board je paměť, větev je záznam

Každá session oznamuje, co dělá a kterých cest se chce dotknout, a každá jiná session si ta oznámení může přečíst na boardu peerů. Board odpovídá na to, kdo pracuje teď. Nemůže říct, co se postavilo hodinu předtím, než se spustil proces, který ho obsluhuje, protože board žije v paměti procesu.

[ADR 0044](@/adrs/adr-0044.md) (přijaté) opravilo tu část, která byla prostě špatně. Server obsluhuje checkout, ve kterém byl spuštěn, a propojený worktree má server vlastní, takže board jednoho serveru nikdy nebyl boardem celého repozitáře. Board, který session čte, se teď sbírá: každý checkout, který git eviduje, je požádán o svůj vlastní board, a odpověď říká, které checkouty pokryla a jestli dokázala přečíst všechny. Krátký board se nikdy nevydává za prázdný repozitář.

I sesbíraný board vidí jen sessions, které běží. Větev pushnutá včera nenese žádné oznámení, a své cesty si přesto nárokuje.

## Na cesty se ptá každé větve

Než tedy session vytvoří modul, skupinu příkazů, druh dokumentu nebo generovaný artefakt, zeptá se gitu. `scripts/collision-check --new <paths>` projde každou lokální i vzdálenou větev, hledá verzi těch cest, která se liší od základu, a jmenuje každou větev, která ji nese, s jejím posledním commitem a s tím, jak dávno přistál. Pravidlem je [project.work-is-claimed-before-it-is-built](@/rules/project-work-is-claimed-before-it-is-built-1.md).

Kontrola odliší kolizi od související práce. Záchranná kopie se stejnými bajty je vypsaná a projde. Stejně tak větev postavená na té druhé nebo kopie téže práce, která se mezitím rozešla. Koliduje jen cesta, kterou dvě větve přidaly nezávisle na sobě a s jiným obsahem. Kontrola také odmítne odpovědět, když nevidí: mělký klon nebo základ, který nejde dohledat, dostane vlastní návratový kód a nikdy se nečte jako „nic si to nenárokuje“.

Až do 13. září ji nic nespouštělo. Bootstrapy providerů ji jmenovaly, ale žádný hook, recept ani workflow ji nevolal. Teď se jí ptá brána u každého pull requestu, na každou cestu, kterou pull request přidává, a kolizi odmítne. Člověk ji může vědomě přijmout štítkem `collision:accepted`, protože dvě implementace někdy stojí za porovnání a nic mechanického to rozhodnout neumí. Recenzent kolizi i tak uvidí vypsanou.

## Čísla se zjišťují průzkumem, nikdy se neodhadují

Cesty jsou jen polovina. Druhou polovinou jsou identifikátory: číslo ADR, číslo behaviorálního případu (case), id issue. Dne 11. září si tři větve nárokovaly ADR 0044 a dvě 0043 a tři různé sessions kvůli tomu každá napsala tentýž průzkum, aby z toho vybruslily.

Ten průzkum je teď příkaz. `majordomus adr next` přečte každý ref, každý tag, pracovní strom každého sourozeneckého worktree i board peerů, ohlásí, co si který z nich nárokoval, a řekne, když se některý nepodařilo zastihnout. Přiděluje nad nejvyšší číslo, které kterýkoli z nich viděl, nikdy do díry, protože díra je číslo, které někdo vzal a pustil, nebo vzal a ještě nenapsal, a žádný průzkum ty dva případy nerozliší. `majordomus adr check` odmítne větev, která přidává rozhodnutí pod číslem, jež jiný ref už dává jinému rozhodnutí, a pre-commit hook spouští tutéž kontrolu přes `doctor`. Zdůvodnění je v [ADR 0053](@/adrs/adr-0053.md) (navržené).

Čísla případů dostala totéž zacházení ze stejného důvodu. Dne 12. září průzkum všech refů zjistil, že skoro třetinu čísel případů, která kdy byla přidána, neslo víc než jeden soubor, a že tři sessions si jedno číslo vzaly během několika minut po sobě. `scripts/case-numbers next` dělá průzkum stejného druhu: tento pracovní strom, sourozenecké worktrees, každý ref a board peerů. Brána u každého pull requestu odmítne číslo, které sdílejí dva soubory, kromě kolizí zaznamenaných v den, kdy brána vznikla. Ty leží na baseline, protože jejich přečíslování by rozbilo větve jiných sessions.

## Oznamuje se číslo, ne jen cesta

Ty dvě poloviny nejsou zaměnitelné. Prohledání větví odpovídá na to, co už bylo pushnuto. Jen board vidí číslo, pro které se session rozhodla před minutou a ještě ho nikam nezapsala. Oznámení proto kromě cest jmenuje i identifikátor, který se přiděluje, a session si board přečte dřív, než si číslo vezme.

Když se práce nesmí překrývat napříč stroji, nestačí ani oznámení. Na meshi ([ADR 0067](@/adrs/adr-0067.md), navržené) cestuje oznámení z boardu na každý propojený stroj jako poradní (advisory) claim. Práce, která musí být výlučná, se claimuje exkluzivně, a druhá session, která o ni požádá, je odmítnuta se jménem claimu, na který narazila. Tak se drží role integrace, jak to popisují [Křesla, ne jména](@/kresla-ne-jmena.md).

## Co claim není

Claim na boardu není zámek. Kontrola překryvů jen hlásí a nic z toho, co řekne, nezastaví commit. Změnu odmítne rozsah (scope) vlastního tasku, ke kterému ji drží `majordomus check`, `majordomus finish` a pre-push hook. Pravidlo, které žádá prohledání větví, zůstává jako zvyk poradní, zatímco jeho části, které lze posoudit mechanicky, tedy cesta, kterou dvě větve přidaly různě, nebo číslo, které sdílejí dvě rozhodnutí, jsou brány.

Cena je chvilka před každou novou cestou a jeden řádek na boardu před každým novým číslem. Alternativa se počítala na odpoledne: dvě sessions stavějící jeden subsystém a tři sessions píšící jeden průzkum, aby unikly jedné kolizi. Jak si sessions na několika strojích sdílejí své claimy, popisuje [`docs/MESH.md`](@/docs/mesh.md), a kde se na které větvi pracuje, [`docs/WORKTREES.md`](@/docs/worktrees.md).
