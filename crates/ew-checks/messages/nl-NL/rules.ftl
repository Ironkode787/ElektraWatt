# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: GPL-3.0-or-later

# The rule messages in nl-NL (doc 15 §3.8; WP-S1.16 D6). A rule or attribute missing here falls back to en-GB; until
# 1.0 the message test lists the missing ones as warnings. Shipped first for the rules that report on Golden A
# (doc 20 §18.1); the other texts follow (the de-scope order of WP-S1.16 §13.1).

rule-ew-core-001 =
    .title = Manifest beschadigd
    .hint = Sla het project op om een nieuw manifest te schrijven, of neem de versie van een van beide kanten uit git.

rule-ew-core-002 =
    .title = Gemengde generatie
    .hint = Zet de bestanden van het project terug uit één opslag (git, de synchronisatieclient of een back-up), of speel een onverwacht beëindigde sessie opnieuw af met ew recover.

rule-ew-core-003 =
    .title = Vermeld bestand ontbreekt
    .hint = Zet het bestand terug uit git, de synchronisatieclient of een back-up, of laat een cloudmap het downloaden afmaken en open het project opnieuw.

rule-ew-core-004 =
    .title = Verweesd bestand
    .hint = Vergelijk het bestand met het project, voeg wat u nodig hebt samen als externe wijziging en verwijder daarna het bestand.

rule-ew-core-005 =
    .title = JSON-syntaxisfout
    .hint = Herstel het project met ew fmt --repair, of corrigeer de regel in een teksteditor.

rule-ew-core-006 =
    .title = Git-conflictmarkeringen
    .hint = Los het conflict op in git, of handel de conflictrecords af met ew merge resolve.

rule-ew-core-007 =
    .title = Schemaschending
    .hint = Corrigeer de regel in een teksteditor volgens de referentie van het formaat, of zet haar terug uit git.

rule-ew-core-008 = { $form ->
        [duplicate_number] { WORD($module) } heeft een tweede kanaal met nummer { $number }; de module houdt één kanaal per nummer, dus dit kanaal wordt nergens afgedrukt
        [channel] kanaal { WORD($channel) } hoort bij { WORD($module) }, dat geen PLC-module is; verwijder het kanaal of maak van { WORD($module) } weer een module
       *[form] folio { $page }: { $names ->
            [both] het kader voor paginatype { $type } noemt formulierkopie { $type_copy }, en het standaardkader noemt formulierkopie { $default_copy }
            [type] het kader voor paginatype { $type } noemt formulierkopie { $type_copy }
           *[default] het standaardkader noemt formulierkopie { $default_copy }
        }, die het project niet bevat; de pagina neemt het volgende kader
    }
    .title = Verwijzing naar een ontbrekend record
    .hint = Herstel het record waarnaar verwezen wordt, of verwijder de verwijzing met de snelle oplossing.

rule-ew-core-009 =
    .title = Dubbele record-ID
    .hint = Verwijder de extra regel in een teksteditor, of geef het record een nieuwe ID als u beide wilt houden.

rule-ew-core-010 =
    .title = Record in het verkeerde bestand
    .hint = Sla het project op: de opslag zet het record in zijn bestand.

rule-ew-core-011 = { $form ->
        [file] { $file ->
            [page] het bestand van folio { $name }
            [layout] het layoutbestand van { $name }
            [a-layout] een layoutbestand
            [a-page] een paginabestand
            [import] een bestand van een import
           *[path] { $name }
        }: het samenvoegen kon { $side ->
            [ours] onze versie
            [theirs] hun versie
           *[other] één kant
        } niet lezen; de regels staan in een conflictrecord
       *[record] { $target ->
            [project] het project
            [device] apparaat { WORD($tag) }
            [page] folio { $page }
            [a-page] een pagina
            [function] een functie van apparaat { WORD($tag) }
            [a-function] een functie
            [connection-on] verbinding van { WORD($from) } naar { WORD($to) } op folio { $page }
            [connection] verbinding van { WORD($from) } naar { WORD($to) }
            [a-connection] een verbinding
           *[other] een record van soort { $kind }
        }: { $side ->
            [ours-deleted] onze versie heeft het verwijderd en hun versie wijzigde { $members }
            [theirs-deleted] hun versie heeft het verwijderd en onze versie wijzigde { $members }
           *[differ] onze en hun versie verschillen in { $members }
        }
    }
    .title = Conflictrecords aanwezig
    .hint = Kies voor elk conflict onze of hun versie met ew merge resolve, of verwijder het record dat conflicteert.

rule-ew-core-012 =
    .title = Structurele invariant geschonden
    .hint = Laat de volgende wijziging van het record de invariant herstellen, of zet het bestand terug uit git.

rule-ew-core-014 =
    .title = Bibliotheekkopie gewijzigd
    .hint = Neem de kopie opnieuw over uit de bibliotheek, of behoud de wijziging bewust.

rule-ew-core-015 =
    .title = Middel ontbreekt
    .hint = Importeer het middel opnieuw of zet het bestand terug uit git, en verwijder het beschadigde bestand.

rule-ew-core-016 =
    .title = Gegevens uit een nieuwer formaat
    .hint = Open het project in de nieuwere ElektraWatt die het schreef, of werk ElektraWatt bij.

rule-ew-core-017 =
    .title = Ongeldige bekende variant
    .hint = Voer de waarde opnieuw in bij Eigenschappen, dat de ruwe waarde toont.

rule-ew-core-018 =
    .title = Onbekend afleidingsprofiel
    .hint = Open het project in een build die het profiel meelevert.

rule-ew-core-019 =
    .title = Leeslimiet overschreden
    .hint = Splits of herstel het bestand buiten ElektraWatt, en controleer waar het project vandaan komt.

rule-ew-core-021 = folio { $page } ligt buiten het foliobereik { $from }–{ $to } van sectie { $section }; hernummer de pagina of verplaats haar naar de sectie waarvan het bereik { $number } bevat
    .title = Folio buiten zijn sectie
    .hint = Hernummer de pagina binnen het foliobereik van haar sectie, of verplaats haar naar de sectie die het bereik bezit.

rule-ew-core-022 = folio { $page } wordt door { $count } pagina's afgedrukt ({ WORD($kinds) }); geef elke pagina een eigen folio
    .title = Dubbel folio
    .hint = Geef een van de pagina's een ander folio; twee pagina's delen nooit een folio.

rule-ew-core-023 = folio { $page } overschrijft de projectomschrijving in zijn titelblok met "{ $text }"
    .title = Overschrijving in het titelblok
    .hint = Verwijder de overschrijving om de gekoppelde projectwaarde weer af te drukken, of behoud haar als de pagina een eigen tekst nodig heeft.

rule-ew-core-025 = de inhoudsopgave kwam niet tot een vast aantal pagina's; ze gebruikt { $pages } pagina's en haar laatste pagina vermeldt alle overige regels
    .title = Inhoudsopgave stabiliseert niet
    .hint = Verkort de gegenereerde inhoudsopgave of haar paginaregels, zodat haar aantal pagina's vast komt te liggen.

rule-ew-core-026 = { $family ->
        [terminal_diagram] klemmenstrook { WORD($device) } staat in geen klemmenschema
        [cable_overview_diagrams] kabel { WORD($device) } staat in geen kabeloverzicht en kabelschema's
       *[plc_overview] PLC-station { WORD($device) } staat in geen PLC-overzicht
    }: geen rapportblok van die soort dekt het; voeg er een toe of neem het op
    .title = Rapport dekt niet alle records
    .hint = Voeg een rapportblok van die soort toe, of sluit de records bewust uit van het rapport.

rule-ew-core-031 = ader { $core } heeft de kleur { $colour }: { $codes } is geen IEC 60757-code en geen bedrijfscode van het kleurprofiel
    .title = Ongeldige aderkleur
    .hint = Kies een kleurcode uit de norm voor aderkleuren, bijvoorbeeld BN in plaats van BR.

