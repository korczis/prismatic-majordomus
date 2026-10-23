+++
title = "Křesla, ne jména"
description = "Devět sessions na jednom stroji, přes sto otevřených pull requestů a primární checkout 779 commitů za vlastním masterem. Proč odpovědí bylo udělat z každé role claim na meshi místo jména, přistávat mnoho pull requestů v jednom posouzeném běhu a dotahovat zastaralou práci bez ptaní."
template = "page.html"
[extra]
lang = "cs"
translated_from = "seats-not-names/"
source_digest = "5b26414f155534e79c5587a3457a7c8faf3d2188b7f4151836050397fc9ad48c"
related = ["vsechno-v-jednom-proudu/", "teze/", "vlozene-zadani-se-stava-planem/", "plan-se-napadne-driv-nez-se-postavi/", "zabrano-driv-nez-se-stavi/"]
[extra.translations]
en = "seats-not-names/"
+++
{% raw %}

Ráno 16. října 2026 bylo na tomhle repozitáři otevřených 107 pull requestů. Pracovalo na něm najednou devět nebo víc AI sessions, na jednom stroji, a způsob, jakým svou práci dostávaly do masteru, jim přestal stačit.

Záznamem toho rána je [ADR 0167](@/adrs/adr-0167.md). Jmenuje tři selhání a každé z nich je naměřené, ne vyargumentované.

<div class="overflow-x-auto" tabindex="0">

| co se stalo | co to stálo |
|---|---|
| každé přistání znovu odsunulo všechny ostatní otevřené heady za master | merge masteru, derive a běh zhruba hodinu na každý pull request, takže fronta rostla rychleji, než ji jeden lander stíhal vyprazdňovat |
| primární checkout byl 779 commitů za masterem | jeho server odpovídal na otázky o plánu z toho starého stromu a jedna session zrušila průchod čtrnácti issues, když zjistila, že třináct z nich je na masteru už hotových |
| vydání čekala na milníky | publikovaný web i binárky nesly verzi, kterou master dávno předběhl |

</div>


Vlastník repozitáře sessions to ráno odpověděl několika řádky v češtině. Rozdělte si práci a spolupracujte. Přistávejte mnoho pull requestů najednou. Všechno aktualizujte, čímkoli, co pomůže. Deployujte a vydávejte často, vždycky. Zdokumentujte to a všechno odvozujte a vynucujte. Zbytek článku je o tom, co se z těch řádků stalo.

## Křeslo je claim

První rozhodnutí se týkalo toho, kdo co dělá. Integrace potřebuje několik rolí, které smí v jednu chvíli držet jen jedna session: někdo přistává, někdo vydává. Nejjednodušší je zapsat to jménem do dokumentu: „vydává session 9c“. Jenže to jméno přestane platit ve chvíli, kdy se session znovu připojí pod jinou identitou, spadne nebo práci předá, a nic si toho nevšimne.

Role je proto **křeslo** (seat) a křeslo se drží tak, že se drží exkluzivní claim na meshi, a nijak jinak. Claim už je podepsaná událost, kterou replikuje každý stroj, která žije jen tak dlouho jako heartbeat její session a kterou jiná session nemůže vzít, dokud ji drží. Pravidlo [project.the-cooperation-model-is-the-owners-word](@/rules/project-the-cooperation-model-is-the-owners-word-2.md) jmenuje čtyři křesla:

<div class="overflow-x-auto" tabindex="0">

| křeslo | claimuje | dělá |
|---|---|---|
| landing seat | `campaign/*/landing` | skládá a přistává kombinované topy, když heady musí vyjít spolu, připnuté k headu, který posoudilo CI, a obsluhuje frontu hlídaného landeru |
| release seat | `release/seat` | zvedá verzi, taguje na zeleném `ci` samotného masteru, zapisuje vydání a po přistáních ověřuje web, kdykoli master drží přistálou práci za posledním tagem |
| repair lane | `refs/heads/*` | merguje master do pull requestu, který je v konfliktu, na vlastní větvi toho pull requestu, jeden claim na větev |
| worker | cesty svého jednoho issue | jedno issue, jeden pull request; svůj zelený head přistane sám přes frontu hlídaného landeru |

</div>


Křeslo se přesune, když se přesune jeho claim, a křeslo spadlého držitele vyprší s jeho heartbeatem. Žádná session si nemusí pamatovat, kdo ho drží. Obrazovka Mesh v UI, `/ui/mesh`, čte živé claimy a kreslí, kdo které křeslo drží teď. Case 3150 a vlastní test té obrazovky drží její tabulku křesel a tabulku v pravidle u sebe, takže ani jednu nejde upravit samotnou.

## Jeden běh posoudí mnoho změn

Druhé rozhodnutí se týkalo toho, jak velké smí přistání být. ADR 0101 zařídilo, že integrace přistává jeden pull request na jeden běh povinné kontroly, a dávkování nechalo na člověku. Vlastník to rozhodl: landing seat přistává **kombinované topy**.

