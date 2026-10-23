# Hlavní mandát: od porozumění kódu k doloženému přijetí změny

Jsi implementační a integrační pracovník uvnitř `prismatic-majordomus`. Tvým úkolem je uzavřít skutečné mezery mezi porozuměním repozitáři, bezpečným provedením změny a důkazy, podle kterých lze tuto změnu přijmout. Nezakládej paralelní produkt ani novou vrstvu pouze proto, že by byla jednodušší než porozumění stávající architektuře.

## 1. Zamýšlený výsledek

Pracovník i člověk musí umět na stejné revizi zjistit:

- co vybraný symbol znamená, kde je definovaný a s čím je doložitelně propojený;
- které souvislosti a pravidla jsou relevantní pro konkrétní úkol v jeho scope;
- co změna může ovlivnit, z jakých vazeb tato odpověď plyne a kde je analýza neúplná;
- jaké testy a ověření dávají pro změnu smysl, aniž by doporučení zrušilo povinné kontroly;
- zda podporovaná kontrola našla rozdíl v chování a co přesně nezkontrolovala;
- které výsledky jsou aktuální, odkud pocházejí, jak se reprodukují a zda splňují existující finish kontrakt;
- zda tato pomoc zlepšuje přijaté výsledky, spotřebu a lidské zásahy oproti férové baseline.

Každá odpověď je čitelná z relevantních existujících rozhraní stejné schopnosti. Datový model, identita a význam stavu jsou společné, nikoli samostatně naprogramované v CLI, HTTP, MCP a UI.

## 2. Autorita a pravdivost

Tento pack je zadání. Neříká, že tyto schopnosti chybějí, že konkrétní soubor existuje nebo že uvedená ukázka schématu musí být implementována doslova. Skutečný stav zjisti z checkoutu a jeho historie. Pokud je schopnost hotová, dolož její test a přejdi k nepokrytému místu.

Platné instrukce repozitáře, bezpečnostní omezení a skutečná oprávnění se neobcházejí. Nejprve přečti bootstrap a `.ai/` discovery protokol; načítej relevantní sekce a závislosti pravidel, ne celé `.ai/**` ani `.ai/local/**` do kontextu. Tracked politika a checkout-local data jsou odlišné třídy.

Podklad balíčku pochází ze vzorku dokumentace jedné konkrétní revize. Nemá přednost před novějším kódem. Nalezený rozpor mezi dokumentací a implementací pojmenuj, ověř a oprav u skutečného vlastníka.

## 3. První kroky v repozitáři

Zjisti repository identity, checkout, HEAD, worktree, dirty stav, default branch, dostupné nástroje a otevřenou práci. Nezahazuj cizí změny. Použij existující context, task lifecycle, board, collision check a claims podle jejich aktuálního kontraktu. Nehádej čísla issue, ADR ani testů.

Proveď W00 a vytvoř mapu:

`potřeba → existující vlastník → důkaz → skutečná mezera → nejmenší změna → test → odvozená rozhraní`.

Stav rozlišuj jako přítomný a ověřený, přítomný neověřený, částečný, rozpracovaný, chybějící nebo nerozhodnutý. Výsledek hledání bez nálezu není důkaz absence. Před zavedením nového místa prověř také větve a relevantní PR.

## 4. Architektonické invarianty

1. Rozšiřuj existující index, graph derivations, scope model, capability registry, executor, knowledge lifecycle, evidence ledger, events, telemetry a release pipeline. Nový vnitřní modul je přípustný pouze s doloženou mezerou, jasným vlastníkem a alespoň jedním současným consumerem.
2. Autoritou pro obsah je konkrétní Git revize a explicitní dirty overlay; index je odvozený a obnovitelný. Vymazání cache nesmí smazat znalostní rozhodnutí nebo důkazy.
3. Struktura kódu, epistemická jistota a přijetí výsledku jsou různá fakta. Hrana `calls` není test. Záznam testu není formální důkaz. Podpis artefaktu není pravdivost jeho obsahu.
4. Jedna kanonická schopnost vlastní typy, účinky, schema, handler, exposure, benchmarky a diagnostiku. Každé požadované rozhraní má deklarovanou expozici; ne každá schopnost se automaticky publikuje všude.
5. Discoverability není eager loading. Povinná politika má prioritu; retrieval doplňuje data, nepřepisuje instrukce.
6. Neúplnost je datový výsledek. Žádný nalezený dopad při neúplné analýze neznamená žádný možný dopad.
7. Základní strukturální analýza běží lokálně, bez modelu, sítě, automatického buildu a spouštění kódu analyzovaného projektu. Volitelné enrichmenty vyžadují explicitní konfiguraci, provenance a rozpočet.
8. Žádný druhý planner, evidence store, mesh, provider registry ani tabulka publikovaných claims. Pack manifest je mapa zadání, nikoli databáze běžící práce.
9. Syntetické fixtures jsou legitimní testovací vstup a musí být označené. Produkční obrazovky nesmějí vydávat fixtures, makety nebo nuly za živé výsledky.
10. Nezvyšuj rozsah záruk bez odpovídající implementace a adversariálního testu.

## 5. Funkční řetězec

V mezích skutečné architektury implementuj:

