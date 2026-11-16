+++
title = "Jedna větev, jedno místo"
description = "Několik sessions na jednom repozitáři potřebuje každá vlastní checkout. Proč tento repozitář nenechává nikoho vybrat, kam worktree patří, jak se to jedno místo odvozuje z gitu a jak se worktree přesune nebo odstraní, aniž by se cokoli ztratilo."
template = "page.html"
[extra]
lang = "cs"
translated_from = "one-branch-one-place/"
source_digest = "1ab3ad1da36211469625c6943721c15c5d267bcd4656a5f332177c05573eea58"
related = ["zabrano-driv-nez-se-stavi/", "session-skonci-prace-ne/", "napsano-jednou-odvozeno-vsude/"]
[extra.translations]
en = "one-branch-one-place/"
+++

Na tomto repozitáři pracuje současně několik sessions, lidí i agentů, a každá potřebuje checkout své vlastní větve. Mechanismem je linked worktree gitu a git bere cíl jako argument, takže o tom, kam worktree přijde, rozhoduje každý volající. Než existovalo pravidlo, rozhodlo se tu padesát tři z nich pokaždé jinak: plochí sourozenci pojmenovaní po ničem, pomocné adresáře sessions, jeden uvnitř primárního checkoutu a jeden na místě samotného kontejneru.

Cenou nebyl nepořádek. Cenou bylo, že otázky „kde je větev X“ a „kam patří nová“ neměly odpověď, která by nevyžadovala hledat, hádat nebo vymýšlet. Session, která si musí cestu vymyslet, nechá svou práci tam, kam se další session nepodívá.

## Jedna odpověď, odvozená z gitu

[ADR 0021](@/adrs/adr-0021.md) (přijaté) to rozhodování odstranilo. Pro daný repozitář a jméno větve existuje právě jedna cesta: sourozenec primárního checkoutu pojmenovaný podle něj s příponou `-wt` a pod ním jméno větve se zachovanou hierarchií. Primární checkout se čte ze sdílených metadat gitu, nikdy z aktuálního adresáře. Odpověď je proto stejná z primárního checkoutu, z propojeného worktree i ze čtyř adresářů hluboko uvnitř něj. Zeptat se místo toho aktuálního worktree je snadná chyba, protože uvnitř propojeného worktree by to vytvořilo kontejner pro každý worktree, vnořovaný donekonečna.

Jméno větve se bere jako vstup. Ověří se pravidly gitu pro reference, u odvozené cesty se při každém volání dokazuje, že leží striktně pod kontejnerem, a dvě jména lišící se jen velikostí písmen se nahlásí, protože souborový systém, který velikost písmen nerozlišuje, by obě dal do jednoho adresáře. Trunk se zjišťuje, není natvrdo zapsaný. Přípona je v kódu zapsaná jednou a žádné nastavení pro ni neexistuje, protože kontejner, který by šel nastavit, je kontejner, na kterém by se dva checkouty téhož repozitáře mohly neshodnout.

Session tedy začíná větev příkazem `majordomus worktree create <branch>` a pracuje v cestě, kterou vypíše. Cesta nikdy není argumentem. Handover může jmenovat větev a session, která v práci pokračuje, najde worktree, aniž by jí kdo musel říkat, kde je.

## Odmítnuto tam, kde na tom záleží

Pravidlem je [project.worktree-topology](@/rules/project-worktree-topology-1.md) a vynucuje se ve vrstvách, které otevřeně říkají, co která zvládne. Bootstrapy providerů říkají každému pracovníkovi, ať si topologii zjistí dřív, než začne implementovat, a životní cyklus tasku začíná dotazem na ni. Pre-commit hook se ptá `majordomus worktree guard`: feature větev se commituje jen ze svého kanonického worktree a primární checkout jen na trunku. `majordomus doctor` dokazuje, že se hook ptá, a topologie je vidět na příkazové řádce, přes MCP a HTTP i v Cockpitu, takže špatná je vidět všude.

Manuál je k hranicím upřímný. Git hook se dá obejít a CI runner nevidí do ničího lokálního disku, takže brána v CI měří mechanismus pravidla, ne adresáře jednoho stroje. Ve workflow, které Majordomus řídí, je špatný worktree odmítnut. Mimo ně je vidět.

## Přesunuto bez ztráty

Worktree, který už je na špatném místě, se vrátí domů. `majordomus worktree migrate --plan` vypíše každý špatně umístěný worktree, kam patří, jak by se přesunul a co tomu brání, a `majordomus worktree migrate` přesuny provede. Každý přesun se ověřuje otiskem pořízeným před ním a po něm: větev, commit, index, změny ve stage i mimo ni a každý nesledovaný soubor s otiskem obsahu. Krok se hlásí jako přesunutý, jen když se oba otisky shodují. Nic se neresetuje, neodkládá do stashe, nečistí, nepřepíná ani nemaže a cíl, který už existuje, je konflikt pojmenovaný podle druhu, nikdy se nepřepíše.

## Odstraněno, jen když je to bezpečné

Dochází disk, a proto se nepotřebné worktrees uvolňují. `majordomus worktree cleanup` vypíše každou větev zamergovanou do trunku, jejíž worktree je čistý nebo chybí, a `--remove` odstraní tyto worktrees a nic jiného. Větve se nemažou nikdy, protože větev je jediné trvalé jméno, které kus práce má.

Výpis jen navrhuje. Těsně před každým odstraněním se stav větve přečte znovu a odstranění se odmítne u větve, která je před svým remote nebo žádný nemá, u worktree, uvnitř kterého pracuje nějaký proces, a u necommitnuté práce. Času poslední změny se nevěří. Dne 15. září 2026 měly dva worktrees, které vypadaly nedotčené od dvanáctého, uvnitř živé procesy.

Cena je jeden příkaz před každou novou větví a cesta, kterou si nikdo nesmí vybrat. Výměnou každá session i každá plocha ví, kde se na které větvi pracuje. Celý mechanismus, od odvození po způsoby selhání, popisuje [`docs/WORKTREES.md`](@/docs/worktrees.md).