rule-ew-core-032 = ader { $core } { $form ->
        [foreign] is geen ader van het kabelartikel { $part }
       *[differs] wijkt af van ader { $def } van het kabelartikel { $part }: { $colour ->
            [yes] { $sep_colour }kleur { $colour_core } ≠ { WORD($colour_part) }
           *[no] {""}
        }{ $csa ->
            [yes] { $sep_csa }doorsnede { DECIMAL($csa_core) } ≠ { DECIMAL(WORD($csa_part)) }
           *[no] {""}
        }{ $kind ->
            [yes] { $sep_kind }soort { $kind_core ->
                [shield] afscherming
               *[core] ader
            } ≠ { $kind_part ->
                [shield] afscherming
               *[core] ader
            }
           *[no] {""}
        }{ $pair ->
            [yes] { $sep_pair }paar { $pair_core } ≠ { WORD($pair_part) }
           *[no] {""}
        }
    }
    .title = Ader wijkt af van het artikel
    .hint = Gebruik de ader zoals het kabelartikel haar definieert, of kies het artikel dat deze ader heeft.

rule-ew-core-033 = { WORD($cable) } heeft geen gerouteerde, handmatige of artikellengte
    .title = Kabel zonder lengte
    .hint = Voer een handmatige lengte in, routeer de kabel in de layout of wijs een artikel met een vaste lengte toe.

rule-ew-core-034 = de externe verbinding { $connection } heeft geen kabel: { $count ->
        [one] draad { $wires } is
       *[many] draden { $wires } zijn
    } geen kabelader; voeg de kabel toe en wijs de aders toe
    .title = Externe verbinding zonder kabel
    .hint = Voeg de kabel toe die de locatie verlaat en wijs de aders toe aan de draden.

rule-ew-core-036 = ader { $core } is aan { $count } draden toegewezen ({ WORD($wires) }); een ader draagt één draad
    .title = Ader dubbel toegewezen
    .hint = Wijs de ader aan één verbinding toe, of gebruik een andere ader van de kabel.

rule-ew-core-037 = afscherming { $core } is aan geen van beide einden aangesloten; sluit haar aan één einde of aan beide aan
    .title = Afscherming niet aangesloten
    .hint = Sluit de afscherming aan op een aardingspunt, of markeer haar als bewust niet aangesloten.

rule-ew-core-041 = { WORD($device) } heeft geen artikel; wijs er een toe of markeer het als geleverd door derden
    .title = Apparaat zonder artikel
    .hint = Wijs een artikel uit de bibliotheek toe of markeer het apparaat als geleverd door derden.

rule-ew-core-042 = artikel { $part } van { WORD($device) } heeft geen { $missing ->
        [description] omschrijving
        [manufacturer] fabrikant
       *[both] omschrijving en geen fabrikant
    }
    .title = Onvolledige artikelgegevens
    .hint = Vul de gegevens van het artikel aan in de bibliotheek en werk de ingebedde kopie bij.

rule-ew-core-043 = tijdelijk artikel "{ $part }" (artikelnummer TBD) wordt gebruikt door { WORD($users) }; kies het artikel vóór de vrijgave
    .title = Tijdelijk artikel
    .hint = Vervang vóór de vrijgave elk tijdelijk artikel door het echte artikel.

rule-ew-core-044 = { $form ->
        [bus] { $end ->
            [connector] een connector
            [device] een apparaat:{ $port }
            [ip] einde { $port }
            [segment] een segmenteinde
           *[named] { $port }
        }: het stationsadres is TBD; vul het in vóór de vrijgave
        [start] { WORD($module) }: het startadres van de { $which ->
            [input] ingangen
           *[output] uitgangen
        } is TBD; vul het in vóór de vrijgave
       *[channel] { WORD($module) }: het adres van kanaal { $number } is TBD; vul het in vóór de vrijgave
    }
    .title = Open TBD-waarde
    .hint = Vul vóór de vrijgave de waarde in die nog als TBD gemarkeerd is.

rule-ew-core-045 = { WORD($device) } gebruikt { $need_no } maakcontacten, { $need_nc } verbreekcontacten en { $need_co } wisselcontacten; zijn artikelen hebben er { $have_no }, { $have_nc } en { $have_co }
    .title = Meer hulpcontacten dan het artikel heeft
    .hint = Verwijder contacten uit de tekening, of kies een artikel met genoeg hoofd- en hulpcontacten.

rule-ew-core-047 = { WORD($device) } heeft { $part ->
        [yes] een artikel maar geen representatie op een pagina; de stuklijst telt het nog mee
       *[no] geen representatie op een pagina
    }
    .title = Apparaat niet in het schema
    .hint = Plaats een weergave van het apparaat in het schema, of verwijder het apparaat.

rule-ew-core-048 = { WORD($device) } wordt { $form ->
        [covered-twice] gedekt door { $count } bestelcodes van { WORD($assembly) }; zijn artikelen zouden { $count } keer worden afgedrukt
       *[uncovered] door geen enkele bestelcode van { WORD($assembly) } gedekt; de stuklijst van de samenstelling vermeldt zijn eigen artikelen naast de geconfigureerde producten
    }
    .title = Bestelcodes dekken de samenstelling niet
    .hint = Neem de ontbrekende leden op in een bestelcode van de samenstelling, of verwijder ze uit de samenstelling.

rule-ew-core-051 = onderbrekingspunt { $name } { $form ->
        [no-cp] heeft geen aansluitpunt; het vormt met niets een paar
       *[unpaired] heeft geen tegenhanger; zijn draad eindigt open
    }
    .title = Onderbrekingspunt zonder tegenhanger
    .hint = Plaats het bijbehorende onderbrekingspunt met dezelfde naam, of verwijder dit punt.

rule-ew-core-052 = onderbrekingspunt { $name }: { $form ->
        [flagged] { $count } punten zijn als sterbron gemarkeerd; laat de markering op één ervan staan
       *[sources] { $count } bronpunten en geen sterbron; maak ze op één na tot doel, of markeer er één als sterbron
    }
    .title = Dubbelzinnig onderbrekingspunt
    .hint = Houd één bron per naam: maak de andere bronnen tot doel, of markeer precies één punt als sterbron.

rule-ew-core-053 = { $item }: { $form ->
        [outside] buiten het kader (het anker van het doel heeft geen cel)
       *[empty] lege cel (cel { $cell } bevat geen grafiek van het doel; zijn ankerbeleid plaatst het anker daarbuiten)
    }
    .title = Ongeldige cel voor de kruisverwijzing
    .hint = Herstel het standaardbeleid voor ankers, of verplaats het anker naar een cel die de grafiek bevat.

rule-ew-core-061 = tekststijl { $style } drukt af met een hoofdletterhoogte van { DECIMAL($height) } mm, onder het minimum van { DECIMAL($minimum) } mm
    .title = Tekst onder de minimale hoogte
    .hint = Vergroot de tekststijl of geef de teksten een stijl die minstens op de minimale hoogte afdrukt.

rule-ew-core-063 = een tekst op folio { $page } heeft de grootte { $style }, die geen tekststijl vastlegt; geef de tekst een tekststijl
    .title = Tekstgrootte zonder tekststijl
    .hint = Geef de tekst een van de tekststijlen van het project in plaats van een vrije grootte.

rule-ew-core-065 = omschrijving "{ $text }" van artikel { $part }{ $form ->
        [rating] {" "}bevat de nominale waarde { $matches }: verplaats die naar de technische gegevens
       *[unit-format] : eenheidsnotatie { $matches } (een spatie tussen getal en eenheid, eenheden geschreven zoals in SI)
    }
    .title = Technische gegevens in de omschrijving
    .hint = Verplaats nominale waarden en technische gegevens uit de omschrijving naar hun eigen velden.

rule-ew-core-067 = Draad { WORD($wire) } loopt door { $point ->
        [junction] de aftakking in { $pin }
        [junction-off-grid] een aftakking
       *[cp] { $pin }
    }, waarmee hij niet verbonden is.
    .title = Draad door een aansluitpunt
    .hint = Routeer de draad opnieuw met Selectie routeren, of wijzig zijn route met Route bewerken zodat hij vrij blijft van het punt.

rule-ew-core-068 = Draad { WORD($wire) } loopt over { WORD($other) } op folio { $page }.
    .title = Draad over een andere draad
    .hint = Routeer een van de draden opnieuw met Selectie routeren, zodat elke draad een eigen lijn heeft.

rule-ew-core-069 = Draad { WORD($wire) } loopt door { $symbol ->
        [ip] onderbrekingspunt { WORD($device) }
       *[device] { WORD($device) }
    }.
    .title = Draad door een symbool
    .hint = Routeer de draad opnieuw met Selectie routeren, of verplaats het symbool uit de weg.

