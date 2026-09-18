+++
title = "Dokumentace, kterou lze odmítnout"
description = "Tvrzení, které nemůže spadnout, je jen dobře naformátovaná próza. Jak jsem donutil repozitář odmítat vlastní dokumentaci, co to dneska dokazuje a kde to přestává platit."
template = "page.html"
[extra]
lang = "cs"
alternate = "proof-carrying-documentation/"
alternate_lang = "en"
+++
{% raw %}

Nejdražší věc na práci s coding agenty není model. Je to to, co se stane každé ráno, než vůbec něco napíšeš.

Otevřu novou session. Agent neví nic. Neví, proč je ta abstrakce rozdělená zrovna takhle, protože důvod je v diskuzi u pull requestu před třemi měsíci. Neví, že tenhle adresář se negeneruje ručně, protože to je v hlavě jednoho člověka. Neví, co se minule nepovedlo. Přečte si soubory, udělá si vlastní představu — obvykle rozumnou a obvykle jinou, než jaká je pravda — a začne pracovat.

Za dvacet minut jsem to zjistil a opravil. Zítra znovu. A to je ta levnější varianta. Ta dražší je, když to nezjistím.

## Co se mi dělo pořád dokola

Časem jsem si ty poruchy přestal brát osobně a začal je psát. Vypadaly takhle:

**Dokumentace lže a nic se nerozbije.** V README stojí, že CLI a HTTP API nabízejí totéž. Nabízely, půl roku zpátky. Pak někdo přidal endpoint a neměl důvod sahat na CLI. Věta v README zůstala pravdivě vypadat. Žádný build kvůli ní nespadl, protože prosa nemá návratový kód.

**Pravidla existují jako text, ne jako rozhodnutí.** Soubor s instrukcemi pro agenta roste, protože do něj každý přidá to, co se minule pokazilo. Vznikne seznam jizev. Nikdo ho celý nečte a nikdo neumí říct, kolik z těch řádků ještě platí.

**Kontext se ztrácí mezi sessions.** Rozhodnutí, proč se něco udělalo takhle, je rozprostřené přes git history, tři markdowny, issue tracker a konverzaci, která už neexistuje.

**Víc agentů pracuje na nekonzistentním stavu.** Dva si sáhnou na stejný subsystém, protože ani jeden se nepodíval, co dělá ten druhý — a neměl kam se podívat.

Společný jmenovatel: **tvrzení o projektu nikde nemá formu, kterou by šlo odmítnout.**

## Proč "dát agentovi víc kontextu" ten problém neřeší

První reakce je zřejmá: napiš agentovi konvence do souboru, který si vždycky načte. Funguje to asi týden.

Neselže to kvůli kapacitě okna. Selže to strukturálně. Konvence zapsaná v souboru, který nic nevynucuje, má dva stavy — dodržená a porušená — a ten soubor sám neumí říct, ve kterém z nich zrovna jsi. Takže se přidává další řádek. A reálné chování repozitáře se mezitím od toho textu vzdaluje, protože zjistit rozdíl by znamenalo přečíst celý soubor a proauditovat proti němu celý repozitář. Což je přesně ta práce, na kterou nikdy není čas.

Pravidlo, o kterém nemůže rozhodnout žádný příkaz, je pravidlo, ve které doufáš.

Otázka tedy není „jak dostat do agenta víc kontextu". Otázka je: **která z těch vět jde udělat rozhodnutelnou — a co udělám s těmi, které nejdou.**

## Co je Majordomus

Majordomus je řídicí vrstva nad repozitářem, na kterém pracují lidé a coding agenti dohromady. Prakticky jsou to dva programy.

Shellový nástroj `majordomus` drží lifecycle práce: na začátku session dá briefing o tom, co repozitář ví, zapisuje rozhodnutí a handovery, kontroluje změny proti deklarovanému scope úkolu a odmítne `finish`, jehož závazky nejsou splněné.

Rust binárka indexuje vrstvu `.ai/` — pravidla, rozhodnutí, milestones, issues, use cases, tvrzení, záznamy sessions — do typovaného grafu objektů a ten graf pak nabízí přes příkazovou řádku, HTTP API, OpenAPI dokument, MCP server a webový Cockpit.

