+++
title = "Automatizace je schopnost"
description = "Vlastní automatizace tohoto repozitáře žila v shellu a každá opakující se třída chyb, kterou o sobě repozitář zaznamenal, v něm měla svůj výskyt. Proč ADR 0069 dělá z nové automatizace typovanou nebo skriptovanou schopnost jediného registru, jak kontrola odmítá shell, který nikdo nedeklaroval, a co ještě postavené není."
template = "page.html"
[extra]
lang = "cs"
translated_from = "automation-is-a-capability/"
source_digest = "178f3e6409fd546a14f4b0cf31fff193302002cde4b749ef9ed15afb14489f82"
related = ["napsano-jednou-odvozeno-vsude/", "teze/", "dokumentace-kterou-lze-odmitnout/"]
[extra.translations]
en = "automation-is-a-capability/"
+++
{% raw %}

[ADR 0069](@/adrs/adr-0069.md) (navržené), datované 15. září 2026, vychází z toho, kde automatizace tohoto repozitáře žila: v shellu, od většiny `scripts/` a `lib/` až po každý behaviorální case. Každá opakující se třída chyb, kterou repozitář o sobě zaznamenal, měla svůj výskyt v shellu: přepínač BSD nebo GNU, který se na runneru chová jinak, pipeline, jejíž počet se ztratí v subshellu, gate, který nedokáže přečíst vlastní vzor a hlásí čistý stav.

Téhož dne vlastník repozitáře rozhodl, že shell bude postupně nahrazen jazykem Rhai, skriptovacím jazykem vestavěným do Rustu, a že Perl nepřichází v úvahu nikdy. Nabízející se výklad, tedy subsystém automatizace s vlastním registrem, je odmítnut: spustitelný program v Rustu už vlastní jeden registr, z něhož se odvozuje každé rozhraní, a pravidlo [project.no-new-nouns](@/rules/project-no-new-nouns-1.md) druhý zakazuje. Rozhodnutí se vejde na jeden řádek: schopnost (capability) smí být skriptovaná a nic dalšího se nepřidává.

## Co shell stál

První cenou byla přenositelnost. Uživatel potřeboval Bash, aby mohl spustit jakoukoli část automatizace, a Perl se do stromu dostal, aniž by si ho kdo vybral, přes `shasum`, který je na macOS skriptem v Perlu.

Druhou byl inventář, který nikdo nevedl. Nic neodmítlo nový soubor v shellu a ty stávající popisovala jen zpráva z auditu, přitom migrace konverguje, jen když nový shell přestane přibývat, zatímco se ten starý přesouvá.

Třetí bylo chování, na které se nikdo nemohl zeptat. Schopnost deklaruje svůj vstup a výstup jako JSON Schema, svůj účinek a to, zda ji lze zastavit, a každé rozhraní ukazuje tutéž deklaraci. U skriptu musel fakta o tom, co dělá, kdo ho volá a co předpokládá o platformě, posbírat migrační audit.

## Schopnost smí být skriptovaná

Podle ADR se skriptovaná schopnost deklaruje v souboru vrstvy, ne v kódu v Rustu, a jejím handlerem je skript v Rhai, ne zkompilovaná funkce. Její deklarace je nový druh (kind), který uvádí její vstup a výstup jako JSON Schema, její účinek (`read`, `process state` nebo `repository mutation`) a to, zda ji lze zrušit, a vedle ní leží skript. Běží uvnitř spustitelného programu v Rustu, který je nainstalovaný všude, kde je Majordomus, zatímco Perl, Python a Node byly odmítnuty jako běhová prostředí, která by si uživatel musel nainstalovat.

Skript se k repozitáři dostane jen přes schopnosti, jedinou hostitelskou funkcí, která volá `Context::execute`, takže na každé jeho volání platí validace schématu, maskování tajných hodnot, cache, čítače i rovina provádění (execution plane). Kde potřebuje něco, co žádná schopnost nenabízí, je odpovědí typovaná schopnost v Rustu, ne větší skript, a žádná funkce nepřijímá řetězec s příkazem pro shell.

Engine je sandbox, ne `eval`, který zakazuje pravidlo [project.no-network-no-eval](@/rules/project-no-network-no-eval-1.md). Skript je sledovaný soubor, kompiluje se z toho souboru a nikdy z řetězce, který dodal volající; vlastní `eval` jazyka Rhai je vypnutý, engine nemá žádnou vlastní funkci pro soubory, procesy ani síť, jeho práce je omezená a zrušení se kontroluje při každé operaci. Znalostní báze repozitáře k tomu dodává, že jde o hranici jazyka, ne o izolaci procesu.

## Jedna deklarace, každé rozhraní

[ADR 0004](@/adrs/adr-0004.md) (přijaté) udělalo z jedné deklarace celou definici schopnosti: každá projekce se z ní odvozuje a nic se nepíše dvakrát. Skriptovanou schopnost objeví index jako kterýkoli jiný objekt vrstvy, ta se stane schopností jediného registru a dostane se do příkazové řádky, do HTTP routy, do operace OpenAPI, do nástroje MCP, do formuláře runneru v Cockpitu i do generované reference. Přidání ji nikde neregistruje a skriptovaná schopnost dluží totéž co zkompilovaná: definici platnou podle schématu, test, benchmarkové případy, generovanou dokumentaci a gate pokrytí.