rule-ew-core-070 = Draad { WORD($wire) } loopt door { $owner ->
        [free-text] de vrije tekst "{ $text }"
        [structure-box] het bijschrift "{ $text }" van een structuurkader
        [ip] de tekst "{ $text }" van onderbrekingspunt { WORD($device) }
       *[symbol] de tekst "{ $text }" van { WORD($device) }
    }.
    .title = Draad door een tekst
    .hint = Routeer de draad opnieuw met Selectie routeren, of verplaats de tekst.

rule-ew-core-071 = een grafische lijn eindigt op { $cp }, wat elektrisch niets verbindt; teken in plaats daarvan een verbinding
    .title = Grafische lijn op een aansluitpunt
    .hint = Vervang de grafische lijn door een verbinding, zodat ze meetelt in de connectiviteit.

rule-ew-core-072 = { $place ->
        [off-grid] Een aftakking op folio { $page } verbindt vier draden: teken in plaats daarvan twee T-aftakkingen.
       *[cell] De aftakking in { $cell } verbindt vier draden: teken in plaats daarvan twee T-aftakkingen.
    }
    .title = Vierwegaftakking
    .hint = Verplaats één tak naar een eigen aftakking een rasterstap verder, zodat elke aftakking drie draden verbindt.

rule-ew-core-073 = Draad { WORD($wire) } heeft geen route: Pagina routeren tekent hem.
    .title = Draad zonder route
    .hint = Gebruik Pagina routeren of Selectie routeren, of teken de route van de draad met Route bewerken.

rule-ew-core-081 = vrijstelling { $waiver } voor { $rule } { $form ->
        [not-evaluated] is niet beoordeeld: de regel liep niet in schema { $scheme } ({ $mode ->
            [online] online
           *[release] vrijgave
        }modus)
       *[unmatched] past op geen enkele bevinding; verwijder haar of corrigeer haar doel
    }
    .title = Vrijstelling zonder bevinding
    .hint = Verwijder de vrijstelling, of corrigeer haar doel naar de bevinding waarvoor ze bedoeld was.

rule-ew-core-083 = regel { $rule } { $form ->
        [prevent] kon de bewerking niet toetsen: hij stopte met een interne fout, dus de bewerking is niet doorgevoerd; meld het defect
       *[scope] kon niet worden beoordeeld voor { WORD($scope) }: hij stopte met een interne fout, dus zijn bevindingen daar ontbreken; meld het defect
    }
    .title = Regel niet geëvalueerd
    .hint = Meld het defect; de bevindingen van de regel in dat bereik ontbreken tot het is verholpen.

rule-ew-core-084 = pakket { $pack } is niet volledig beoordeeld: zijn gegevenstabel { $file } laadt niet ({ $error }); { $rules } liepen niet
    .title = Regelpakket niet geëvalueerd
    .hint = Herstel het gegevensbestand van het pakket of vertrouw het project, zodat het pakket kan draaien.

rule-ew-fl-001 = { $form ->
        [many] { WORD($valve) }: werkpoorten { $ports_nl } zijn open; sluit ze aan, zet er een blindstop op of blind ze af
       *[one] { WORD($valve) }: werkpoort { $port } is open; sluit hem aan, zet er een blindstop op of blind hem af
    }
    .title = Werkpoort open
    .hint = Teken de werkleiding, of sluit de poort af: met een blindstop, met een geluiddemper of afgeblind.

rule-ew-fl-002 = { $form ->
        [excess] { WORD($valve) }: magneetspoel { WORD($solenoid) } is aan { $count } kanalen gebonden: { $channels_nl }
        [undeclared] { WORD($valve) }: { $channel_nl } stuurt een spoel aan die het artikel niet declareert
       *[unbound] { WORD($valve) }: magneetspoel { WORD($solenoid) } heeft geen kanaal
    }
    .title = Kanalen en magneetspoelen komen niet overeen
    .hint = Bind elke magneetspoel die het ventielartikel declareert aan één kanaal, en geen kanaal aan een andere spoel.

rule-ew-fl-004 = { $form ->
        [blank] { WORD($module) }: ventielplaats { $position } bevat een blindplaat, { WORD($device) }
       *[valve] { WORD($module) }: ventielplaats { $position } bevat een reserveventiel, { WORD($device) }
    }
    .title = Ventielplaatsen in reserve
    .hint = Houd het reserveventiel of de blindplaat bewust, of plaats een gebruikt ventiel op de ventielplaats.

rule-ew-fl-011 = { WORD($valve) } stuurt { $target } aan, maar de leidingen van zijn werkpoorten bereiken { WORD($device) }
    .title = Aangestuurd apparaat wijkt af
    .hint = Koppel het ventiel aan het apparaat dat zijn leidingen bereiken, of teken de leidingen naar het apparaat dat het aanstuurt.

rule-ew-fl-013 = { $form ->
        [both] { WORD($parent) }: ventielplaats { $position } bevat zowel { $a } als { $b }
        [modules] { WORD($parent) }: positie { $position } bevat zowel { $a } als { $b }
        [outside] { WORD($module) }: ventielplaats { $position } ligt buiten de { $count } ventielplaatsen
        [outside_free] { WORD($module) }: ventielplaats { $position } ligt buiten de { $count } ventielplaatsen; vrije ventielplaatsen: { $free }
        [outside_full] { WORD($module) }: ventielplaats { $position } ligt buiten de { $count } ventielplaatsen; elke ventielplaats is bezet
       *[empty] { WORD($module) }: ventielplaats { $position } is leeg; plaats een ventiel of een blindplaat
    }
    .title = Bezetting
    .hint = Geef elke ventielplaats één ventiel of blindplaat, en elke module een eigen positie.

rule-ew-lay-001 = { WORD($device) } is niet geplaatst in de layout van { WORD($location) }; plaats het op een rail of op de montageplaat
    .title = Niet geplaatst apparaat
    .hint = Plaats het apparaat op een rail of op de montageplaat van de layout van zijn locatie.

rule-ew-lay-003 = de plaatsing van { WORD($device) } gebruikt artikel { WORD($part) }, dat het apparaat niet meer heeft; plaats het apparaat met een van zijn artikelen, of geef het het artikel terug
    .title = Artikel niet op het apparaat
    .hint = Plaats het apparaat opnieuw met een van zijn artikelen, of geef het het artikel van zijn plaatsing terug.

rule-ew-lay-004 = { WORD($device) } { $location ->
        [none] heeft geen locatie
       *[other] staat in { WORD($location) }
    } maar is geplaatst in de layout van { WORD($space) }; verplaats het naar de layout van zijn locatie, of corrigeer zijn locatie
    .title = Locatie wijkt af
    .hint = Verplaats het apparaat naar de layout van zijn eigen locatie, of corrigeer zijn locatie.

rule-ew-lay-006 = { $form ->
        [placements] { WORD($device) } overlapt { WORD($other) }; verplaats een van beide
        [same] twee plaatsingen van { WORD($device) } overlappen; verplaats een van beide
        [limit] meer voetafdrukken overlappen op de montageplaat van { WORD($enclosure) }; de eerste { $listed } overlappingen staan vermeld
        [duct] { WORD($device) } overlapt de kabelgoot bij { $duct }; verplaats het apparaat of de kabelgoot
       *[rail] de rail bij { $rail } overlapt de kabelgoot bij { $duct }; verplaats de rail of de kabelgoot
    }
    .title = Botsing
    .hint = Verplaats een van de overlappende objecten tot hun voetafdrukken niet meer overlappen.

