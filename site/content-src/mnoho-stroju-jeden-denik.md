+++
title = "Mnoho strojů, jeden deník"
description = "Board peerů žije v paměti jednoho procesu, takže ho session na jiném stroji nepřečte. Jak mesh Majordomus připouští spojení mezi runtimy podepsaným handshakem, jak replikuje sessions, claimy, handovery a review jako podepsané události jednoho deníku, který každý runtime skládá stejně, a jak nechává claim vypršet s heartbeatem jeho držitele."
template = "page.html"
[extra]
lang = "cs"
translated_from = "many-machines-one-journal/"
source_digest = "1b199fc725a783a1a9a6d70735ed621ebfc5e0fe7ed4832113c3ff70d80121f1"
related = ["kresla-ne-jmena/", "zabrano-driv-nez-se-stavi/", "session-skonci-prace-ne/", "pochybnost-rozhodne-experiment/"]
[extra.translations]
en = "many-machines-one-journal/"
+++

V polovině září 2026 už mesh uměl najít jiné stroje. [ADR 0050](@/adrs/adr-0050.md) (navržené) mu dalo objevování (discovery): podepsaná ohlášení, jeden registr a důvěru jako štítek, a záměrně nic víc. Podle měření z 15. září mesh nedokázal unést produkt, za který byl vydáván, a [ADR 0067](@/adrs/adr-0067.md) zaznamenává proč.

| co se naměřilo | proč |
|---|---|
| nic kooperativního nepřekročilo hranici stroje | claimy žily na boardu peerů jednoho serveru a handovery pod `.ai/local/` jednoho checkoutu; žádná událost, session ani review nikam necestovaly |
| jeden repozitář na dvou strojích byl dvěma repozitáři | ohlášení nesla otisk cesty ke společnému adresáři gitu a cesta checkoutu na macOS se nikdy neshodovala s linuxovou cestou téhož klonu |
| přítomnost byla zahlédnutí, ne vztah | uzel byl přítomný šedesát sekund po ohlášení, ať už se s ním dalo něco vyměnit, nebo ne |

## Board zůstává ve svém procesu

Board peerů už spolupráci modeloval správně, jenže jeho dosahem byl jeden proces. [ADR 0044](@/adrs/adr-0044.md) (přijaté) sbírá boardy checkoutů jednoho stroje přes jejich lease soubory a board záměrně zůstává v paměti. Posílat ho na jiné stroje se odmítlo: je poziční, takže se `p1` používá znovu, žije v paměti a není podepsaný, a jeho replikace by replikovala i jeho nejednoznačnosti. Co se postavilo místo toho, řídí dvě věty v [`docs/MESH.md`](@/docs/mesh.md). Objevování dává povědomí, ne autoritu. Spojení (link) se připouští handshakem a všechno kooperativní je podepsaná událost jednoho deníku (journal), který každý runtime skládá (fold) stejně.

## Důvěru stroji dává commit

Uzel (node) je klíč Ed25519 vytvořený jednou pro uživatele a stroj a jeho id je otisk toho klíče, nikdy adresa; runtime je server jednoho checkoutu na tom uzlu. Repozitář je pojmenován otiskem svých kořenových commitů, které sdílí každý úplný klon, a runtimy různých repozitářů se nikdy nepropojí.

Ve výchozím stavu se nevěří nikomu: pod `deny_unknown` je platný neznámý klíč pozorován, vypsán a nevěří se mu v ničem, a stroj, jehož vlastní klíč na seznamu není, nechává mesh zavřený. Přidat stroj znamená spustit na něm `majordomus mesh identity` a commitnout jeho klíč pod `trust.allow`. Pracovní strom smí důvěru zúžit, nikdy rozšířit, takže větev checkoutnutá kvůli review nedokáže checkout přimět, aby věřil klíči jejího autora.

Důvěra je předpoklad spojení, ne spojení. Volající strana pošle podepsané hello a odpovídající strana mimo jiné ověří jeho podpis, časové razítko, které se od jejích hodin neliší o víc než 300 sekund, rozsah protokolu, identitu repozitáře a politiku důvěry, a až nakonec to, jestli už na tentýž nonce někdy odpověděla. Odpověď welcome se ověřuje stejně a každé selhání je typované odmítnutí, započtené a vypsané.

## Jeden deník, složený všude stejně

Všechno kooperativní je událost jednoho deníku: session otevřená nebo zavřená, claim získaný nebo uvolněný, handover publikovaný nebo převzatý, review vyžádané, zodpovězené nebo stažené. Každá událost nese svůj stream, pořadové číslo a Lamportovo razítko a je podepsaná klíčem svého původce, takže prostředník (relay) může událost zadržet, ale nemůže ji podvrhnout ani změnit. Doručuje se alespoň jednou (at-least-once) a duplikát nic nezmění. Uvnitř streamu se události uplatňují podle pořadových čísel, napříč streamy podle Lamportova razítka, streamu a pořadového čísla, a property test drží, že pořadí příchodu stav nikdy nezmění. Kolo synchronizace porovná značky (marks) a přenese jen to, co druhé straně chybí. Každý runtime se stejnými událostmi a stejnými verdikty o živosti spočítá stejný stav a stejný otisk.

