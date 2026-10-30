+++
title = "Pochybnost rozhodne experiment"
description = "Podstatnou inženýrskou nejistotu rozhodnutou potichu nelze auditovat a odpověď poradce vložená do chatu do další session zmizí. Jak Majordomus zapisuje pochybnost nejdřív s jejími důkazy, jak počítá, kolik nezávislého review si zaslouží, jak bere chybějícího poradce jako stav, jak rozhoduje neshodu experimentem místo počítání a jak předává závěr další session."
template = "page.html"
[extra]
lang = "cs"
translated_from = "a-doubt-is-settled-by-an-experiment/"
source_digest = "0a1f3e7b1e481f311e05e742227c225c5ad0737c1fc87fb753315a8fdef4b382"
related = ["plan-se-napadne-driv-nez-se-postavi/", "session-skonci-prace-ne/", "mnoho-stroju-jeden-denik/", "mlceni-neni-verdikt/", "uspora-se-meri-netvrdi/"]
[extra.translations]
en = "a-doubt-is-settled-by-an-experiment/"
+++
{% raw %}

Session, která si není jistá inženýrským rozhodnutím, má dvě snadné cesty ven: rozhodnout sama a nic neříct, nebo vložit otázku do chatu s jiným modelem a řídit se odpovědí. [`docs/REASONING.md`](@/docs/reasoning.md) pojmenovává, co každá z nich stojí. Nejistota rozhodnutá potichu je nejistota, kterou nikdo nemůže auditovat, a názor poradce (advisor) vložený do chatu do další session zmizí.

[ADR 0098](@/adrs/adr-0098.md) (navržené) z 1. října 2026 vychází z pracovního postupu, kterým vlastník repozitáře takové pochybnosti řešil: prozkoumat věc lokálně, zeptat se modelu jednoho dodavatele a pak jiného, výsledky shrnout a pokračovat. Ten postup žil v soukromých poznámkách jednoho workera a v pomocném skriptu v odkládacím adresáři jedné session, takže nic nezaznamenalo, co poradce řekl, a nic nedokázalo odlišit review, které proběhlo, od review, které bylo jen deklarované. Byl to navíc sled jmen dodavatelů, tedy tvar, který se rozbije, jakmile není nainstalované CLI nebo chybí klíč. Rozhodnutí dělá z nejistoty, rady i rozhodnutí typované záznamy s jediným zapisovatelem (writer) a dodavatele drží mimo logiku. Každé rozhodnutí patří session, která práci dělá, a poradci nerozhodují nic.

## Nejdřív důkazy

Řetěz začíná posouzením (assessment): předmět na jeden řádek, jak podstatná je nejistota na stupnici od `trivial` po `critical`, jak moc si je session jistá, její hypotéza a dosud sebrané důkazy. Důkaz je odkaz, nikdy kopie: soubor a řádek, test, ADR, pravidlo, commit, příkaz a to, co ukázal, pozorování za běhu.

Zapisovatel to pořadí vynucuje. Posouzení nejistoty `material`, `high` nebo `critical`, které necituje žádný důkaz, je odmítnuto s odůvodněním, že podstatné posouzení nese důkazy sebrané dřív, než je kdokoli dotázán.

Jedna taková pochybnost se rozhodovala 21. října 2026. Plánovaná změna by datovala přistání podle záznamu forge o jeho merge, čteného tam, kde executor před každým rozhodnutím pozoruje forge, a test drží, že toto čtení se na sloučené pull requesty nikdy neptá. Session před rozhodnutím měřila: reflog tohoto klonu viděl každé z posledních čtyř přistání nejpozději 94 sekund po záznamu forge, při prahu šesti hodin. Její závěr ponechal reflog a čtení z forge odložil a [`docs/RELEASE.md`](@/docs/release.md) uvádí měření, které rozhodlo.

## Rozpočet spočítá politika

Čistá politika (policy) pak rozhodne, jestli se nezávislé review vyplatí, a začne rozpočtem: kolik poradců smí plán vybrat. V režimu `standard`, který platí, když žádný jiný nastavený není, nedostane triviální ani nízká pochybnost žádného poradce, podstatná dostane jednoho a vysoká nebo kritická dva. `strict` povolí u kritické pochybnosti tři, `fast` nejvýš jednoho a `ci`, režim CI runneru, žádného. Vysoká důvěra v lokální hypotézu sníží rozpočet o jednoho poradce.

Plán se počítá, nikdy se nedodává. Volající jmenuje jen posouzení a zapisovatel spočítá plán z režimu a z dostupnosti každého poradce v tu chvíli a ten snímek pak s plánem uloží. Každý vynechaný poradce je v plánu uvedený i s důvodem. Konzultace se připustí jen u poradce, kterého její plán vybral, takže nikdo nemůže zapsat review, o které žádný plán nežádal.

## Chybějící poradce je stav

Politika jmenuje schopnosti (capabilities), třeba `independent_reasoning` nebo `code_review`, a nikdy poradce. Kteří poradci danou schopnost nabízejí, odpovídá katalog `share/advisors.yaml`, v jehož pořadí je zapsaná preference repozitáře; jestli ji mohou nabídnout teď, odpovídá dostupnost. Poradcem je i propojený runtime meshe, který nabízí review.