rule-ew-lay-010 = { $form ->
        [on_duct] { WORD($device) } is geplaatst op de kabelgoot bij { $duct }, die geen apparaten draagt; plaats het op een rail
        [off_rail] { WORD($device) } is geplaatst op { DECIMAL($s) } mm op de rail bij { $rail }, buiten 0,0 tot { DECIMAL($length) } mm; verplaats het op de rail
        [rail_turn] { WORD($device) } is op de rail bij { $rail } gedraaid over een andere hoek dan 0° of 180°; draai het terug
        [off_axis] { WORD($device) } is naast de as van de rail bij { $rail } geplaatst; verplaats het op de rail
        [profile] { WORD($device) } (artikel { WORD($part) }) past niet op de rail bij { $rail } ({ WORD($profile) }); plaats het op een rail waar het op past
        [surface_turn] { WORD($device) } is op de montageplaat gedraaid over een andere hoek dan een veelvoud van 90°; draai het terug
        [off_surface] { WORD($device) } ligt niet in het vlak van de montageplaat; plaats het op de plaat
        [surface] { WORD($device) } (artikel { WORD($part) }) wordt niet op de plaat zelf gemonteerd; plaats het op een rail
        [overflow] { WORD($device) } beslaat { DECIMAL($from) } tot { DECIMAL($to) } mm op de rail bij { $rail }, voorbij 0,0 tot { DECIMAL($length) } mm; maak het korter of verplaats het
        [outside] { WORD($device) } ligt buiten het bruikbare vlak van de montageplaat; verplaats het naar binnen
        [bracket_overlap] De eindsteun { $side ->
            [before] vóór
           *[after] na
        } { WORD($device) } overlapt { $other_kind ->
            [bracket] de eindsteun { $other_side ->
                [before] vóór
               *[after] na
            } { WORD($other) }
           *[placement] { WORD($other) }
        } op { $rail }.
        [bracket_outside] De eindsteun { $side ->
            [before] vóór
           *[after] na
        } { WORD($device) } steekt { DECIMAL($by) } mm voorbij { $edge ->
            [start] het begin
           *[end] het einde
        } van { $rail }.
        [path] het pad van de { $carrier ->
            [rail] rail bij { $at } is geen twee punten op één horizontale of verticale lijn
            [duct] kabelgoot bij { $at } is geen lijn van horizontale en verticale stukken
           *[other] drager bij { $at } is geen lijn van horizontale en verticale stukken
        }; pas het pad aan
        [carrier_outside] de { $carrier ->
            [rail] rail
            [duct] kabelgoot
           *[other] drager
        } bij { $at } ligt buiten het bruikbare vlak van de montageplaat; verplaats die naar binnen
        [spaces] { WORD($location) } heeft een tweede layoutruimte; een locatie heeft één layoutruimte; verwijder deze
        [enclosures] de layout van { WORD($location) } heeft een tweede kast, { WORD($enclosure) }; een layoutruimte heeft één kast; verwijder deze
       *[plates] de kast { WORD($enclosure) } heeft een tweede montageplaat; een kast heeft één montageplaat; verwijder deze
    }
    .title = Drager past niet
    .hint = Plaats het apparaat op een drager waar het op past, binnen de drager en het bruikbare vlak van de plaat.

rule-ew-lay-016 = { $form ->
        [order] de leden van { WORD($assembly) } op de rail bij { $rail } staan niet op volgorde: { WORD($early) } hoort vóór { WORD($late) }; plaats eerst de ouder, dan de leden op positie
       *[between] { WORD($other) } staat tussen leden van { WORD($assembly) } op de rail bij { $rail }; houd de leden bij elkaar
    }
    .title = Montagevolgorde
    .hint = Plaats eerst de ouder van de samenstelling en de leden ernaast, in de volgorde van hun posities.

rule-ew-num-001 = { $frozen }: vastgezet, wijkt af van het nummeringsschema (afgeleide waarde { $derived })
    .title = Kenmerk wijkt af van het nummeringsschema
    .hint = Maak het kenmerk los om de afgeleide waarde over te nemen, of laat het vastgezet als de afwijking bedoeld is.

rule-ew-num-002 = draadnummer { $number }: vastgezet, wijkt af van de regel ({ $derived ->
        [none] de regel leidt voor de draad geen nummer af
       *[value] afgeleide waarde { $rule }
    })
    .title = Vastgezette waarde wijkt af
    .hint = Losmaken geeft de afgeleide waarde; houd de waarde vastgezet als de afwijking bedoeld is.

rule-ew-plc-001 = { $form ->
        [alignment] { $channel }: het adres { $address } begint niet op een veelvoud van { $align } bytes, de woorduitlijning van het profiel (een niet-geverifieerde conventie)
       *[overlap] adressen overlappen in configuratieproject { WORD($cfg) }: { $channels }{ $more ->
            [none] {""}
           *[other] ; en nog { $more } kanalen
        }; wijzig adressen tot er geen meer overlappen
    }
    .title = Adresoverlap
    .hint = Wijzig de adressen van de kanalen tot er geen meer overlappen, of laat ze een automatisch adres nemen.

rule-ew-plc-002 = { $channel }: het adres { $address } ligt buiten het I/O-bereik van { WORD($module) } ({ $range ->
        [none] geen byte van gebied { $area }
       *[bytes] bytes { $first } tot en met { $last } van gebied { $area }
    })
    .title = Adres buiten de module
    .hint = Geef het kanaal een adres binnen het bereik van zijn module, of corrigeer de start van de module.

rule-ew-plc-003 = { $subject }: { $what ->
        [input] de start van de ingangen
        [output] de start van de uitgangen
       *[address] het handmatige adres
    } noemt bit { $bit }, buiten de { $width } bits van een eenheid
    .title = Bitindex buiten bereik
    .hint = Kies een bit binnen de breedte van een eenheid.

rule-ew-plc-007 = het symbool { $symbol } wordt gebruikt door { $count } kanalen van configuratieproject { WORD($cfg) }: { $channels }; geef elk kanaal een eigen symbool
    .title = Dubbel symbool
    .hint = Geef elk kanaal een eigen symbool.

rule-ew-plc-008 = { $channel } is in gebruik maar heeft geen adres: { $reason ->
        [station] de module hoort bij geen PLC-station; plaats eerst een CPU of koppel de module aan een station
        [excluded] het is uitgesloten van adressering; geef het een adres
       *[other] er is geen adres voor af te leiden; controleer de start van de module en de PLC-sectie van het artikel
    }
    .title = Gebruikt kanaal zonder adres
    .hint = Geef het kanaal een adres, of markeer het als ongebruikt.

rule-ew-plc-012 = { $form ->
        [manual] { $channel }: het handmatige adres { $address } wijkt af van het afgeleide adres { $derived }; zet het adres op automatisch, of houd de afwijking bewust aan
       *[binding] { $channel } ({ $address }) stuurt spoel { WORD($solenoid) } van { WORD($valve) } aan, die de positiekaart van artikel { $part } toewijst aan { $expected } ({ $expected_address }); koppel het kanaal van de kaart, of corrigeer de PLC-sectie van het artikel
    }
    .title = Koppeling wijkt af van de artikelkaart
    .hint = Koppel het kanaal dat de kaart van het artikel toewijst, of corrigeer de PLC-sectie van het artikel.

rule-ew-plc-015 = { $subject }: { $what ->
        [input] de start van de ingangen
        [output] de start van de uitgangen
       *[address] het kanaaladres
    } is TBD; vul het in vóór de vrijgave
    .title = Tbd-adres
    .hint = Vul het adres in vóór de vrijgave.

rule-ew-plc-004 = { $form ->
        [input] { $channel }: het { $kind }-kanaal leest de spoel van { WORD($target) }; een ingang leest een sensor, controleer de bedrading of de kanaalsoort
       *[output] { $channel }: het { $kind }-kanaal stuurt het contact van { WORD($target) } aan; een uitgang stuurt een actuator aan, controleer de bedrading of de kanaalsoort
    }
    .title = Kanaalsoort past niet
    .hint = Bedraad het kanaal naar een functie van zijn soort, of wijzig de kanaalsoort.

rule-ew-plc-005 = { $channel }: het symbool { $symbol } is geen IEC 61131-3-identifier, wat configuratieproject { WORD($cfg) } vereist; gebruik letters, cijfers en enkele underscores
    .title = Ongeldig symbool
    .hint = Geef het kanaal een symbool van letters, cijfers en enkele underscores dat niet met een cijfer begint.

rule-ew-plc-009 = { $channel } heeft { $what ->
        [both] een handmatig adres en een symbool
        [address] een handmatig adres
       *[symbol] een symbool
    } maar geen verbinding; verbind het, of markeer het als reserve of gereserveerd
    .title = Kanaal zonder verbinding
    .hint = Verbind het kanaal, of markeer het als reserve of gereserveerd.

rule-ew-plc-010 = { $channel }: het gegevenstype { $data_type } ({ $bits } bit) past niet bij het { $kind }-kanaal van { $width } bit; geef het kanaal een gegevenstype van zijn breedte
    .title = Breedte van het gegevenstype past niet
    .hint = Geef het kanaal een gegevenstype dat even breed is als het kanaal.