Mezi stroji putují sessions, claimy, handovery a review a manuál nezaručuje replikaci ničeho jiného; session sdílí metadata, třeba svůj záměr, větev a head, nikdy tělo kontextu. Board plní deník při každém heartbeatu: připojená MCP session se stane mesh session `board-<peer>` a každé oznámení poradním (advisory) claimem. Žádné rozhraní si nedrží vlastní peery, spojení ani claimy. Jinak by podle pravidla [project.mesh-cooperation-is-authenticated](@/rules/project-mesh-cooperation-is-authenticated-1.md) Cockpit nakonec ukazoval zelenou tečku u runtimu, o kterém CLI ví, že je pryč.

## Claim žije z tepu

Claim je množina cest relativních k repozitáři, kterou drží session; je buď exkluzivní, což je výchozí stav, nebo poradní. Poradní claim, třeba oznámení z boardu nebo rozsah (scope), který pro task claimuje `majordomus start`, hlásí překryvy a nic neodmítá. Exkluzivní claim, který se potká s živým exkluzivním claimem jiné session, o níž runtime ví, je odmítnut jako `claim_conflict` a odmítnutí jmenuje claimy, se kterými se potkal. Dva exkluzivní claimy vzniklé tam, kde se oba runtimy navzájem neviděly, se do deníku dostanou oba a fold na každém runtimu jmenuje téhož vítěze, totiž claim získaný s nejnižším Lamportovým razítkem, streamem a pořadovým číslem. Pravidlo tomu říká slib produktu: exkluzivní claim vylučuje všude, nebo nikde.

Claim platí, dokud není uvolněný, jeho session je otevřená a tep (beat) jeho streamu stoupl v rámci lhůty vypršení; to je celý nájem (lease). Každý runtime při každém heartbeatu, ve výchozím stavu každých pět sekund, zvýší svůj vlastní podepsaný tep a kola synchronizace přenášejí tep každého streamu i s jeho stářím, takže spadlý runtime přestane tepat pro všechny naráz. Čerstvost se posuzuje na vlastních monotónních hodinách každého runtimu, nikdy porovnáním systémových hodin dvou strojů. Lhůta vypršení je ve výchozím stavu třicet sekund a znovu načtený deník obnoví události, ale žádnou čerstvost, takže restart nikomu nevzkřísí vlastnictví.

Manuál to dokládá záznamem z 15. září, pořízeným mezi MacBookem Pro a linuxovým strojem lundra. Claim zapsaný na Macu platil o půl sekundy později i na stroji lundra. Když byl server na stroji lundra zabit přes `kill -9`, Mac nechal spojení vypršet po desetisekundové lhůtě toho běhu, claim stroje lundra se u něj objevil jako `expired: its runtime stopped beating` a Mac pak mohl ten rozsah claimnout sám.

## Handovery a review jdou po spojení

`mesh handover publish` vezme nejnovější záznam, který `majordomus handover` zapsal pod `.ai/local/state/handovers/`, nebo záznam tam jmenovaný, nic jiného, a udělá z něj událost deníku s jeho taskem, větví, headem, časem a tělem v Markdownu, bez jakékoli cesty publikujícího stroje. Na jiném stroji `mesh handover consume <id> --session <s>` zaznamená převzetí a zapíše handover do adresáře handoverů toho checkoutu, takže ho `majordomus handover --resolve` na té větvi najde. Je to jediný soubor, jehož zápis může vzdálená událost vyvolat, a jen výslovným místním příkazem; nic, co peer pošle, nic nespouští.

Review vyžádané na jednom stroji se zodpoví na jiném. Žádost skončí jedním ze tří způsobů: jiná session ji zodpoví verdiktem, runtime, který se ptal, ji stáhne, nebo vyprší, protože nikdo neodpověděl a ptající se runtime přestal tepat. Zavření session, která se ptala, ji neukončí, protože žádost lane přežije mnoho spojení.

## Co mesh nedělá

Není to služba zámků se silnou konzistencí: rozdělení sítě (partition) dovolí claimovat oběma stranám a konflikt se pojmenuje až po zacelení. Není to synchronizace souborů, protože zdrojový kód patří gitu. Nic se nešifruje, takže pozorovatel v síti vidí záměry sessions, rozsahy claimů, předměty review a těla publikovaných handoverů propojených peerů, a průchod přes NAT neexistuje. Kompromitovaný důvěryhodný peer může pod vlastní identitou publikovat zavádějící claimy a handovery a věří se jim, dokud s ním nevyprší.

Nic z toho není zapnuté ve výchozím stavu: kostra, ze které nový repozitář začíná, žádnou deklaraci meshe nenese a tenhle repozitář commitnul vlastní ([ADR 0059](@/adrs/adr-0059.md), navržené). ADR 0067 je pořád navržené; pravidlo, které ho nese, je aktivní a blokující a drží ho `scripts/ci/mesh-check`, testy, které propojují samostatné serverové procesy přes TCP, a laboratoř linuxových kontejnerů, která síť dělí a uzly zabíjí.

Bootstrap, který repozitář generuje pro své workery, to shrnuje: práci, která se napříč stroji nesmí překrývat, claimuj exkluzivně přes `majordomus_mesh_claim`, předej ji přes `majordomus_mesh_handover_publish`, převezmi přes `majordomus_mesh_handover_consume`, o review požádej přes `majordomus_mesh_review_request` a žádný jiný koordinační kanál nevymýšlej. Jak claim drží roli, popisují [Křesla, ne jména](@/kresla-ne-jmena.md), proč session claimuje dřív, než staví, vysvětluje [Zabráno dřív, než se staví](@/zabrano-driv-nez-se-stavi.md) a celý mesh popisuje [`docs/MESH.md`](@/docs/mesh.md).
