+++
title = "Mlčení není verdikt"
description = "V rozhraní, na které se člověk dívá, jsou kontrola, která se nikdy neozvala, a kontrola, která nahlásila zelenou, táž nepřítomnost. Jak je Majordomus drží od sebe: verdikt zařízený tak, aby dorazil, vlastní stav pro gate, o kterém žádný běh nic nenahlásil, důkaz jako zaznamenaný běh a střízlivý popis toho, co vynucuje gate a co drží jen review."
template = "page.html"
[extra]
lang = "cs"
translated_from = "silence-is-not-a-verdict/"
source_digest = "b3f60dc4668868dae002bdf6e8876feb8a0d5cd7bcb13a872c60e105aeff8592"
related = ["dokumentace-kterou-lze-odmitnout/", "vsechno-v-jednom-proudu/", "pochybnost-rozhodne-experiment/", "uspora-se-meri-netvrdi/"]
[extra.translations]
en = "silence-is-not-a-verdict/"
+++

Dne 10. září 2026 stály ve frontě čtyři validační běhy masteru, nejstarší z nich sedm hodin. Všechny linuxové joby v nich doběhly a v jednom běhu jich pět selhalo, jenže tři joby na runnerech macOS se nikdy nespustily a job s verdiktem, `ci`, potřebuje všechny tři. Nenahlásilo se tedy vůbec nic.

Trunk celou noc nesl chyby, které rozbíjely větve, a přitom každé rozhraní, na které se člověk dívá, ukazovalo, že je všechno v pořádku. Pravidlo [project.never-reported-is-not-green](@/rules/project-never-reported-is-not-green-1.md) touto nocí začíná: nikoho neoklamalo zelené zaškrtnutí, protože žádné zelené zaškrtnutí nebylo, ale prázdné místo, kam patří verdikt, a to se čte úplně stejně. Jeho znění je krátké: kontrola, která se neozvala, je neznámá a nikdy není úspěšná, a změna se merguje proti naměřené množině selhání trunku, ne proti mlčení.

## Stejný tvar v jiném převleku

Pravidlo zaznamenává tentýž tvar ještě dvakrát, a to hned následující den. Publikace, která uspěla, nahlásila `cancelled`, protože vlastní čekání jobu na síť pro doručování obsahu (CDN) vypršelo dřív, než síť změnu dohnala, a web už o minutu později servíroval nový commit. Správnou odpovědí bylo zeptat se webu, co servíruje, ne znovu spouštět nasazení, které už proběhlo. A každý pull request otevřený toho dne ukazoval selhávající kontroly, všechny do jedné selhání samotného trunku, takže worker, který by porovnával s nulou, by nezmergoval nic a trunk by nechal červený.

Levné čtení, tedy barva, slovo stavu nebo počet selhání, není to čtení, které rozhoduje; rozhoduje, co běh skutečně řekl a o kterém stromu. Změna se proto posuzuje proti vlastní množině selhání trunku a laťkou je stejná nebo menší množina, protože striktní nula by při červeném trunku zmrazila každou opravu za všemi ostatními opravami. Ohraničené čekání, které vypršelo, je měření, které nedoběhlo, ne měření, které dopadlo negativně.

## Verdikt se zařídí tak, aby dorazil

Gate, jehož runner je tak vzácný, že jeho odpověď nikdy nepřijde, je podle pravidla dokumentace, ne vynucení. [ADR 0037](@/adrs/adr-0037.md) (přijaté) změřilo, proč by čekání nepomohlo: master v integrační den dostával push zhruba každých sedm minut, sada testů na macOS běžela asi 106 minut a za celý ten den nedoběhl ani jeden z těch gates. Gate označený `on-demand: true` teď běží jen v plánu, který si o něj řekne, třeba v tom nočním, takže ho běžný push přeskočí, místo aby ho zařadil do fronty, a `ci` může nahlásit výsledek. Noční běh, který zčervená a nikdo ho nepřečte, je podle varování záznamu tatáž chyba, jen o jeden takt pomalejší.

## Dotázáno, bez odpovědi

Uvnitř tasku se z principu stává stav, který drží stroj, a to pod blokujícím pravidlem `majordomus.completion-gates`. `majordomus check` a `majordomus finish` posuzují každý gate, který vybere sada změn tasku, a `queued` znamená, že plán gate vybírá a žádný běh o něm nikdy nic nenahlásil: ne `unknown`, protože otázka položena byla a odpověď nepřišla, a ne `pass`, protože právě tuhle chybu má to slovo znemožnit. Gate, který selhal, zastaral nebo je blokovaný, odmítne výsledek `completed`, a stejně tak verdikt, který vůbec nešel přečíst, protože `completed` je tvrzení, že verdikt je známý.

Gate ve stavu `queued` se hlásí jménem, nikdy se nepřijme jako úspěch a nikdy se neodmítne. `check` vypíše povinné gates, které se nikdy neozvaly, za nimi `never-reported is not green` a id pravidla, a nález spustitelného programu dodá, že task dokončený přes ně je dokončený bez ověření. Nic neodmítá proto, že repozitář, kde je zaznamenávání nové, by jinak odmítl každé dokončení (finish).

## Důkaz je zaznamenaný běh