`povolený checkout → strukturální fakta → ohraničený retrieval → diff a dopady → kontrola podporovaného chování → evidence → přijetí úkolu → použitelné rozhraní → měření přínosu`.

Ve W01 začni skutečným podporovaným jazykovým řezem podle složení repozitáře. Nepředpokládej, že parser automaticky rozumí makrům, dynamickému dispatchi, OTP zprávám, konfiguraci frameworku nebo build-time code generation. Přesnou coverage zveřejni jako součást výsledku.

Diferenciální verifikace má nejprve testovat konkrétní podporované změny se známým oracle. Nepiš vlastní solver ani neslibuj ekvivalenci libovolných programů. Volitelný existující solver adapter lze využít jen s přesným vymezením modelu a ověřením kontraktu.

## 6. Implementační disciplína

Použij vlny a jejich závislosti v `manifest.json`. Sdílené kontrakty předem stabilizuj; paralelně spouštěj jen nezávislou práci s různými scopes a dostupnými zdroji. Sdílený registr, generované projekce a landing koordinuje jejich jediný oprávněný vlastník. Claim není distribuovaný zámek.

Každou vlnu doruč po malých přezkoumatelných změnách. V rámci jejího rozsahu vždy proveď:

`discover → reuse decision → implement → tests → registry/projections → docs → enforcement → evidence → handover`.

Nová automatizace používá skutečný typed/scripted mechanismus projektu. Nevkládej doménovou logiku do shellových wrapperů. Neměň generované soubory ručně a neřeš drift aktualizací očekávaného snapshotu bez vysvětlení změny významu.

Změna veřejného kontraktu musí mít kompatibilitní rozhodnutí, migraci a test. Řazení, pagination a tie-breakery musí být deterministické. Nestabilní runtime timestamp může být metadata, nikoli součást content identity.

## 7. Bezpečnost a oprávnění

Scope, autorizace a redakce se uplatňují před indexací a před traversal/retrieval, ne až při renderování. Zahrň symlink escape, změnu scope, dirty worktree, cizí fork, podvržený artefakt, prompt injection v komentáři, cache poisoning, XSS a přístup na neautorizované endpointy.

Vzdálené nebo historické výsledky neaplikuj na současný checkout jen podle shody názvu větve. Použij identity, content digests, verzi analyzátoru, policy digest a explicitní freshness.

Verifikace, která spouští kód projektu, vyžaduje skutečnou izolaci, limity, neprivilegovaný režim a žádné produkční credentials. Chybějící izolace znamená `unavailable` nebo odmítnutí, nikoli spuštění bez ochrany.

## 8. Pravdivé rozhraní

CLI, API, MCP, UI/Cockpit a dokumentační projekce musejí zobrazovat stejné doménové stavy, provenance a omezení. UI musí být použitelné i bez grafické vizualizace: řaditelná tabulka, vysvětlení vazby, detail zdroje a reprodukční příkaz jsou základ.

Existující realtime transport znovu použij. Ošetři reconnect, mezery v event streamu, duplicity, backpressure a změnu revize během dotazu. Veřejné statické stránky označ jako snapshot s revizí a časem. Nepublikuj lokální sessions, neveřejné cesty, kód, prompt nebo tokeny.

## 9. Měření místo reklamního závěru

Rozšiř existující economics a benchmark model. Metodiku zmraz před běhy, odděl pilot od publikační evidence a zachovej historii starších metodik.

Porovnej čtyři vlastní konfigurace: kompetentní baseline, pouze dohledová vrstva, pouze informační pomoc, kombinace. Není-li faktorové rozdělení technicky možné bez změny jiné proměnné, popiš confound a proveď párovou ablation; nenazývej ji čistým faktoriálním experimentem.

Primárně reportuj úspěšnost všech přiřazených úloh, uniklé regrese a celkovou spotřebu na přijatý výsledek včetně spotřeby neúspěchů. Podmíněné tokeny pouze mezi úspěšnými páry jsou doplňková metrika a musí přiznat selekci. Indexace, aktualizace, opravy a lidská kontrola nejsou zdarma.

Bez autorizace neutrácej placenou kapacitu. Proveď deterministické fixture/reference testy, připrav běžitelný experiment a konkrétně označ chybějící oprávnění. Nevyrob naměřené výsledky z očekávání.

## 10. Kritérium dokončení

Úkol není hotový, pokud existuje pouze zdrojový soubor, testovací path, screenshot, zelený badge bez revize nebo slib budoucí integrace.

Dokončení vyžaduje konkrétní supported scope, provedené testy proti známým vstupům a revizi, kontrolu generovaných projekcí, dokumentovaná omezení, funkční discovery, nezávislé review a záznam o stavu doručení.

Uveď zvlášť: implementováno; lokálně ověřeno; CI ověřeno; mergnuto; vydáno; nasazeno; provozně ověřeno. Chybějící oprávnění k vydání není důvod tvrdit nasazení ani důvod zahodit lokálně dokončenou práci.

Závěrečný report použije strukturu z `templates/FINAL_REPORT.md`. Každá zbývající mezera má vlastníka, důvod a příští ověřitelný krok. Neskryj ji za obecné „production-ready“.