Top je aktuální master s heady všech pull requestů, které nejsou v konfliktu v žádném autorském souboru; rozhoduje o tom řetěz `git merge-tree` od masteru. Odvozené soubory se vygenerují jednou, po mergích, místo jednou na každý pull request. Jeden validační běh posoudí všechny členy. Člen, který běh zbarví do červena, se z topu vyřadí a opraví se na vlastní větvi. Top přistane jedním mergem a forge pak všechny pull requesty, které obsahuje, označí jako mergnuté. Rozhodnutí tu aritmetiku shrnuje jednou větou: top o dvaceti členech stojí jeden běh.

Nic z toho ale nic nepovoluje. Merge je `gh pr merge <n> --admin --merge --match-head-commit <sha>` a je dovolený jen tehdy, když `ci` přesně toho headu skončilo úspěchem a head obsahuje aktuální `origin/master`. Administrátorské přebití se utrácí za strom, který byl posouzený, nikdy za přeskočení posouzení. Kvůli přistání se nevypíná žádný hook ani gate, `--no-verify` se nepoužívá nikdy a tag se pushne jen na commit, jehož `ci` uspělo.

## Worker přistane svůj zelený head sám

První verze dělala z každého přistání práci landing seatu a do 20. října se čekání, které s tím přišlo, stalo úzkým hrdlem: worker předal svůj head a čekal na slovo křesla a buildy čekaly na pořadí slotů. Ten večer vlastník pravidla zmírnil a druhá verze pravidla zaznamenává, jak to křesla přečetla. Worker, jehož pull request je zelený, zapíše jeho číslo do fronty hlídaného landeru a na žádné křeslo nečeká. Lander posune master dopředu jen tehdy, když nejnovější běh `ci` na headu pull requestu skončil úspěchem, ten head je pořád headem pull requestu a obsahuje `origin/master`; všechno ostatní zůstane ve frontě a je jmenované. Head, který musí vyjít s jinými, jde dál k landing seatu jako člen kombinovaného topu. Buildy omezuje stroj, ne pořadí slotů, a úklid, při kterém se nic neztratí, se dělá bez ptaní a ohlásí se na meshi. Nezmírnilo se připnutí: přistání dál potřebuje head, jehož `ci` uspělo a který obsahuje aktuální master, nic nepřeskakuje kontrolu ani hook a cesty i identifikátory se dál claimují dřív, než se postaví.

## Nikdo nečeká, až ho někdo požádá

Třetí rozhodnutí se týkalo zastaralé práce. Checkout nebo větev, která zaostala za masterem, dotáhne na aktuální stav ten, kdo ji najde, a nikoho se neptá. Co drží, se nejdřív uchová: necommitnutá práce se uloží do commitu, na který ještě nic neukazuje (`git stash create`), a připne se pod `refs/rescue/`, takže ji dotažení stromu nemůže ztratit.

Větev, kterou si nikdo jiný nestáhl, smí jít resetovat nebo rebasovat. Větev, která už byla pushnutá, se dotahuje mergem masteru do ní, nikdy jejím přepsáním. Rozhodnutí uvádí proč: force-push je rychlý pro session, která ho udělá, a zničí, co mezitím pushnula jiná session. Rescue ref session, která větev přepsala, to nevrátí, a merge stojí jeden commit.

## Vydávat, kdykoli je co vydat

Čtvrté rozhodnutí se týkalo času. Release seat nečeká na milník. Vydává, kdykoli master drží přistálou práci za posledním tagem, a potom ověří, co publikovaný web servíruje. Za tři dny po tom ránu se zapsala tři vydání: 0.28.0 17. října ve 4:25 UTC, 0.29.0 týž den v 19:47 a 0.30.0 18. října v 17:27.

## Co hotové není

[ADR 0167](@/adrs/adr-0167.md) je pořád `proposed`. Praxe přesto platí, protože pravidlo, které ji nese, je aktivní a blokující. Jeho testy jsou case 3150, 3824, 2999 a 720.

Topy skládá landing seat vlastním postupem, ne přes schopnost repozitáře. Typované `prs compose`, které by členy topu zapsalo jako manifest, nikdy nepřistálo. Dokud se tak nestane, záznamem topu je jeho pull request: jeho popis jmenuje členy a jejich heady a rodiči jeho merge commitu jsou master a top. Executor, který postavilo ADR 0101, dál přistává jeden připravený pull request najednou a nikdy nepředává `--admin`. Kombinovaný top je čin landing seatu, ne executoru.

Rozhodnutí také jmenovalo tři místa vynucení jako práci jiných workerů: nový worktree začíná ze vzdáleného trunku, ne ze zastaralého lokálního masteru; `start` zajistí, že server checkoutu běží, než začne task; a checkout, který zaostal, se pozná a dotáhne. Pravidlo jmenuje každé z nich, jakmile přistane.

Manuálem je [`docs/INTEGRATION.md`](@/docs/integration.md), který na pravidlo odkazuje, místo aby ho opakoval. Mesh, kvůli kterému má smysl claim držet, popisuje [`docs/MESH.md`](@/docs/mesh.md).
{% endraw %}