To, co Majordomus spravuje, **nejsou zdrojáky**. Na ty existují kompilátory a testy. Je to vrstva nad nimi: co projekt slibuje, jaká pravidla svazují změnu, která rozhodnutí jsou uzavřená, čeho se session nesmí dotknout a co z toho je čím doložené.

Ta vrstva existuje v každém projektu. Obvykle je rozsypaná přes CONTRIBUTING, pár stránek v Notionu, diagram, který přestal platit při předminulém refaktoru, a paměť toho, kdo je v týmu nejdéle.

## Jak to funguje pod kapotou

Architektura z té otázky přímo plyne: jedna kanonická deklarace, z ní odvozené povrchy, a gate všude tam, kde by odvození mohlo tiše zestárnout.

<pre class="mermaid">
flowchart TD
  A[kanonický stav&lt;br/&gt;.ai/ pravidla, rozhodnutí, issues, tvrzení] --&gt; B[typovaná discovery&lt;br/&gt;jeden index, jeden graf objektů]
  B --&gt; C[registry capabilit&lt;br/&gt;deklarovaná v Rustu]
  C --&gt; D[HTTP API + OpenAPI]
  C --&gt; E[MCP tools a resources]
  C --&gt; F[Cockpit]
  C --&gt; G[generovaná dokumentace]
  C --&gt; H[příkazová řádka]
  B --&gt; I[validátory a gates]
  I --&gt; J[odmítnutí + příkaz, kterým ho zopakuješ]
</pre>


Registry je ta část, která se zaplatí sama. Každá operace je deklarovaná jednou, v Rustu, včetně identity, efektu a toho, kde má být vidět. Dneska v ní je 1482 capabilit: 110 z nich je vystavených jako MCP tools, 1410 jako MCP resources, 112 jako HTTP operace. Všechny povrchy se renderují ze stejných řádků, takže operace nemůže existovat v API a chybět v OpenAPI dokumentu. Gate to přegeneruje a spadne, pokud se výsledek liší od toho, co je zacommitované.

A teď to, co bych v marketingovém textu vynechal: **CLI odvozené není.** V grafu příkazů je 195 příkazů a 64 z nich nemá pod sebou žádnou capability — nástroj to o sobě sám hlásí řádkem `commands no capability claims: 64`. CLI je druhá deklarace, která se proti té první kontroluje, ne z ní odvozuje. Než se ta mezera zavře, platí „jedna deklarace, všechny povrchy" pro API, MCP a dokumentaci a je to předsevzetí pro CLI.

## Tvrzení jako objekt, ne jako věta

Tohle je mechanismus, kvůli kterému ten článek píšu.

Každá věta o schopnostech na veřejném webu projektu pochází z jednoho souboru, `docs/CLAIMS.yaml`. Tvrzení je v něm typovaný objekt: id, jedna věta, kanonický soubor definující chování, soubor, který to implementuje, test, který to dokazuje, a status. Statusy jsou v tom souboru deklarované jako data — guaranteed, advisory, planned, rejected — takže generátor si nemůže vymyslet pátý.

*Guaranteed* znamená: implementováno, a dokazuje to behaviorální test.

Co z toho dělá víc než tabulku, je požadavek, aby ten vztah platil **oběma směry**, a rozhoduje o tom gate. `scripts/ci/claim-proof-check` vezme každé guaranteed tvrzení, otevře test, na který ukazuje, a vyžaduje, aby ten test tvrzení jmenoval zpátky ve své hlavičce. Tvrzení ukazující na test, který o něm nikdy neslyšel, je nález. Pak ještě vyžaduje, aby ten test byl **spustitelný** — shellový case v `test/cases/` nebo crate test v `apps/majordomus-cli/tests/`. Protože tvrzení, jehož důkazem je markdown, není dokázané. Je popsané.

Aktuálně ten check hlásí:

```console
$ scripts/ci/claim-proof-check --strict
guaranteed claims:   0 of 161 name a test that does not name the claim back
runnable tests:      every guaranteed claim names a shell case or a crate test
claim-proof-check: every guaranteed claim names a test that names it back
```

161 guaranteed tvrzení, všechna uzavřená, a baseline výjimek vedle toho checku obsahuje třináct řádků komentářů a nula tvrzení. Na seznamu výjimek nestojí nic.

