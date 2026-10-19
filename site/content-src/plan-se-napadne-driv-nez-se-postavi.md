+++
title = "Plán se napadne dřív, než se postaví"
description = "Na tomhle repozitáři nezačíná práce na intentu proto, že má někdo plán. Začíná, až když byl plán napaden a každý blokující nález má odpověď, kterou si čtenář může ověřit. Co kritiky našly, co bylo zamítnuto a proč, a co kritika nevidí."
template = "page.html"
[extra]
lang = "cs"
translated_from = "a-plan-is-attacked-before-it-is-built/"
source_digest = "552eeebea9eeeb366c563af37a4273f27fbc4dfe1843eb8e625a05231e10c8e7"
related = ["vlozene-zadani-se-stava-planem/", "dokumentace-kterou-lze-odmitnout/", "kresla-ne-jmena/", "napsano-jednou-odvozeno-vsude/"]
[extra.translations]
en = "a-plan-is-attacked-before-it-is-built/"
+++

Plán, který napsal tentýž agent, jenž ho pak provede, má známou slabinu: souhlasí sám se sebou. Kroky vyplývají z předpokladů, předpoklady nikdo neověří a nic v plánu neříká, co by muselo platit, aby byl špatně. Code review přichází pozdě, protože v té chvíli už se z plánu stal kód.

Tenhle repozitář staví mezi plán a práci útok. Intent je typovaný záznam toho, co se má stát pravdou, s kritérii, která může rozhodnout test ([ADR 0070](@/adrs/adr-0070.md)). Issues, které ho uskutečňují, každé jmenuje kritérium, kterému slouží ([ADR 0073](@/adrs/adr-0073.md)). Než kterékoli z nich smí začít, musí být plán podroben kritice a kritika musí dostat odpověď.

## Kritika je záznam, ne rozhovor

Kritika leží vedle intentu, který posuzuje, jeden soubor na intent pod `.ai/repo/project/critiques/`. Zaznamenává commit, u kterého byl plán posouzen, kdo ho posoudil, a seznam nálezů. Každý nález je zařazený podle otázky, kterou klade:

| třída | otázka |
|---|---|
| `missed_requirement` | co plán nedělá, ačkoli to jeho intent vyžaduje? |
| `unproven_assumption` | co bere jako samozřejmé, aniž to kdo ukázal? |
| `insufficient_work` | kde plánované issue končí dřív, než dosáhne svého kritéria? |
| `unnecessary_work` | co staví, ačkoli to nic nežádá? |
| `regression_risk` | co z toho, co dnes funguje, by mohl rozbít? |
| `surface_missing` | který další soubor, schéma nebo stránka se musí změnit s ním? |
| `delivery_verification` | co je potřeba vidět běžet, nejen napsané? |

Každý nález je buď blokující, nebo ne. Každý se vyřeší jedním ze tří způsobů: `planned` do pojmenovaného issue, které slouží intentu, `rejected` s důvodem, nebo zůstane `open`. Manuál plánování k tomu říká jasně: „Zamítnutí, které si nikdo nemůže přečíst, není vyřešení.“

## Gate ho čte

Záznam by byl ozdobou, kdyby ho nic nečetlo. Než task začne, `majordomus intent preflight` se ptá, jestli práce smí pokračovat. Odmítne s `intent_not_critiqued`, když intent, kterému issue slouží, nemá kritiku, a s `open_blocking_finding`, když je některý blokující nález té kritiky pořád otevřený. Kde politika repozitáře nastaví `intent.binding` na `required`, takové issue se nikdy nestane aktivním. Práci, která přesto začala, zachytí později `intent validate` a gate `intent-check` jako `executing_without_critique` nebo `executing_with_open_blocker`.

Co repozitář nedělá, je psát kritiku. Manuál to říká přímo: „Majordomus tyto záznamy posuzuje, nepíše je.“ Kritizuje člověk nebo agent. Repozitář výsledek odmítne, když nedrží pohromadě.