Dostupnost se odvozuje z přítomnosti, režimu a zaznamenaných výsledků a nic na cestě do repozitáře, v `doctor` ani v CI nekontaktuje žádný model. Přítomnost je spustitelný soubor na `PATH` nebo nastavená proměnná s přístupovým údajem, jejíž hodnotu crate nikdy nečte; jestli poradce odpovídá, se zjistí teprve dotazem. Poradce, který chybí nebo je vypnutý, je `unavailable`, `not_configured` nebo `disabled` a zaznamenaná selhání z něj na ochrannou lhůtu (cooldown) udělají `temporarily_failed` nebo `rate_limited`. Bootstrap, který repozitář generuje pro své workery, to říká přímo: žádný poradce není potřeba a chybějící poradce je hlášený stav, nikdy důvod zastavit se nebo se ptát člověka, co dělat.

Když plán u podstatné pochybnosti nevybere nikoho, nese místo toho strukturované lokální review. Session v něm uvede hypotézu a její nejsilnější alternativu, vypíše každý předpoklad i s důkazy, které za ním stojí, pojmenuje, co by hypotézu vyvrátilo, spustí nejmenší experiment, který mezi nimi rozliší, a zapíše závěr.

Důvod zaznamenává pravidlo [project.advisors-are-optional](@/rules/project-advisors-are-optional-1.md). Když ten postup poprvé běžel přes kód, v session, která postavila ADR 0098, byl přístupový údaj jednoho poradce jen zástupný a jiný poradce se odmítl spustit v nedůvěryhodném adresáři. Workflow napsaný jako „zeptej se tohohle, pak tamtoho“ by se tam zastavil. Zaznamenaná jako `auth_failed` a `error` stála ta dvě selhání každé jedno ohraničené čekání a práce pokračovala.

## Počet nerozhoduje

Když se poradci neshodnou mezi sebou nebo se session, je neshoda (disagreement) samostatným záznamem: postoje, předpoklady, na kterých se rozcházejí, a otázka, jejíž odpověď rozhoduje. Závěr k tomu posouzení je odmítán, dokud neshodu nerozhodne rozřešení (resolution), a rozřešení bez důkazů je odmítnuto. Jmenuje experiment, co ukázal, a postoj, kterému dává za pravdu, případně žádný.

Počet poradců na každé straně do rozhodnutí nikdy nevstupuje. Pravidlo [project.review-is-recorded-not-claimed](@/rules/project-review-is-recorded-not-claimed-1.md) uvádí důvod: modely sdílejí trénovací data i způsoby selhání, takže počet souhlasných odpovědí měří korelaci, ne správnost. Case 732 to drží se dvěma skriptovanými poradci, z nichž jeden podporuje hypotézu session a druhý navrhuje jinou. Když se dva ze tří postojů shodují, zapisovatel odmítne závěr i „rozřešení“ bez důkazů a postoj menšiny připustí, jakmile otázku rozhodne experiment.

## Závěr, který si další session přečte

Závěr (conclusion) zaznamenává rozhodnutí, jeho zdůvodnění a důkazy, konzultace, které zvážil, alternativy, které zamítl, rizika, která zbývají, a plán validace. Je odmítnut, dokud vynechává odpověď, kterou jeho posouzení dostalo, a u podstatné nejistoty je odmítnut i bez důkazů nebo bez plánu validace. Kdo ho posoudil, `reviewed_by`, a kolik jich bylo, `independent_review_count`, spočítá zapisovatel z konzultací, které závěr cituje. Vstup pro ně žádné pole nemá, protože počet review, který může volající napsat, je review, které si volající může vymyslet.

Záznamy jsou stavem checkoutu, každý je jeden soubor JSON pod `.ai/local/state/reasoning/<task>/`, git je nikdy nesleduje a nikdy se nepublikují. `majordomus reasoning status` z nich odvodí stav tasku a tentýž report se objeví v `majordomus context` i v každém handoveru, který zapíše `majordomus handover --derive`. Nová session si rozhodnutí i to, kdo ho posoudil, přečte, místo aby je objevovala znovu. Plán pro pozdější posouzení téhož předmětu znovu použije platný závěr a nikoho se neptá, pokud to posouzení nedeklaruje nové důkazy; nový závěr pak ten starý jmenuje v `supersedes`.

## Co to nedělá

Neuchovává žádný přepis, jen pole, která z každé odpovědi normalizoval adaptér. Spustitelný program nevolá žádný model, protože síťový kód zůstává mimo crate. Nekontroluje, jestli lokální review opravdu proběhlo, ani neposuzuje, jestli byl experiment dobrý; to ze záznamu posoudí recenzent. Shoda předmětů se posuzuje podle normalizovaného textu, ne podle významu. A počet review počítá poradce, ne zdroje, takže dva peery meshe za jedním adaptérem se započtou jako dvě review; issue I3419 to plánuje změnit.

ADR 0098 je pořád navržené a pravidla, která ho nesou, jsou aktivní a blokující. Drží je case 730 až 736 a `reasoning check`, který odmítne kód nezávislý na providerovi, jenž jmenuje poradce, dodavatele nebo model z katalogu, i CI, které jmenuje přístupový údaj k modelu. Tentýž bootstrap tu praxi shrnuje: zapiš posouzení s jeho důkazy, ať se `scripts/advisor-consult` zeptá těch volitelných poradců, které plán vybere, rozhodni neshodu experimentem, ne počítáním, zapiš závěr a pokračuj. Jak se plán napadá, popisuje [Plán se napadne dřív, než se postaví](@/plan-se-napadne-driv-nez-se-postavi.md), jak práce přežije session, vysvětluje [Session skončí, práce ne](@/session-skonci-prace-ne.md), jak review přechází mezi stroji, ukazuje [Mnoho strojů, jeden deník](@/mnoho-stroju-jeden-denik.md) a celý návrh popisuje [`docs/REASONING.md`](@/docs/reasoning.md).
{% endraw %}
