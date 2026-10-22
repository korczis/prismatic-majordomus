+++
title = "Session skončí, práce ne"
description = "Kontextové okno AI session se zaplní, zkompaktuje nebo se zavře spolu s terminálem, obvykle uprostřed práce. Proč Majordomus neukládá konverzaci, co místo toho zapisuje na každé hranici a jak ho šest dní zastaralého pokynu naučilo říkat, jak starý záznam je."
template = "page.html"
[extra]
lang = "cs"
translated_from = "a-session-ends-the-work-does-not/"
source_digest = "7acb745322bac99c976add6a19a2c5ff919311c2cea550c7631b1a5786152247"
related = ["teze/", "vsechno-v-jednom-proudu/", "zabrano-driv-nez-se-stavi/", "jedna-vetev-jedno-misto/", "mnoho-stroju-jeden-denik/", "pochybnost-rozhodne-experiment/", "znalost-odvodi-nastroj-povysi-clovek/"]
[extra.translations]
en = "a-session-ends-the-work-does-not/"
+++

Konverzace s AI pracovníkem je špatné místo na uchovávání čehokoli. Její okno se zaplní a zkompaktuje. Člověk zavře terminál, stroj se restartuje nebo práci převezme jiný pracovník s jiným oknem. Většinou se to stane dřív, než je práce hotová, a další pracovník musí zjistit, kde stojí.

Samozřejmým řešením je konverzaci uložit. [`docs/CONTINUITY.md`](@/docs/continuity.md) vysvětluje, proč to tento repozitář nedělá. Přepis zaznamenává, jak někdo došel k závěru, včetně každé slepé uličky, a stav to není. Provozní stav žije v souborech, které přežijí každou konverzaci. Další pracovník dostane briefing sestavený z těch souborů, ne přepis, který by musel číst znovu.

## Hranici kreslí provider

Jednotkou je **epizoda**, vlastní hranice konverzace u providera: začíná, když se klient připojí, a končí, když se odpojí. Kde provider vysílá události životního cyklu, otevírají a zavírají ji právě ty události ([ADR 0015](@/adrs/adr-0015.md), navržené). Obojí je idempotentní, protože start přijde znovu při obnovení i při kompakci: ponechá epizodu, která je otevřená, a konec, který nenajde nic otevřeného, nic nezapíše. Důvod události rozhodne, jestli se epizoda uzavřela, nebo byla přerušena, a důvod, který adaptér nezná, ji uzavře jako přerušenou, protože prohlásit useknutou epizodu za úplnou je horší z obou chyb.

Když se epizoda otevře, další pracovník dostane to, co zanechala ta minulá ([ADR 0017](@/adrs/adr-0017.md), navržené). Událost startu zapíše ohraničený briefing do kontextu, který se provider chystá sestavit. Řekne, o kterou epizodu jde, co zanechala ta minulá spolu se štítkem, nakolik tomu věřit, a co brání přijetí, a ocituje tu jednu sekci handoveru, podle které pokračující pracovník jedná. Když není co předat, řekne to, protože mlčení vypadá stejně jako briefing, který se nespustil.

Záznam dostanou ještě dva okamžiky. Kompakce je ohlášena dřív, než proběhne, a to je jediný okamžik, kdy se dá cokoli zapsat o kontextu, který se má zahodit, takže se tam odvodí checkpoint. A když epizoda končí, odvodí se před jejím uzavřením handover, ať je task otevřený, nebo ne.

## Checkpoint a handover

Ty dva záznamy se snadno zamění a jejich záměna je drahá. **Checkpoint** je krátká poznámka o postupu, psaná často a omezená délkou, aby ji příští briefing mohl ocitovat celou. **Handover** je záměrný balíček pro pokračování, psaný ve chvíli, kdy pracovník končí. Má tři povinné sekce, cíl, současný stav a další krok, a je odmítnut, když je kterákoli z nich prázdná.

Co umí určit stroj, to pracovník uvádět nesmí. Repozitář, worktree, větev, commit a změněné soubory každého záznamu se při zápisu spočítají z gitu a tělo, které kterýkoli z nich obsahuje, je odmítnuto. Věrohodné shrnutí s hashem commitu, který nikdo neověřil, je horší než žádné shrnutí, protože vypadá ověřitelně.

## Šest dní zastaralého pokynu

Dne 11. září 2026 audit zjistil, že repozitář od 5. září nezapsal jediný checkpoint ani handover. Šest dní přitom `doctor` i `watch` hlásily, že je vše v pořádku. Epizody se normálně otevíraly a zavíraly a zastavily se jen ty dva záznamy, ze kterých pracovník pokračuje.