Kontrola, která hlídá migraci, takhle funguje už dnes. `shell.check` je deklarovaná jednou v Rustu a tentýž verdikt vydává jako `majordomus shell check`, jako nástroj MCP `majordomus_shell_check` i na `GET /api/v1/shell/check`.

## Inventář a jeho výjimky

Migrace se sleduje v `.ai/repo/automation/inventory.jsonl`, jeden objekt JSON na řádek, v kanonickém pořadí. Obsahuje každou jednotku, kterou migrační audit našel, od souborů v shellu a awk, jq a JavaScriptu, které volají, až po shell vložený do workflow, receptů a hooků. Každý záznam nese zařazení (disposition) a jeho důvod: A přechází do typované schopnosti, B se stává skriptovanou schopností, C je lepicí kód bootstrapu, D je mrtvý kód, E je generovaný kód a F je adaptér platformy.

Soubor v shellu pod hlídaným adresářem musí navíc nést výjimku, která říká, proč dnes musí být v shellu a za jaké podmínky bude odstraněn, a která se při review čte jako celý argument pro ten soubor. Výjimku má i `bin/majordomus`, vstupní bod shellového nástroje: smaže se, jakmile chování převezme typovaná schopnost a všichni volající se budou ptát jí.

Nic mechanického se nezaznamenává, protože počty řádků a interprety kontrola odvozuje ze stromu a zaznamenaná kopie by nepozorovaně zastarala. Výjimky žijí přímo v inventáři, protože druhý seznam by byl opakovanou definicí, kterou ADR 0004 nazývá vadou. Seznam se jen zmenšuje: záznam migrované jednotky se maže ve stejné změně jako jednotka sama.

## Nový shell se odmítá

Bootstrap, který čte každý worker, to říká rovnou: nová automatizace repozitáře je typovaná nebo skriptovaná schopnost, ne shell, a `majordomus shell check` odmítne jednotku v shellu, kterou `.ai/repo/automation/` nevyjímá. Jednotka v shellu je soubor pod `bin/`, `lib/`, `scripts/`, `share/`, `.githooks/` nebo `.claude/hooks/`, jehož první řádek jmenuje shell nebo jehož jméno končí na `.sh`, a kontrola čte soubory, které by git commitnul, takže nový skript je odmítnut dřív, než se přidá. Její nález jmenuje soubor a jako nápravu nabízí samo rozhodnutí: skriptovanou nebo typovanou schopnost, nebo tam, kde je shell opravdu nutný, třeba u instalátoru spouštěného přes `curl | sh`, záznam s výjimkou.

Smazání migrovaného souboru bez jeho záznamu selže jako `shell.stale`, a to je ráčna (ratchet), která nedovolí seznamu přežít jednotky, jež popisuje. Kontrola čte index gitu a první bajty každého souboru, nic nebuilduje a nepoužívá síť, a exit `12`, tedy strom, který nedokázala změřit, není úspěch. V CI je to gate `shell-inventory`, typovaný Rust, protože nový gate v shellu je přesně to, co odmítá.

Shell, který zůstává, drží pravidlo [project.portable-shell](@/rules/project-portable-shell-1.md), aktivní a blokující: spodní hranicí je bash 3.2 a userland BSD, bez asociativních polí, bez `mapfile`, bez přepínačů jen pro GNU a s `shellcheck -x -s bash`, který projde na každém skriptu. Nástroj běží na Macu stejně často jako na linuxovém stroji a konstrukce, která funguje jen na jednom z nich, je podle slov pravidla chyba, která se ukáže jen tam, kde se nikdo nedívá. O pravidle nerozhoduje žádný příkaz; rozhoduje recenzent a v CI k tomu běží shellcheck.

## Co ještě postavené není

ADR 0069 je navržené a jeho práce přistává jako jednotlivé části milníku `automation`. Kontrola a inventář se na master dostaly v den, kterým je ADR datované, jeden case od 30. září odmítá Perl všude, kde repozitář něco spouští, a sandboxovaný engine přistál s testem pro každou ze svých záruk.

Zatím se nespouští žádný skript. Modul enginu říká, že ho ke schopnosti připojí příští změna milníku a že do té doby ho sestavují jen jeho testy, a každá výjimka u jednotky orchestrace říká, že ji zatím žádná skriptovaná schopnost nespouští. Tato změna, která ze skriptované schopnosti udělá registrovaný druh spustitelný přes `majordomus run` a promítaný jako kterýkoli jiný, je v plánu připravená a čeká na ni první migrace gates ze `scripts/ci`. Do té doby slib ADR, že automatizaci spustí samotný nainstalovaný spustitelný program, popisuje, kde milník končí, ne kde stojí.

ADR 0004 je přijaté, ADR 0069 navržené a project.portable-shell je aktivní a blokující. Jak se jedna deklarace dostane do každého rozhraní, popisuje [Napsáno jednou, odvozeno všude](@/napsano-jednou-odvozeno-vsude.md), proč je Majordomus vrstva, a ne další agent, vysvětluje [Sázka na vrstvu, ne na model](@/teze.md), jak repozitář odmítá vlastní dokumentaci, ukazuje [Dokumentace, kterou lze odmítnout](@/dokumentace-kterou-lze-odmitnout.md) a kontrolu s inventářem popisuje [`docs/SHELL.md`](@/docs/shell.md).
{% endraw %}