Matice tvrzení měla tutéž vadu, a to přímo před veřejností. Před [ADR 0041](@/adrs/adr-0041.md) (navržené) se u testu tvrzení kontrolovalo jen to, že jeho cesta vede k existujícímu souboru, takže veřejný web zobrazoval záruku proto, že soubor existoval. Rozhodnutí dělá z důkazu tvrzení zaznamenané provedení, které jmenuje commit, proti němuž běželo, a příkaz, který ho spustí znovu. Záznam počítal s tím, že v den, kdy přistane, bude každé tvrzení `not_run`, a nazval to zlepšením: předchozí stav byl neznámý, zobrazený jako zaručený.

Výsledek `not_run`, který by šlo zaznamenat, záměrně neexistuje a slabší úspěšný stav, `inputs_unchanged`, je nepřítomnost známého zneplatnění, ne důkaz na HEAD. [`docs/EVIDENCE.md`](@/docs/evidence.md) říká, že rozhraní, které ho vykreslí stejným zaškrtnutím jako `proven`, tu vadu zavedlo znovu, a gate důkazů, `evidence-check`, je naplánovaný při každém pushi s `always: true`, což dokument nazývá druhou polovinou pravidla.

Když [ADR 0080](@/adrs/adr-0080.md) (navržené) měřilo master na `8d37d011ef`, ledger obsahoval šest lokálních provedení a žádné tvrzení nebylo `proven`, a gate přesto svítil zeleně, protože jeho ráčna (ratchet) tehdy počítala `stale` jako podporu. Ta zelená byla podle záznamu poctivá k tomu, co měřila, a říkala mnohem méně, než za co ji čtenář má. Rozhodnutí zní, že běh zaznamenává a nikdy necommituje: záznam přinese samostatný pull request, jehož diff tvoří jen ledger, pod stejnými povinnými kontrolami jako kterýkoli jiný. Ten pull request zatím zapojený není, takže řádky validačního běhu dál o žádném verdiktu nerozhodují a chybějící report se jmenuje, nikdy se nepočítá jako úspěch.

## Přeskočený gate dluží důkaz

[ADR 0186](@/adrs/adr-0186.md) (navržené) slouží záměru (intent), aby povinný verdikt pull requestu dorazil do pěti minut, nikdy ale tím, že by se oslabil gate. Každý gate deklaruje svou dráhu (lane) a povinný verdikt potřebuje, aby byl každý gate dráhy zpětné vazby zelený a každý další vybraný gate buď proběhl, nebo byl znovu použit jako `inputs_unchanged`: zaznamenaný úspěšný výsledek, jehož vstupy, toolchain i význam jsou bajtově shodné. Dráha důkazu, tedy celá sada testů, úplný průchod prohlížečem, macOS, pokrytí a instalátor, je povinná na kombinovaném topu před přistáním a u vydání (release) a neznámý dopad vybírá všechno. Červená v ní blokuje další přistání a záznam vztahuje pravidlo na důkaz za každý přeskočený gate. Vlastník ho nepřijal.

## Co drží gate a co drží review

Samo pravidlo je aktivní a poradní a podle vlastních slov o něm rozhoduje review a záznam, ne gate, protože žádná kontrola nepozná, že člověk uvěřil mlčení. Nejbližší mechanickou oporou je mu to, že plánovač odmítne model, jehož verdiktu nelze dosáhnout, a že gate publikovaného webu, `pages-live`, se ptá webu, ne workflow; case plánovače, jeho jediné ověření, drží, že každý deklarovaný gate je dosažitelný v nějakém plánu. Všechno ostatní, včetně porovnání s množinou selhání trunku, drží review, a report, který tvrdí, že něco prochází, když žádný běh nedoběhl, je podle slov pravidla chybou správnosti, ne otázkou tónu.

[ADR 0039](@/adrs/adr-0039.md) (navržené) obrací tentýž argument na pravidla samotná: doktrína živosti přistála jako čtyři poradní pravidla, protože pravidlo, jehož třída tvrdí vynucení, které neexistuje, je přesně ten tvar, který tohle pravidlo odmítá ve směru CI. Dvě z nich se 30. září stala blokujícími, jakmile je dokázaly rozhodnout skeny, a jedno z nich, `project.every-wait-is-bounded`, na tomhle pravidle závisí a přenáší ho do živého čekání, kde se mlčení zkoumá a nikdy se nepočítá jako postup. Milník zapsaný 9. října 2026 zjistil, že úplný plán CI spouštěný podle rozvrhu, jediný, který vybírá každý gate, neměl v posledních deseti bězích zelený verdikt, zatímco push běhy na masteru zůstávaly zelené, nic nevyvolalo poplach a pravidlo bylo jen věcí review.

ADR 0037 je přijaté, zatímco ADR 0039, ADR 0041, ADR 0080 a ADR 0186, která pravidlo citují, jsou pořád navržená. Jak proud signálů pojmenuje zdroj, který nedokázal přečíst, popisuje [Všechno v jednom proudu](@/vsechno-v-jednom-proudu.md), jak repozitář odmítá vlastní dokumentaci, vysvětluje [Dokumentace, kterou lze odmítnout](@/dokumentace-kterou-lze-odmitnout.md), jak se neshoda rozhoduje experimentem místo počítání, ukazuje [Pochybnost rozhodne experiment](@/pochybnost-rozhodne-experiment.md) a model důkazů popisuje [`docs/EVIDENCE.md`](@/docs/evidence.md).