rule-ew-plc-011 = { $form ->
        [slot] { WORD($module) } zit in slot { $slot } van { WORD($station) }, maar zijn artikel { $part } staat slots { $allowed } toe; verplaats het naar een toegestaan slot
        [max_modules] { WORD($station) } heeft { $have } modules, meer dan de { $limit } die zijn artikel toestaat; verwijder modules
        [input] { WORD($station) } heeft { $have } ingangsbytes, meer dan de { $limit } die zijn artikel toestaat; verwijder modules
       *[output] { WORD($station) } heeft { $have } uitgangsbytes, meer dan de { $limit } die zijn artikel toestaat; verwijder modules
    }
    .title = Slotregels
    .hint = Plaats elke module in een slot dat zijn artikel toestaat, en houd het station binnen zijn grenzen.

rule-ew-plc-013 = { $form ->
        [fe] { $cp_shared ->
            [yes] { $cp_tag } pen { $cp_pin } ({ $cp_nth }e)
           *[no] { $cp }
        } is de FE-pen van een PLC-kaart en wordt gevoed uit de PE-potentiaal { $potential }; sluit hem aan op functionele aarde, of declareer een FE–PE-verbinding
        [unconnected] { $cp }, de { $role }-pen van de { $group ->
            [electronics] elektronica-
            [load] belasting-
           *[other] {""}
        }voedingsgroep, is niet aangesloten; sluit de voeding van de kaart aan
       *[wrong] { $cp }, de { $role }-pen van de { $group ->
            [electronics] elektronica-
            [load] belasting-
           *[other] {""}
        }voedingsgroep, wordt gevoed uit een potentiaal van type { $type ->
            [signal] signaal
            [undefined] onbepaald
           *[other] { $type }
        }; voed hem uit { $role }
    }
    .title = Kaartvoeding onjuist
    .hint = Sluit elke voedingspen van de kaart aan op een potentiaal van zijn rol, en zijn FE-pen op functionele aarde.

rule-ew-plc-014 = { $subject }: { $form ->
        [channel] de uitgang
        [group] de belastingvoedingsgroep
       *[module] de module
    } voert { $load } mA, meer dan zijn waarde van { $limit } mA; verdeel de belastingen of gebruik een sterkere uitgang
    .title = Uitgangsbelasting boven de waarde
    .hint = Verdeel de belastingen over meer uitgangen, of gebruik een sterkere uitgang of module.

rule-ew-saf-001 = { $form ->
        [missing] { WORD($module) } is een F-module zonder F-doeladres; vul het in
        [range] { WORD($module) }: het F-doeladres { $address } ligt buiten { $min }..{ $max }, het bereik van zijn apparaatbeschrijving; vul een adres binnen dat bereik in
       *[duplicate] het F-doeladres { $address } wordt gebruikt door { $modules }; geef elke F-module een eigen adres
    }
    .title = F-adres
    .hint = Geef elke F-module een eigen F-doeladres binnen het bereik van zijn apparaatbeschrijving.

rule-ew-saf-002 = { $form ->
        [incomplete] { $channel } is een 1oo2-kanaal zonder partner die het terugnoemt; koppel het aan het andere kanaal van de sensor
       *[devices] het 1oo2-paar { $channel } en { $other } leest twee apparaten, { WORD($first) } en { WORD($second) }; bedraad beide kanalen naar dezelfde tweekanaalssensor
    }
    .title = 1oo2-paar
    .hint = Koppel elk 1oo2-kanaal aan het andere kanaal van dezelfde tweekanaalssensor.

rule-ew-saf-003 = { $channel }: de sensor { WORD($sensor) } wordt niet gevoed uit de testpulspen van de module; sluit zijn voeding op die pen aan
    .title = Testpuls
    .hint = Voed de sensor van een F-DI-kanaal uit de testpulspen van de module.

rule-ew-prc-001 = { $form ->
        [scheme] { WORD($device) }: het procesmerkteken { $tag } past niet in het schema { $scheme } van het project; corrigeer het merkteken
       *[duplicate] het procesmerkteken { $tag } staat op { $devices }; houd het op het ene veldapparaat en volg het vanaf de andere
    }
    .title = Schema van procesmerktekens
    .hint = Geef elk veldapparaat een eigen procesmerkteken in het schema van het project.

rule-ew-bus-001 = { $form ->
        [duplicate] { $named ->
            [none] { $untagged } poorten zonder kenmerk
           *[some] { $others ->
                [none] { $ports }
                [one] { $ports } en een poort zonder kenmerk
               *[other] { $ports } en { $untagged } poorten zonder kenmerk
            }
        }: stationsadres { $address } komt meer dan eens voor in logisch net { $net }; geef elke poort een eigen adres
       *[tbd] { $end ->
            [connector] een connector
            [device] een apparaat:{ $port }
           *[named] { $port }
        }: het stationsadres is TBD; vul het in vóór de vrijgave
    }
    .title = Busadres ontbreekt of dubbel
    .hint = Vul vóór de vrijgave een stationsadres in dat uniek is in het logische net.

rule-ew-bus-004 = { $form ->
        [external] extern einde { $port } ({ $partner })
       *[other] { $end ->
            [ip] einde { $port }
            [connector] een connector
            [device] een apparaat:{ $port }
            [segment] een segmenteinde
           *[named] { $port }
        }
    }: { $form ->
        [external] de afsluiting aan het externe einde is onbekend; controleer dat de partner het segment afsluit
        [missing] geen afsluitend element aan dit segmenteinde; sluit het segment hier af
        [end] de afsluiting staat uit aan het segmenteinde; zet haar aan
        [middle] de afsluiting staat aan binnen het segment; zet haar uit
       *[unknown] de afsluitstand is van een nieuwer formaat; controleer haar
    }
    .title = Afsluiting
    .hint = Zet de afsluiting aan op beide segmenteinden en uit overal daartussen.

rule-ew-bus-002 = { $port }: het stationsadres { $address } ligt buiten { $min }..{ $max }; vul een adres binnen het bereik in
    .title = Adres buiten het bereik
    .hint = Vul een stationsadres in binnen het bereik van de bus en van het artikel van de poort.

rule-ew-bus-003 = { $form ->
        [name] { $port }: de apparaatnaam { $name } is niet DNS-compatibel; gebruik kleine letters, cijfers, koppeltekens en punten
        [subnet] { $port }: het IP-adres { $ip } ligt buiten het subnet van de master, { $master_ip } met masker { $mask }; vul een adres van dat subnet in
        [duplicate_name] de apparaatnaam { $value } wordt gebruikt door { $ports } in logisch net { $net }; geef elke poort een eigen naam
       *[duplicate_ip] het IP-adres { $value } wordt gebruikt door { $ports } in logisch net { $net }; geef elke poort een eigen adres
    }
    .title = PROFINET-naam of IP
    .hint = Geef elk PROFINET-apparaat een eigen DNS-compatibele naam en een eigen IP-adres in het subnet van de master.

rule-ew-bus-005 = { $port } ondersteunt { $rate } bit/s niet, de snelheid van logisch net { $net }; kies een snelheid die elk station ondersteunt
    .title = Snelheid niet ondersteund
    .hint = Kies een snelheid die elk station van het logische net ondersteunt.

rule-ew-bus-006 = het segment vanaf { $segment } { $form ->
        [stations] heeft { $count } stations, meer dan de { $limit } die een PROFIBUS-segment neemt; deel het met een repeater
        [length] is { $metres } m lang, meer dan de { $limit } m die PROFIBUS bij { $rate } bit/s toestaat; maak het korter of verlaag de snelheid
       *[stub] vertakt, en PROFIBUS staat boven 1,5 Mbit/s geen aftakkingen toe; voer de lijn door elk station
    }
    .title = Segmentgrenzen
    .hint = Houd elk segment binnen het aantal stations, de lengte en de topologie die zijn snelheid toestaat.

rule-ew-bus-007 = logisch net { $net } heeft geen master; noem zijn masterpoort, of de externe partner die master is
    .title = Geen master
    .hint = Noem de master van het logische net: een poort van het project of de externe partner.

rule-ew-bus-009 = { $form ->
        [cable] { $port } is verbonden met { WORD($cable) }, dat geen kabel voor zijn bus is; gebruik een buskabel
       *[connector] { $port } ({ $connector }) is gekoppeld aan { $other } ({ $other_connector }); de connectortypen verschillen
    }
    .title = Verkeerde buskabel
    .hint = Verbind buspoorten met een kabel voor hun bus, en koppel connectoren van één type.