## Co kritiky našly

Jedna kritika, a to plánu, který svázal každý task s intentem, ukazuje, co to přináší. Governance toho plánu citovala ADR 0111, které neexistovalo v žádném stromu, a nevyřešený odkaz shodí `intent validate`. Nález byl naplánován: ADR se stalo výstupem issue I2000 a přistálo spolu se záznamem intentu. Jiný nález byl vážnější. `plan start` a capability `plan.transition` uměly posunout issue do stavu active úplně bez preflightu, takže gate u `start` šlo jednoduše obejít. To bylo naplánováno do issue, které hlídá přechod do startu v obou plánovacích enginech.

Ne každý nález je přijatý. Tentýž požadavek chtěl, aby zastaralá kritika zablokovala start. Nález, že to plán nedělá, byl zamítnut a důvod je zapsaný v záznamu. Kritika zaznamenává commit, u kterého vznikla, ne revizi plánu, takže nebylo s čím zastaralost porovnat. Pozdější milník dává záznamu tu revizi a intent tohle omezení jmenuje jako non-goal. Užitečné je právě to zamítnutí: čtenář s ním může nesouhlasit, protože říká proč.

A ne každý nález musí být uzavřený, než práce začne. Neblokující nález o intentu plánování konstatuje, že čtenář majordomus.dev plánovací polovinu neuvidí, dokud se nenasadí a neověří generovaná dokumentace. Pořád je `open` a to je v pořádku, protože nic neblokuje a říká to.

## Kdo útočí

Záznam říká, kdo ho napsal, v poli `reviewed_by`, a odpovědi nejsou všude stejné. Některé kritiky napsal čerstvý subagent. Dostal záznamy plánu, požadavek a kód, ale nic z autorových úvah. Jeden z těch záznamů přidává větu, kterou stojí za to citovat celou: recenzent byl „tentýž model a tatáž operátorská session jako autor, tedy nezávislý na kontextu, ve kterém plán vznikl, ale ne na jeho slepých skvrnách“.

Jiné jsou sebekritiky a říkají to. „SELF-REVIEW: tuhle kritiku napsala tatáž session, která napsala I2780 až I2783,“ stojí v jedné z nich, a pokračuje: „kritika má větší váhu, když ji napíše jiná session nebo vlastník.“

To je poctivá mez té metody a záznamy ji uvádějí, místo aby ji skrývaly. Druhé čtení tímž druhem mysli zachytí, co první čtení přehlédlo, protože bylo zaměstnané souhlasem se sebou: chybějící soubor, gate se zadními vrátky, číslo, které žádný test nedokazuje. Nezachytí to, co tenhle druh mysli nevidí. Nic nebrání tomu, aby další kritiku napsala jiná session, jiný model nebo vlastník, a pole řekne, kdo to byl.

## Co kritika nedokáže

Gate kontroluje, že kritika existuje a že žádný blokující nález není otevřený. Nekontroluje, jestli byla kritika k něčemu. Recenzent, který nic nenajde, vytvoří záznam bez nálezů a gate práci pustí. Kritika se také neporovnává s pozdějšími verzemi plánu, jak zaznamenává zamítnutý nález výše, takže plán přepsaný po své kritice za to odmítnut není. To, co záznam zaručuje, je užší, a přesto to stojí za to. Každá námitka, kterou někdo vznesl, je v repozitáři vedle plánu, kterého se týká, i s odpovědí, a tu odpověď si lze přečíst a odmítnout.

Metodu popisuje [manuál plánování](@/docs/planning.md). Rozhodnutí jsou [ADR 0070](@/adrs/adr-0070.md) o intentech a [ADR 0073](@/adrs/adr-0073.md) o tom, čemu issue slouží. Samotné kritiky jsou obyčejné soubory YAML pod `.ai/repo/project/critiques/` v repozitáři.