Stejný tvar platí o patro výš, pro pravidla. Pravidlo je tu verzovaný markdown se strojově čitelným blokem, který deklaruje, **jak** je vynucené. 137 pravidel se rozpadá do deterministického pořadí, 116 má validátor a všech 109 blokujících pravidel ho má. Čtyři pravidla deklarují, že je vynucuje člověk při review — a to je typovaný stav s povinným odůvodněním, ne absence. Ten rozdíl je podstatný: „tohle kontrolujeme okem, protože automat by musel hádat" je obhajitelný inženýrský postoj. „Tohle nikdy nikdo nerozhodl" ve stejném kabátě není.

## Co zatím tvrdit nemůžu

Všechno výše dokazuje, že důkaz **existuje**. Nic z toho nedokazuje, že ten důkaz **proběhl**.

Repozitář má evidence ledger — `.ai/repo/evidence/ledger.json` — postavený na to, aby spojoval tvrzení se zaznamenanými běhy, a tvrzení pak neslo stav jako proven, stale nebo failing místo pouhého „má test". Ten subsystém je implementovaný. V ledgeru je šest běhů. Všech šest má `"origin": "local"`. Nejnovější je šest dní starý.

Když se nástroje zeptám na stav důkazu u všech 176 tvrzení, odpoví: 153 nespuštěno, 14 zastaralých, 9 bez testu. Nula dokázaných. Report pravidel říká totéž svým slovníkem: `recorded: 0, passing: 0`.

Příčina je nudná a konkrétní: žádný CI workflow nevolá `majordomus evidence record`. Suite běží při každém pushi — 224 shellových cases a 1545 Rust testovacích funkcí — a zapíše výsledek do logu, který GitHub při dalším re-runu smaže. Strojově čitelný report testů se nikde nepublikuje; `https://majordomus.dev/reports/tests.json` vrací 404.

Přesné shrnutí současného stavu tedy zní: **tenhle repozitář umí dokázat, že každé guaranteed tvrzení má test, který ho jmenuje zpátky, a zatím neumí dokázat, že ten test nedávno prošel.** První půlka je vynucený invariant bez výjimek. Druhá je nezapojený drát.

Mohl jsem s článkem počkat, až ledger naplním, a napsat čistší příběh. Myslím, že ta mezera je poučnější artefakt než hotová verze. Je to přesně ten typ polorozestavěného mechanismu, ze kterého se — když se o něm mlčí — stane za rok další věta, která nemůže spadnout.

## Konkrétní průchod: pravidlo, které odmítlo mě

Nejlepší důkaz, že ta governance vrstva je spustitelná a ne dekorativní, je ten, že mě při psaní tohohle článku odmítla.

Tenhle text má anglický protějšek. Nepřekládal jsem ho — napsal jsem dva články ze stejné důkazní základny pro dvě různá publika. Abych publikoval ten český, musel jsem do repozitáře vložit soubor s českou prózou.

Repozitář má pravidlo `project.english-only` a jeho znění bylo: *„Code, comments, commits, documents and governance records are written in English, with no exceptions."* Je to blokující pravidlo a na rozdíl od většiny takových vět ve většině repozitářů je napojené na něco konkrétního. `scripts/ci/english-only-check` projde každý verzovaný autorský soubor a hledá písmena, která se vyskytují v češtině, slovenštině nebo polštině a v angličtině nikdy, plus celá nelatinková písma — porovnává je jako sekvence UTF-8 bajtů, takže výsledek nezávisí na locale stroje. Obsahový strom webu je v záběru. V tomhle stromu neexistuje baseline dluhu, takže jediný zásah shodí gate.

Check má allow list se dvěma druhy záznamů: `name` pro vlastní jména a `fixture` pro soubory, které cizí řetězec používají jako testovací data. Oba se deklarují s odůvodněním. A oba mají v hlavičce checku napsáno: *neither exempts a sentence.* Ani jeden neomlouvá větu.

Takže ten český článek nešlo přidat. Ne „nebylo by to hezké" — **nešlo**: gate by zčervenal a dopsat `fixture` řádek, aby ztichl, by bylo zneužití výjimky, jejíž vlastní dokumentace přesně tohle zakazuje.