rule-ew-bus-010 = de poorten die met { $port } verbonden zijn, horen bij de logische netten { $nets }; zet elke poort van het segment in één logisch net
    .title = Netten verschillen
    .hint = Zet elke poort die busverbindingen samenvoegen in één logisch net.

rule-ew-iol-001 = { $form ->
        [mode] { $channel } staat in modus { $mode ->
            [deactivated] uitgeschakeld
           *[other] { $mode }
        }, maar het IO-Link-apparaat { WORD($device) } is aangesloten; zet de poort op IO-Link
       *[class] { $channel } is een poort van klasse { $port_class }, maar { WORD($device) } heeft klasse { $device_class } nodig; sluit het aan op een poort van klasse B
    }
    .title = Klasse van de IO-Link-poort
    .hint = Sluit elk IO-Link-apparaat aan op een poort van zijn klasse, in IO-Link-modus.

rule-ew-iol-002 = { $channel }: { $form ->
        [window] de { $have } { $dir ->
            [input] ingangs
           *[output] uitgangs
        }bits van het apparaat zijn meer dan het poortvenster van { $room } bits; gebruik een master met een groter venster
       *[cable] de kabel { WORD($cable) } is { $metres } m lang, meer dan de { $max } m die een IO-Link-poort toestaat; maak hem korter
    }
    .title = IO-Link-grenzen
    .hint = Houd elk IO-Link-apparaat binnen het venster en de kabellengte van zijn poort.

rule-ew-pot-001 = { $form ->
        [flow] de doorverbinding van functie { WORD($device) } verbindt type { $from } met type { $to } zonder omzettende functie
       *[attrs] { $conflicts }{ $more ->
            [none] {""}
           *[other] ; en nog { $more } aansluitpunten
        }{ $hint ->
            [bond] ; declareer de potentiaalverbinding met Tekenen ▸ Verbinden ▸ Potentiaalverbinding
           *[none] {""}
        }
    }
    .title = Type- of spanningsconflict
    .hint = Scheid de potentialen, corrigeer de definitie of declareer de potentiaalverbinding die ze samenbrengt.
    .conflict-type = { $subject } stelt type { $value } in; { $applied } geldt (eerste definitie in natuurlijke volgorde)
    .conflict-voltage = { $subject } stelt spanning { DECIMAL($value) } in; { DECIMAL($applied) } geldt (eerste definitie in natuurlijke volgorde)
    .conflict-environment = { $subject } stelt omgeving { $value } in; { $applied } geldt (eerste definitie in natuurlijke volgorde)
    .conflict-class = { $subject } stelt stroomkringklasse { $value } in; { $applied } geldt (eerste definitie in natuurlijke volgorde)
    .conflict-role = aansluitpunt { $subject } heeft rol { $value }; het gedefinieerde type { $applied } geldt
    .conflict-roles = aansluitpuntrollen { $value } botsen met { $applied }; de eerste in de rolvolgorde geldt

rule-ew-pot-002 = { $form ->
        [selv] SELV-potentiaal { $potential } is met PE verbonden ({ $count } 0 V–PE-verbinding(en)); een SELV-stroomkring mag niet geaard zijn
       *[pelv] PELV-potentiaal { $potential } { $bonds ->
            [none] heeft geen 0 V–PE-verbinding
           *[some] heeft { $count } 0 V–PE-verbindingen
        }; een PELV-stroomkring heeft er precies één nodig (declareer de potentiaalverbinding op de 0 V–PE-draad)
    }
    .title = 0 V–PE-verbinding bij PELV of SELV
    .hint = Definieer precies één 0 V–PE-verbinding voor een PELV-potentiaal, en geen voor een SELV-potentiaal.

rule-ew-pot-003 = { $cp_shared ->
        [yes] { $cp_tag } pen { $cp_pin } ({ $cp_nth }e)
       *[no] { $cp }
    } heeft de rol FE en is aangesloten op de PE-potentiaal { $potential }; declareer een FE–PE-verbinding of sluit het aan op functionele aarde
    .title = FE-pen op PE
    .hint = Declareer een FE–PE-verbinding of sluit de pen aan op functionele aarde.

rule-ew-pot-004 = draad { WORD($wire) } is GNYE op potentiaal { $potential }{ $typed ->
        [yes] {" "}(type { $type })
       *[no] {""}
    }; GNYE is voorbehouden aan beschermingsleiders
    .title = GNYE op een potentiaal die geen PE is
    .hint = Gebruik groen-geel alleen voor beschermingsleiders; geef de draad een andere kleur.

rule-ew-pot-005 = draad { WORD($wire) } is BU op de potentiaal { $potential }{ $type ->
        [none] {""}
       *[other] {" "}(type { $type })
    }; in een project met een nulleider markeert BU de nulleider
    .title = BU op een geleider die geen N is
    .hint = Gebruik een andere kleur voor de draad, of een kabel waarvan de blauwe ader de nulleider voert.

rule-ew-pot-006 = PE-draad { WORD($wire) } heeft { DECIMAL($pe) } mm², minder dan de { DECIMAL($required) } mm² die nodig is voor de lijngeleiders van { DECIMAL($line) } mm² bij { $at }{ $verified ->
        [yes] {""}
       *[no] {" "}(de dimensioneringstabel is niet getoetst aan de norm)
    }
    .title = PE te klein
    .hint = Vergroot de doorsnede van de beschermingsleider tot de vereiste maat.

rule-ew-pot-008 = { $form ->
        [terminal] { WORD($terminal) } is met de rail verbonden maar heeft geen locatie, en het project heeft { $locations } eigen locaties; de klem sluit op geen PE-knooppunt van een locatie aan
       *[island] { $location ->
            [main] de hoofdlocatie
           *[node] { WORD($node) }
        } heeft { $count } PE-potentialen: { $potentials }; verbind ze via een met de rail verbonden klem of een PE-draad
    }
    .title = PE-eiland
    .hint = Verbind de PE-potentialen van de locatie, bijvoorbeeld via de railverbinding van de klemmen.

rule-ew-pot-009 = { $cp_shared ->
        [yes] { $cp_tag } pen { $cp_pin } ({ $cp_nth }e)
       *[no] { $cp }
    } is niet aangesloten{ $unused ->
        [yes] {" "}(gemarkeerd als ongebruikt)
       *[no] ; sluit het aan of markeer het als ongebruikt
    }
    .title = Niet-aangesloten aansluitpunt
    .hint = Sluit het aansluitpunt aan of markeer het als ongebruikt.

rule-ew-pot-011 = { $potential }: het is niet duidelijk welke draad { $place ->
        [junction] de aftakking { $at ->
            [cell] in { $cell } op folio { $page }
           *[folio] op folio { $page }
        } voedt (draden { WORD($wires) })
        [ip] de onderbrekingspunten { $group } voedt (draden { WORD($wires) })
       *[other] een aftakking of een groep onderbrekingspunten voedt
    }; teken één draad ernaartoe vanaf de voedingszijde
    .title = Voeding van aftakking onduidelijk
    .hint = Teken de aftakking zo dat één draad haar voedt, of stel de bedradingsvolgorde in.

rule-ew-pot-012 = { $form ->
        [ring] de verbinding { WORD($wire) } sluit een ring in de tekening; ze heeft de hoogste ID van de ring en vormt geen fysieke draad
       *[other] { $detail }
    }
    .title = Ring in de tekening
    .hint = Verwijder de draad die de ring sluit, of bevestig dat de ring bedoeld is.

rule-ew-pot-013 = { WORD($device) } { $form ->
        [isolating] is een N-scheidingsklem op { $from } → { $to }
       *[switching] schakelt de nulleider { $from } → { $to }{ $loads ->
            [one] ; belasting { $load } blijft verbonden met { $line }
            [many] ; deze belastingen blijven verbonden: { $pairs }
           *[none] {""}
        }.
    }
    .title = Schakelen in de nulleider
    .hint = Verplaats het contact naar de lijngeleider, of gebruik een apparaat dat L en N tegelijk opent.

rule-ew-pot-014 = { $form ->
        [conductor] draad { WORD($wire) }: een deel ervan noemt een andere ader; de ader van de draad geldt
        [override] draad { WORD($wire) }: een deel ervan overschrijft een waarde die de draad al heeft; de eerste waarde geldt
       *[other] { $detail }
    }
    .title = Conflict in een fysieke draad
    .hint = Geef de fysieke draad één set eigenschappen, of splits hem in afzonderlijke draden.