Jeden task byl označen jako předaný a nikdy nebyl nahrazen. Tři kontroly, každá správně podle vlastního kontraktu, se před jakýmkoli zápisem ptaly, jestli je nějaký task aktivní, a každá skončila, aniž po sobě zanechala záznam, že skončila. Mezitím dostávala každá nová epizoda v briefingu nejnovější handover na větvi, jeho další krok doslovně ocitovaný, pod štítkem, který říkal, že zaznamenaný commit je předkem toho současného. To byla pravda, a o stáří to neříkalo nic. Pracovníci dostávali šest dní pokyn pokračovat v práci, která byla šest dní hotová.

[ADR 0052](@/adrs/adr-0052.md) (navržené) odstranilo předpoklad, na kterém ty tři kontroly stály, totiž že epizoda probíhá uvnitř tasku. Životní cyklus epizody už na stavu tasku nezávisí. Každá událost životního cyklu se zaznamená dřív, než proběhne jakákoli kontrola, takže i přeskočený záznam po sobě zanechá důkaz, že událost dorazila. Čerstvost se stala samostatným rozměrem vedle divergence: záznam může být zároveň `advanced` i `stale`, a právě tahle kombinace výpadek způsobila. Zastaralý záznam se ukazuje jako historie, s jeho stářím a důvodem, a jeho další krok se necituje jako to, co je třeba udělat teď. Manuál ten princip shrnuje jednou větou: záznam je důkaz, nikdy autorita.

## Zeptat se dřív, než se okno zaplní

Napsat handover stojí samostatný tah a ten musí přijít dřív, než okno dojde. Proto si o něj mod providera řekne předem ([ADR 0123](@/adrs/adr-0123.md), navržené). Jakmile okno překročí hranici, pevnou rezervu před svým koncem nebo jeho podíl, podle toho, co nastane dřív, je pracovník jednou požádán o `majordomus handover` se třemi sekcemi, je mu řečeno, ať si nejdřív přečte živý stav, a ať potom skončí. V tomtéž okně se ho mod znovu nezeptá, a vůbec se ho nezeptá, když tam handover už napsal. Adoptující repozitář to zapne jedním přepínačem v politice a chybějící přepínač není souhlas, protože mod zapisuje do vlastního stromu `.claude/` toho repozitáře.

## Handover se stěhuje mezi stroji

Práce se stěhuje i z jednoho stroje na druhý a handover se dřív na jiný stroj dostal jen přes živé spojení meshe. Teď ho lze zveřejnit jako záznam, který nese jen přenositelný stav ([ADR 0105](@/adrs/adr-0105.md), navržené): samotný handover a k němu identitu repozitáře, zařízení, epizodu, větev, commit, změněné cesty a otisk necommitnuté změny, nikdy však její obsah, dále task a jeho rozhodnutí a záznam, na který navazuje. Záznam je identifikován svým obsahem a podepsán meshovým klíčem zařízení. Cestuje ve vlastním refu, zapsaném gitovou plumbingovou vrstvou, takže se nepohne žádná větev, index ani pracovní strom a žádný pull request ho nenese.

Je odmítnut, ne začerněn, když by nesl něco, co cestovat nesmí: přihlašovací údaj známého tvaru, hodnotu tajné proměnné prostředí, cestu do něčího domovského adresáře nebo cestu v repozitáři, která je absolutní nebo z repozitáře vede ven. Odmítnutí jmenuje pole a hodnotu nikdy nezopakuje, a co záznam řekne místo toho, rozhodne jeho autor. Každý záznam projde přijetím, podle schématu, id, podpisu, identity repozitáře a přenositelnosti, dřív, než ho cokoli přečte. Pravidlem je [project.session-continuity-is-portable](@/rules/project-session-continuity-is-portable-1.md).

## Co to stojí

Nic z toho samo nevolá model a nic z toho neshrnuje konverzaci. Stojí to pár souborů za epizodu, briefing na začátku každé z nich, omezený politikou, jeden handover za každé zastavení a tam, kde je mod zapnutý, jeden tah za okno. Získá se tím, že pracovník, který začíná uprostřed cizí práce, se dozví, kde ta práce stojí a jak staré to tvrzení je. Celý životní cyklus, od jedné epizody na jednom stroji po handover mezi dvěma, popisuje [`docs/CONTINUITY.md`](@/docs/continuity.md).