Udělal jsem místo toho to, co repozitář na změnu vynucovaných pravidel předepisuje. Zapsal jsem rozhodnutí (ADR) s rozlišením, které původní pravidlo nemuselo dělat, protože když vznikalo, byl každý artefakt v repozitáři pracovní artefakt. Publikovaný článek adresovaný čtenáři v jeho jazyce je jiná kategorie než komentář, commit message nebo governance záznam. Pravidlo jsem zvedl na verzi 2 s tím rozlišením ve znění, přidal třetí druh výjimky, `translation`, který vyžaduje cestu v publikovaném obsahu, deklarovaný jazyk a odůvodnění — a který si deklaraci ověřuje proti jazyku, ke kterému se hlásí sama stránka, aby ta výjimka nebyla jednostranná. Pak jsem napsal behaviorální case, který dokazuje, že upravený gate pořád odmítne českou větu v běžném zdrojovém souboru a pustí ji jen tam, kde to nový druh dovoluje.

Zajímavý je ten tvar, ne to konkrétní pravidlo. Změna toho, co repozitář vynucuje, stála záznam rozhodnutí, zvýšení verze, změnu checku a test. Nic z toho nebyl rituál: bez kteréhokoli z těch kroků by změna neprošla, protože gate, který čte pravidla, by našel, že deklarované vynucení neodpovídá implementaci.

Tohle myslím spustitelnou governance. Ne že někde stojí, že na pravidlech záleží — ale že se kolem nich nedostane ani autor.

## Co z toho vychází

**Gate mění povahu dokumentu.** Než vznikl `claim-proof-check`, byl soubor s tvrzeními marketingový inventář, který měl náhodou sloupec „test". Potom znamenalo přidat větu na web napsat test — a cestou nejmenšího odporu se stalo snížit status tvrzení, ne ho nafouknout. Soubor se nezměnil. Změnil se důsledek toho, že do něj člověk píše.

**Většina governance dluhu je jednostranná.** Opakovaná vada, kterou nacházím ve vlastní práci, je check ověřující vztah jen z jednoho konce: tvrzení jmenuje test, ale nic nevyžaduje, aby test jmenoval tvrzení. Jednostranné checky vypadají jako vynucení a nejsou jím, protože ta polovina, kterou nečtou, je přesně ta, která hnije.

**Rozdíl mezi „deklarováno" a „zaznamenáno" je místo, kde bydlí poctivost.** Postavit systém, který ví, co slibuje, je snadné. Postavit systém, který ví, kdy to naposledy ověřil, je výrazně těžší. Mám to první a zatím ne to druhé, a pojmenovat tu hranici přesně — ve výstupu nástroje, na veřejném webu i v tomhle textu — má větší cenu, než kdybych ji potichu zavřel.

## Důkazy

Všechna čísla výše jsem změřil v září 2026, na commitu, ze kterého je tahle stránka publikovaná — jmenuje ho `build.json` na tomhle webu — a u každého je příkaz, který ho vypsal. Repozitář je veřejný.

- Počty capabilit a projekcí, nepokryté příkazy: `majordomus capabilities projections --format json`
- Počty tvrzení a stavy důkazů: `majordomus evidence show --format json`
- Uzavřenost tvrzení: `scripts/ci/claim-proof-check --strict`, baseline `.ai/repo/claim-proof-baseline.txt`
- Pravidla a způsoby vynucení: `majordomus rules report --format json`, `scripts/ci/rule-proof-check`
- Testy: 224 souborů v `test/cases/`, 1545 funkcí `#[test]`, z toho 374 v `apps/majordomus-cli/tests/`
- Gates: 54 deklarovaných v `.ai/repo/ci/gates.yaml`, 17 běží vždy
- Nasazení: `build.json` na publikovaném webu jmenuje verzi zdroje, commit a generátor
- Pravidlo, které tenhle článek musel změnit: `.ai/repo/rules/project/english-only.v2.md`

Tvrzení samotná jsou publikovaná po jednom, každé se svým testem. Pokud je některá věta na tom webu špatně, existuje konkrétní soubor, který se dá otevřít, a konkrétní příkaz, který měl spadnout.
{% endraw %}