rule-ew-pot-015 = draadnummer { $number } staat op { $count } draden; een draadnummer benoemt één nummeringseenheid
    .title = Draadnummerconflict
    .hint = Geef de draden van één potentiaal hetzelfde draadnummer, of scheid de potentialen.

rule-ew-pot-016 = de overschrijving { $member ->
        [csa] doorsnede { DECIMAL($value) }
        [colour] kleur { $value }
        [wire_part] draadartikel
        [insulation] isolatie
        [function] draadfunctie
        [shielded] afgeschermd
       *[twisted] getwist
    } van draad { WORD($wire) } is gelijk aan de waarde eronder in de overervingsketen; verwijder de overschrijving
    .title = Overbodige overschrijving
    .hint = Verwijder de overschrijving; ze zet de waarde die de draad toch al krijgt.

rule-ew-pot-017 = de definities van één potentiaal geven die de namen { $names }; de eerste in natuurlijke volgorde ({ $first }) geldt
    .title = Twee namen voor één potentiaal
    .hint = Houd één definitie van de potentiaal, of geef de onderbrekingspunten één naam.

rule-ew-pot-021 = { $cp_shared ->
        [yes] { $cp_tag } pen { $cp_pin } ({ $cp_nth }e)
       *[no] { $cp }
    } ligt op { $potential }, dat geen voeding bereikt: de belasting wordt gevoed maar niet teruggevoerd; sluit het aan op de retourgeleider (0 V, N)
    .title = Belasting niet teruggevoerd
    .hint = Verbind de retourzijde van de belasting met de retourgeleider van de voeding (0 V, N), of met de retourpin van de module.

rule-ew-rds-001 = { WORD($device) } is geen geldige IEC 81346-aanduiding: { $reason ->
        [empty] lege referentieaanduiding
        [prefix] het voorteken ontbreekt (zoals '-' of '+')
        [unexpected] onverwacht teken { $char } (U+{ $code })
        [empty-level] voorteken of scheidingsteken zonder niveau
        [invalid] teken { $char } (U+{ $code }) is hier niet toegestaan
        [whitespace] witruimte binnen de aanduiding
        [order] aspecten niet in de standaardvolgorde
        [repeated] een aspect komt meer dan eens voor
        [empty-cp] leeg aansluitpunt na ':'
        [cp-without-product] aansluitpunt zonder productaanduiding
        [empty-signal] lege signaalaanduiding na ';'
       *[other] { $detail }
    }
    .title = Ongeldige aanduiding
    .hint = Corrigeer de aanduiding naar de syntaxis van referentieaanduidingen volgens IEC 81346.

rule-ew-rds-002 = de structuurcode { $code } bevat scheidingstekens; programma's die geëxporteerde aanduidingen lezen, kunnen hem in meerdere niveaus opsplitsen
    .title = Scheidingsteken in een structuurcode
    .hint = Verwijder het scheidingsteken uit de code, of gebruik in plaats daarvan een structuurniveau.

rule-ew-rds-003 = { WORD($device) }: de lettercode { $letter } past niet bij de functiecategorie { $category }; de identificatieset { $set } stelt { $suggested } voor{ $verified ->
        [yes] {""}
       *[no] {" "}(de set is niet getoetst aan de norm)
    }
    .title = Lettercode tegen functie
    .hint = Gebruik de voorgestelde lettercode, of behoud het kenmerk als de classificatie bedoeld is.

rule-ew-rds-004 = `{ $tag }` wordt afgedrukt door { $count } apparaten
    .title = Dubbel kenmerk
    .hint = Geef een van de apparaten een ander kenmerk.

rule-ew-rds-007 = { WORD($device) }: de lettercode { $letter } gebruikt gereserveerde letters: { $io_form ->
        [none] {""}
        [one] { $io } wordt niet gebruikt in lettercodes
       *[many] { $io } worden niet gebruikt in lettercodes
    }{ $class_form ->
        [after] ; de identificatieset { $set } reserveert de klasseletter { $class }
        [alone] de identificatieset { $set } reserveert de klasseletter { $class }
       *[none] {""}
    }{ $other ->
        [yes] ; de identificatieset reserveert ze
       *[no] {""}
    }
    .title = Gereserveerde letters
    .hint = Vermijd de letters I en O in een lettercode; kies een andere letter.

rule-ew-rds-009 = { $kind ->
        [letter] de lettercode
        [frozen] het vastgezette kenmerk
       *[designation] de klemaanduiding
    } { $text } van { WORD($device) } bevat niet-Latijnse tekens die op Latijnse lijken: { $lookalikes }; zoeken en kruisverwijzingen vinden het niet; typ het opnieuw in Latijnse letters
    .title = Verwarrend teken
    .hint = Vervang het teken door zijn gewone Latijnse vorm.

rule-ew-rds-010 = de structuurcodes { $codes } verschillen alleen in scheidingstekens of hoofdletters; voeg ze samen tot één knooppunt en houd de andere schrijfwijzen als aliassen
    .title = Varianten van een structuurcode
    .hint = Voeg de structuurknooppunten samen waarvan de codes alleen in spelling verschillen.

rule-ew-rel-001 = revisie { $revision }: { $form ->
        [author] geen auteur ingesteld; stel die in met Controle ▸ Revisie ▸ Auteur van revisie instellen…
        [checker] geen controleur ingesteld; de controleur keurt de revisie goed
        [four-eyes] de controleur is de auteur ({ $role ->
            [customer] klant
            [end_user] eindgebruiker
            [panel_builder] paneelbouwer
            [author] auteur
            [checker] controleur
            [engineer] ontwerper
           *[other] { WORD($role) }
        }); het vierogenprincipe vraagt een andere partij
        [sign-off] goedkeuring ontbreekt; de controleur keurt de opgeslagen revisie goed
       *[stale] de goedkeuring past niet bij de opgeslagen inhoud (gewijzigd na goedkeuring); keur haar opnieuw goed
    }
    .title = Vier-ogenprincipe
    .hint = Stel de auteur in en laat daarna een ander de opgeslagen revisie goedkeuren; keur opnieuw goed na een inhoudelijke wijziging.

rule-ew-rel-002 =
    .title = Verouderde vastgelegde berekening
    .hint = Bereken het verouderde resultaat opnieuw en leg het vóór de vrijgave opnieuw vast.

rule-ew-rel-004 = revisie { $revision }: { $form ->
        [format] het label volgt niet de notatie van revisielabels van het project, { $pattern }
       *[order] het label van de open revisie komt niet na { $later }; revisielabels lopen op, de open revisie als laatste
    }
    .title = Volgorde van de revisielabels
    .hint = Geef de revisie een label in het labelformaat van het project dat na elk eerder label komt.

rule-ew-term-001 = een klem van { WORD($strip) } heeft geen aanduiding
    .title = Klem zonder aanduiding
    .hint = Geef de klem een aanduiding.

rule-ew-term-002 = { $form ->
        [taken] aanduiding { $designation } is al bezet op { $end ->
            [untagged] de klemmenstrook
           *[named] { WORD($strip) }
        } ({ $count } klemmen){ $remedy ->
            [free] ; gebruik { $free }
            [swap] ; verwissel { $old } ↔ { $designation } in één transactie (terminal.designate)
            [both] ; gebruik { $free } of verwissel { $old } ↔ { $designation } in één transactie (terminal.designate)
           *[none] {""}
        }
       *[used] aanduiding { $designation } wordt gebruikt door { $count } klemmen van { $end ->
            [untagged] de klemmenstrook
           *[named] { WORD($strip) }
        }
    }
    .title = Dubbele aanduiding
    .hint = Hernummer de klemmenstrook, of geef een van de klemmen een andere aanduiding.

rule-ew-term-003 = reserveklem { WORD($terminal) } is aan zijn veldzijde bedraad; bedraad hem aan de andere zijde of hef de reservestatus op
    .title = Reserveklem aan de veldzijde bedraad
    .hint = Verwijder de draad aan de veldzijde, of maak de klem een gebruikte klem.

rule-ew-term-004 = bruggroep { $group } van { WORD($strip) } is ongeldig: { $gap ->
        [yes] { $sep_gap }zijn klemmen liggen niet naast elkaar
       *[no] {""}
    }{ $shafts ->
        [yes] { $sep_shafts }hij loopt over { $shaft_count } brugschachten
       *[no] {""}
    }{ $reaches ->
        [yes] { $sep_reaches }hij reikt tot { WORD($reached) }
       *[no] {""}
    }{ $overlaps ->
        [yes] { $sep_overlaps }hij overlapt bruggroep { $overlapped } op zijn brugschacht
       *[no] {""}
    }; een bruggroep loopt over naast elkaar liggende klemmen van één klemmenstrook op één brugschacht
    .title = Ongeldige bruggroep
    .hint = Brug alleen naburige klemmen van één klemmenstrook, of zet de klemmen naast elkaar.

rule-ew-term-005 = { WORD($terminal) } is met de rail verbonden maar heeft geen PE-aansluitpunt; de klem sluit op geen PE-knooppunt van een locatie aan
    .title = Railverbinding op een klem die geen PE is
    .hint = Wis de railverbinding, of maak de klem een PE-klem.

rule-ew-term-006 = { WORD($terminal) } ligt op draad { WORD($wire) } zonder die te verbinden; sluit de klem op de draad aan of verplaats haar
    .title = Klem op een draad die hij niet verbindt
    .hint = Neem de klem op in de draad, of haal hem van de draad af.

## WP-S1.17 part 8 (COL49): botsingen na samenvoegen

rule-ew-core-013 = { $collision } na samenvoegen; { $keeper ->
        [frozen] het record met het vastgezette kenmerk
       *[oldest] het oudste record
    } behoudt het, en { $others ->
        [many] de andere moeten
       *[one] het andere moet
    } wijzigen
    .title = Botsing na samenvoegen
    .hint = Hernummer het nieuwere apparaat met de snelle oplossing, of geef een van de records een ander nummer.

## EW-IMP: de importbevindingen van het migratierapport (WP-S3.09 D9)

rule-ew-imp-001 =
    .title = Invoerlimiet overschreden; item of bestand geweigerd
    .hint = Splits de bron in kleinere bestanden, of verwijder de items boven de limiet, en importeer opnieuw.

rule-ew-imp-002 =
    .title = Onleesbaar of geweigerd item overgeslagen
    .hint = Corrigeer het item in de bron, of teken het opnieuw in dit project.

rule-ew-imp-003 =
    .title = Onoplosbare verwijzing weggelaten
    .hint = Teken de ontbrekende verbinding of koppeling opnieuw in dit project.

rule-ew-imp-004 =
    .title = Inhoud bewust weggelaten
    .hint = Niets te doen: deze inhoud heeft geen tegenhanger in een project; teken hem opnieuw als hij nodig is.

rule-ew-imp-005 =
    .title = Object afgeleid, controleren
    .hint = Controleer het afgeleide object en bevestig of splits het.

rule-ew-imp-006 =
    .title = Lettertype vervangen
    .hint = Niets te doen, of kies een andere tekststijl als de vervanger anders afdrukt.

rule-ew-imp-007 =
    .title = Waarde via een aliastabel omgezet
    .hint = Controleer de omgezette waarde; zet hem terug of kies een andere als de alias niet klopt.

rule-ew-imp-008 =
    .title = Waarde als vreemde of ondoorzichtige tekst bewaard
    .hint = Zet de waarde om naar een waarde die het project kent, met de snelle oplossing als die er is.

rule-ew-imp-009 =
    .title = Afwijking in de afdruk
    .hint = Vergelijk de afgeleide tekst met de afgedrukte tekst van de bron en corrigeer de verkeerde.

rule-ew-imp-010 =
    .title = In de bron verwijderd, hier behouden
    .hint = Verwijder het record als het niet meer nodig is; behoud het anders.

rule-ew-imp-011 =
    .title = In de bron en hier gewijzigd, hier behouden
    .hint = Vergelijk de wijziging in de bron met die in dit project en neem met de hand over wat nodig is.

rule-ew-imp-101 =
    .title = Opgeslagen label wijkt af van het formuleresultaat
    .hint = Gebruik het afgeleide kenmerk, of behoud het opgeslagen label als vastgezet kenmerk.

rule-ew-imp-103 =
    .title = Label als vastgezet kenmerk behouden
    .hint = Zet het kenmerk om naar het schema waar het past, of laat het vastgezet.

rule-ew-imp-104 =
    .title = Conflict in een titelblokwaarde per folio
    .hint = Maak de revisies die de folio's noemen en laat de waarden per pagina vallen.

rule-ew-imp-109 =
    .title = Klemmenstrookdelen samengevoegd tot één klemmenstrook
    .hint = Niets te doen, of splits de klemmenstrook als de delen aparte klemmenstroken zijn.

rule-ew-imp-115 =
    .title = Namen van een bladverwijzingspaar verschillen
    .hint = Hernoem een kant van het paar, of zet de koppeling op ketenmodus.

rule-ew-imp-116 =
    .title = Slaaf zonder meester, of meester boven zijn slaven
    .hint = Koppel de slaaf aan zijn meester, of verminder de koppelingen van de meester.

rule-ew-imp-117 =
    .title = Standaardformule voor de bladindex
    .hint = Laat de bladverwijzingen het bladlabel gebruiken.

rule-ew-imp-118 =
    .title = Bedrading van een knooppunt aangenomen
    .hint = Zet de bedrading van het knooppunt op Keten waar de bron het zo bedraadde.

rule-ew-imp-119 =
    .title = Element zonder functiebetekenis
    .hint = Koppel het symbool aan een functiedefinitie, of behoud de algemene black box.

rule-ew-imp-120 =
    .title = Gedeelde element-UUID onderscheiden
    .hint = Niets te doen: elke kopie kreeg een eigen record.

rule-ew-imp-122 =
    .title = Element zonder label, kenmerk gemaakt
    .hint = Geef het apparaat een label; zijn kenmerk drukt tot dan "?" af.

rule-ew-imp-102 =
    .title = Spellingsvarianten van een structuurcode
    .hint = Voeg de varianten samen tot een structuurcode en behoud de andere als aliassen.

rule-ew-imp-105 =
    .title = Beschrijving per blad als overschrijving behouden
    .hint = Laat de overschrijving vallen om de projectbeschrijving weer af te drukken, of behoud hem.

rule-ew-imp-114 =
    .title = Geïmporteerde tekststijlen niet aan slotsoorten gebonden
    .hint = Pas de tekststijl toe die de slotsoort noemt.

# De labels van de snelle oplossingen (FT-1482, ID-1457), per sleutel (`QuickFix::key`).
fix-drop-override = overschrijving verwijderen
fix-anchor-graphics-centre = anker op het midden van de grafische elementen
fix-renumber-newer = De nieuwere hernummeren
fix-unfreeze-tag = Kenmerk losmaken
fix-realise-primary-part = Het primaire artikel opnieuw plaatsen
fix-rebind-map-channel = Koppelen aan het kanaal van de kaart
fix-declare-pelv-bond = 0 V–PE-verbinding (PELV) vastleggen

# De woorden die een regel afdrukt waar een record geen eigen naam heeft (FT-912), per sleutel; WORD(…) in een patroon
# drukt ze in het Nederlands af.
word-device = apparaat
word-cable = kabel
word-without-a-tag = zonder kenmerk
word-a-junction = een aftakking
word-assembly = samenstelling
word-a-location = een locatie
word-a-device-without-designation = een apparaat zonder aanduiding
word-a-missing-part = een ontbrekend artikel
word-an-unknown-profile = een onbekend profiel
word-a-placement = een plaatsing
word-none = geen
word-terminal = klem
word-strip = klemmenstrook
word-another-strip = een andere klemmenstrook
word-valve = ventiel
word-the-valve = het ventiel
word-a-device = een apparaat
word-a-module = een module
word-a-part = een artikel
word-a-configuration-project = een configuratieproject
word-no-address = geen adres
word-a-role-of-a-newer-format = een rol van een nieuwer formaat
word-junction = aftakking
word-a-drawn-page = een getekende pagina
word-a-section-divider = een sectiescheiding
word-a-report-page = een rapportpagina
word-the-strip = de klemmenstrook
word-a-device-that-is-not-a-terminal = een apparaat dat geen klem is
word-the-project = het project
word-a-page = een pagina
word-a-logical-bus-net = een logisch busnet
word-a-physical-bus-net = een fysiek busnet
word-the-junction-in = de aftakking in { $at }
word-a-connection-point-of-another-page = een aansluitpunt van een andere pagina
word-its-target = zijn doel
word-the-station = het station
word-a-station = een station
word-an-f-module = een F-module
word-the-sensor = de sensor
word-the-cable = de kabel
word-the-device = het apparaat
