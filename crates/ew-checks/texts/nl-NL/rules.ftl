# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: GPL-3.0-or-later

# The rule messages in nl-NL (doc 15 §3.8). A rule or attribute missing here falls back to en-GB; until
# 1.0 the message test lists the missing ones as warnings.

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
        [duplicate_number] { $module } heeft een tweede kanaal met nummer { $number }; de module houdt één kanaal per nummer, dus dit kanaal wordt nergens afgedrukt
        [channel] kanaal { $channel } hoort bij { $module }, dat geen PLC-module is; verwijder het kanaal of maak van { $module } weer een module
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
            [device] apparaat { $tag }
            [page] folio { $page }
            [function] een functie van apparaat { $tag }
            [connection-on] verbinding van { $from } naar { $to } op folio { $page }
            [connection] verbinding van { $from } naar { $to }
           *[other] { $kind }
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

rule-ew-core-022 = folio { $page } wordt door { $count } pagina's afgedrukt ({ $kinds }); geef elke pagina een eigen folio
    .title = Dubbel folio
    .hint = Geef een van de pagina's een ander folio; twee pagina's delen nooit een folio.

rule-ew-core-023 = folio { $page } overschrijft de projectomschrijving in zijn titelblok met "{ $text }"
    .title = Overschrijving in het titelblok
    .hint = Verwijder de overschrijving om de gekoppelde projectwaarde weer af te drukken, of behoud haar als de pagina een eigen tekst nodig heeft.

rule-ew-core-025 = de inhoudsopgave kwam niet tot een vast aantal pagina's; ze gebruikt { $pages } pagina's en haar laatste pagina vermeldt alle overige regels
    .title = Inhoudsopgave stabiliseert niet
    .hint = Verkort de gegenereerde inhoudsopgave of haar paginaregels, zodat haar aantal pagina's vast komt te liggen.

rule-ew-core-026 = { $family ->
        [terminal_diagram] klemmenstrook { $device } staat in geen klemmenschema
        [cable_overview_diagrams] kabel { $device } staat in geen kabeloverzicht en kabelschema's
       *[plc_overview] PLC-station { $device } staat in geen PLC-overzicht
    }: geen rapportblok van die soort dekt het; voeg er een toe of neem het op
    .title = Rapport dekt niet alle records
    .hint = Voeg een rapportblok van die soort toe, of sluit de records bewust uit van het rapport.

rule-ew-core-031 = ader { $core } heeft de kleur { $colour }: { $codes } is geen IEC 60757-code en geen bedrijfscode van het kleurprofiel
    .title = Ongeldige aderkleur
    .hint = Kies een kleurcode uit de norm voor aderkleuren, bijvoorbeeld BN in plaats van BR.

rule-ew-core-032 = ader { $core } { $form ->
        [foreign] is geen ader van het kabelartikel { $part }
       *[differs] wijkt af van ader { $def } van het kabelartikel { $part }: { $colour ->
            [yes] { $sep_colour }kleur { $colour_core } ≠ { $colour_part }
           *[no] {""}
        }{ $csa ->
            [yes] { $sep_csa }doorsnede { DECIMAL($csa_core) } ≠ { DECIMAL($csa_part) }
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
            [yes] { $sep_pair }paar { $pair_core } ≠ { $pair_part }
           *[no] {""}
        }
    }
    .title = Ader wijkt af van het artikel
    .hint = Gebruik de ader zoals het kabelartikel haar definieert, of kies het artikel dat deze ader heeft.

rule-ew-core-033 = { $cable } heeft nog geen lengte: typ die in de eigenschappen, of routeer de kabel in de layout
    .title = Kabel zonder lengte
    .hint = Typ de lengte in de eigenschappen van de kabel, routeer de kabel in de layout of kies een kabelartikel met een vaste lengte. Alleen in de controle voor oplevering is het een fout.

rule-ew-core-034 = de externe verbinding { $far ->
        [unknown] van een onbekende plek naar { $inside }
       *[known] { $connection }
    } heeft geen kabel: { $count ->
        [one] draad { $wires } is
       *[many] draden { $wires } zijn
    } geen kabelader; voeg de kabel toe en wijs de aders toe
    .title = Externe verbinding zonder kabel
    .hint = Voeg de kabel toe die de locatie verlaat en wijs de aders toe aan de draden.

rule-ew-core-036 = ader { $core } is aan { $count } draden toegewezen ({ $wires }); een ader draagt één draad
    .title = Ader dubbel toegewezen
    .hint = Wijs de ader aan één verbinding toe, of gebruik een andere ader van de kabel.

rule-ew-core-037 = afscherming { $core } is aan geen van beide einden aangesloten; sluit haar aan één einde of aan beide aan
    .title = Afscherming niet aangesloten
    .hint = Sluit de afscherming aan op een aardingspunt, of markeer haar als bewust niet aangesloten.

rule-ew-core-041 = { $device } heeft geen artikel, dus de stuklijst kan het niet bestellen. Kies er een in zijn eigenschappen (Artikel).
    .title = Apparaat zonder artikel
    .hint = Open de melding: de eigenschappen van het apparaat openen. Kies daar zijn artikel (Artikel), uit de bibliotheek of een eigen artikel.

rule-ew-core-042 = artikel { $part } van { $device } heeft geen { $missing ->
        [description] omschrijving
        [manufacturer] fabrikant
       *[both] omschrijving en geen fabrikant
    }
    .title = Onvolledige artikelgegevens
    .hint = Vul de gegevens van het artikel aan in de bibliotheek en werk de ingebedde kopie bij.

rule-ew-core-043 = tijdelijk artikel "{ $part }" (artikelnummer TBD) wordt gebruikt door { $users }; kies het artikel vóór de vrijgave
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
        [start] { $module }: het startadres van de { $which ->
            [input] ingangen
           *[output] uitgangen
        } is TBD; vul het in vóór de vrijgave
       *[channel] { $module }: het adres van kanaal { $number } is TBD; vul het in vóór de vrijgave
    }
    .title = Open TBD-waarde
    .hint = Vul vóór de vrijgave de waarde in die nog als TBD gemarkeerd is.

rule-ew-core-045 = { $device } gebruikt { $need_no } maakcontacten, { $need_nc } verbreekcontacten en { $need_co } wisselcontacten; zijn artikelen hebben er { $have_no }, { $have_nc } en { $have_co }
    .title = Meer hulpcontacten dan het artikel heeft
    .hint = Verwijder contacten uit de tekening, of kies een artikel met genoeg hoofd- en hulpcontacten.

rule-ew-core-047 = { $device } is op geen pagina getekend, daarom toont het kenmerk ? waar het paginanummer hoort{ $part ->
        [yes] , en de stuklijst telt het nog mee
       *[no] {""}
    }. Teken het op een pagina, of verwijder het als het niet nodig is.
    .title = Apparaat niet getekend
    .hint = Open de melding: de eigenschappen van het apparaat openen. Plaats het vanuit de lijst Apparaten op een pagina (sleep het op de pagina), of verwijder het.

rule-ew-core-085 = { $device } noemt de variant { $variant }, die { $part } niet heeft
    .title = Variant niet in het artikel
    .hint = Kies in Eigenschappen ▸ Artikelen ▸ Variant een variant die het artikel heeft; tot dan drukken de lijsten het artikel af.

rule-ew-core-048 = { $device } wordt { $form ->
        [covered-twice] gedekt door { $count } bestelcodes van { $assembly }; zijn artikelen zouden { $count } keer worden afgedrukt
       *[uncovered] door geen enkele bestelcode van { $assembly } gedekt; de stuklijst van de samenstelling vermeldt zijn eigen artikelen naast de geconfigureerde producten
    }
    .title = Bestelcodes dekken de samenstelling niet
    .hint = Neem de ontbrekende leden op in een bestelcode van de samenstelling, of verwijder ze uit de samenstelling.

rule-ew-core-051 = onderbrekingspunt { $name } { $form ->
        [no-cp] heeft geen aansluitpunt; het vormt met niets een paar
        [target] op { $at } is een doel zonder bron: plaats de bron waar de potentiaal vertrekt, of maak van deze markering de bron
        [source] op { $at } is een bron zonder doel: plaats een doel waar de potentiaal aankomt, of maak van deze markering een doel
       *[unpaired] heeft geen tegenhanger; zijn draad eindigt open
    }
    .title = Onderbrekingspunt zonder tegenhanger
    .hint = Een potentiaal komt van één bron en gaat verder bij zijn doelen, elk een punt met dezelfde naam: plaats het punt dat deze markering mist, of wijzig de Richting ervan in Eigenschappen.

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

rule-ew-core-061 = { $name } drukt { DECIMAL($height) } mm hoog af, te klein om op papier te lezen (minstens { DECIMAL($minimum) } mm). Maak het groter in Tekststijlen.
    .title = Tekst onder de minimale hoogte
    .hint = Open de melding: Tekststijlen opent bij de stijl. Maak hem daar minstens 1,2 mm hoog, of kies Op 1,2 mm zetten.

rule-ew-core-063 = een tekst op folio { $page } heeft de grootte { $style }, die geen tekststijl vastlegt; geef de tekst een tekststijl
    .title = Tekstgrootte zonder tekststijl
    .hint = Geef de tekst een van de tekststijlen van het project in plaats van een vrije grootte.

rule-ew-core-065 = omschrijving "{ $text }" van artikel { $part }{ $form ->
        [rating] {" "}bevat de nominale waarde { $matches }: verplaats die naar de technische gegevens
       *[unit-format] : eenheidsnotatie { $matches } (een spatie tussen getal en eenheid, eenheden geschreven zoals in SI)
    }
    .title = Technische gegevens in de omschrijving
    .hint = Verplaats nominale waarden en technische gegevens uit de omschrijving naar hun eigen velden.

rule-ew-core-067 = { $line ->
        [tube] Slang
       *[wire] Draad
    } { $wire } kruist { $point ->
        [junction] de aftakking in { $pin }
        [junction-off-grid] een aftakking
       *[cp] { $pin }
    } zonder ermee verbonden te zijn, dus op papier lijkt hij aangesloten. Routeer hem opnieuw zodat hij ernaast loopt.
    .title = Draad door een aansluitpunt
    .hint = Open de melding: de draad is geselecteerd op zijn pagina. Kies Routeren, of sleep zijn ronde grepen zodat hij vrij blijft van het punt.

rule-ew-core-068 = { $line ->
        [tube] Slang
       *[wire] Draad
    } { $wire } ligt over { $other } op pagina { $page }, dus op papier zijn de twee niet uit elkaar te houden. Routeer een van beide opnieuw.
    .title = Draad over een andere draad
    .hint = Open de melding: beide draden zijn geselecteerd op hun pagina. Kies Routeren, of sleep de grepen van één draad, zodat elke draad een eigen lijn heeft.

rule-ew-core-069 = { $line ->
        [tube] Slang
       *[wire] Draad
    } { $wire } loopt door { $symbol ->
        [ip] onderbrekingspunt { $device }
       *[device] { $device }
    }.
    .title = Draad door een symbool
    .hint = Selecteer de draad en gebruik Routeren, of verplaats het symbool uit de weg.

rule-ew-core-070 = { $line ->
        [tube] Slang
       *[wire] Draad
    } { $wire } loopt door { $owner ->
        [free-text] de vrije tekst "{ $text }"
        [structure-box] het bijschrift "{ $text }" van een structuurkader
        [ip] de tekst "{ $text }" van onderbrekingspunt { $device }
        [wire] het nummer "{ $text }" van { $device }
        [mark] de kabelmarkering "{ $text }"
       *[symbol] de tekst "{ $text }" van { $device }
    }.
    .title = Draad door een tekst
    .hint = Selecteer de draad en gebruik Routeren, of verplaats de tekst.

rule-ew-core-071 = een grafische lijn eindigt op { $cp }, wat elektrisch niets verbindt; teken in plaats daarvan een verbinding
    .title = Grafische lijn op een aansluitpunt
    .hint = Vervang de grafische lijn door een verbinding, zodat ze meetelt in de connectiviteit.

rule-ew-core-072 = { $place ->
        [off-grid] Een aftakking op folio { $page } verbindt vier draden: teken in plaats daarvan twee T-aftakkingen.
       *[cell] De aftakking in { $cell } verbindt vier draden: teken in plaats daarvan twee T-aftakkingen.
    }
    .title = Vierwegaftakking
    .hint = Verplaats één tak naar een eigen aftakking een rasterstap verder, zodat elke aftakking drie draden verbindt.

rule-ew-core-073 = { $line ->
        [tube] Slang { $wire } heeft geen route: zij is als gestreepte lijn getekend.
       *[wire] Draad { $wire } heeft geen route: hij is als gestreepte lijn getekend.
    }
    .title = Draad zonder route
    .hint = Gebruik Routeren; waar dat geen vrije weg vindt, versleep de grepen van de draad of verplaats de symbolen.

rule-ew-core-074 = { $line ->
        [tube] Slangen
       *[wire] Draden
    } { $wire } en { $other } maken een bocht op hetzelfde punt { $place ->
        [off-grid] op folio { $page }
       *[cell] in { $cell }
    } zonder aftakking, dus op papier lijken ze verbonden. Routeer een van beide opnieuw.
    .title = Draden raken elkaar zonder verbinding
    .hint = Open de melding: beide draden zijn geselecteerd. Routeer er een opnieuw, of sleep een bocht een rasterstap verder.

rule-ew-core-075 = { $owner ->
        [free-text] De vrije tekst "{ $text }"
        [structure-box] Het bijschrift "{ $text }" van een structuurkader
        [ip] De tekst "{ $text }" van onderbrekingspunt { $device }
        [wire] Het nummer "{ $text }" van { $device }
        [mark] De kabelmarkering "{ $text }"
       *[symbol] De tekst "{ $text }" van { $device }
    } staat op de tekst "{ $other }" { $place ->
        [off-grid] op folio { $page }
       *[cell] in { $cell }
    }, zodat geen van beide te lezen is. Verplaats een van beide.
    .title = Tekst op een tekst
    .hint = Sleep een van de teksten naar een vrije plek, of verplaats het symbool; een tekst die u verplaatst, blijft waar u hem zet.

rule-ew-core-076 = { $owner ->
        [free-text] De vrije tekst "{ $text }"
        [structure-box] Het bijschrift "{ $text }" van een structuurkader
        [ip] De tekst "{ $text }" van onderbrekingspunt { $device }
        [wire] Het nummer "{ $text }" van { $device }
        [mark] De kabelmarkering "{ $text }"
       *[symbol] De tekst "{ $text }" van { $device }
    } staat op { $symbol ->
        [ip] onderbrekingspunt { $part }
       *[device] { $part }
    } { $place ->
        [off-grid] op folio { $page }
       *[cell] in { $cell }
    }. Verplaats de tekst of het symbool.
    .title = Tekst op een symbool
    .hint = Sleep de tekst naar een vrije plek, of verplaats het symbool waar hij op staat.

rule-ew-core-077 = { $symbol ->
        [ip] Onderbrekingspunt { $device }
       *[device] { $device }
    } { $form ->
        [outside] staat buiten het tekengebied van pagina { $page }. Verplaats het binnen het kader.
       *[part] ligt op { $other_symbol ->
            [ip] onderbrekingspunt { $other }
           *[device] { $other }
        }. Verplaats een van beide.
    }
    .title = Symbool op een symbool
    .hint = Open de melding: de symbolen zijn geselecteerd. Sleep een van beide naar een vrije plek.

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
       *[scope] kon niet worden beoordeeld voor { $scope }: hij stopte met een interne fout, dus zijn bevindingen daar ontbreken; meld het defect
    }
    .title = Regel niet geëvalueerd
    .hint = Meld het defect; de bevindingen van de regel in dat bereik ontbreken tot het is verholpen.

rule-ew-fl-001 = { $form ->
        [many] { $valve }: werkpoorten { $ports } zijn open; sluit ze aan, zet er een blindstop op of blind ze af
       *[one] { $valve }: werkpoort { $port } is open; sluit hem aan, zet er een blindstop op of blind hem af
    }
    .title = Werkpoort open
    .hint = Teken de werkleiding, of sluit de poort af: met een blindstop, met een geluiddemper of afgeblind.

rule-ew-fl-002 = { $form ->
        [excess] { $valve }: magneetspoel { $solenoid } is aan { $count } kanalen gebonden: { $channels }
        [undeclared] { $valve }: { $channel } stuurt een spoel aan die het artikel niet declareert
       *[unbound] { $valve }: geen PLC-uitgang stuurt magneetspoel { $solenoid } aan; koppel er een aan
    }
    .title = Magneetspoelen en hun uitgangen
    .hint = Geef elke magneetspoel die het ventielartikel noemt één PLC-uitgang (op een ventieleiland met busknooppunt een kanaal van het ventieleiland, op een multipol-ventieleiland de uitgang die met een draad op zijn stekkerpen is aangesloten), en geen uitgang aan een andere spoel.

rule-ew-fl-004 = { $form ->
        [blank] { $module }: ventielplaats { $position } is afgesloten met een blindplaat, { $device }
       *[valve] { $module }: ventielplaats { $position } bevat een reserveventiel, { $device }
    }
    .title = Ventielplaatsen in reserve
    .hint = Houd het reserveventiel of de blindplaat bewust, of plaats een gebruikt ventiel op de ventielplaats.

rule-ew-fl-005 = het fluïdumnet bij { $at } voert { $media } rechtstreeks; houd één medium per net
    .title = Twee media in één net
    .hint = Verwijder een van de mediumdefinities, of scheid de netten.

rule-ew-fl-006 = { $form ->
        [od] slang { $tube } is { $od } mm buitendiameter, maar poort { $port } neemt { $port_od } mm
       *[rating] slang { $tube } is berekend op { $rating } bar, minder dan de { $pressure } bar in zijn net
    }
    .title = Slang past niet
    .hint = Kies een slang die bij de poort en bij de druk van het medium past.

rule-ew-fl-007 = { $missing ->
        [material] slang { $tube } is { $size } mm maar heeft geen materiaal; geef de standaardslang van haar medium of van het project een materiaal, of stel het in op de slang
       *[spec] slang { $tube } heeft geen slangspecificatie; geef het medium of het project een standaardslang, of stel haar in op de slang
    }
    .title = Slang zonder specificatie
    .hint = Geef het medium of het project een standaardslang, of stel haar in op de slang.

rule-ew-fl-008 = { $form ->
        [gallery] { $terminal }: verdeelkanaal { $gallery } ({ $count } afvoerpoorten) heeft geen geluiddemper, verzamelleiding of afsluiting
       *[ports] afvoer { $ports } heeft geen geluiddemper, verzamelleiding of afsluiting
    }
    .title = Afvoer open
    .hint = Plaats een geluiddemper, teken een verzamelleiding of sluit de afvoer af.

rule-ew-fl-009 = { $valve }: de spoel neemt { $voltage }, maar { $feeds } voeden haar
    .title = Spoelspanning
    .hint = Kies een ventiel waarvan de spoel de spanning van zijn voeding neemt, of voed haar uit een potentiaal van die spanning.

rule-ew-fl-010 = { $form ->
        [valve] { $device }: er komt geen perslucht op poort { $port }. Teken de voedingsleiding erheen, of kies daar Medium aan poort definiëren.
       *[gallery] { $device }: er komt geen perslucht bij de ventielen, omdat er niets op de voeding van het ventieleiland is aangesloten. Teken de voedingsleiding naar de voedingspoort, of kies daar Medium aan poort definiëren.
    }
    .title = Geen voeding
    .hint = Teken de leiding die de lucht van de voeding naar de poort brengt, of klik met rechts op de poort en kies Medium aan poort definiëren.

rule-ew-fl-011 = { $valve } stuurt { $target } aan, maar de leidingen van zijn werkpoorten bereiken { $device }
    .title = Aangestuurd apparaat wijkt af
    .hint = Koppel het ventiel aan het apparaat dat zijn leidingen bereiken, of teken de leidingen naar het apparaat dat het aanstuurt.

rule-ew-fl-012 = { $form ->
        [role] werkpoort { $working } is verbonden met { $other } zonder ventiel ertussen
       *[valve] { $ports } zijn poorten van één ventiel en zijn verbonden
    }
    .title = Poortfuncties in conflict
    .hint = Teken elke werkleiding naar een verbruiker, en verbind geen twee poorten van één ventiel.

rule-ew-fl-013 = { $form ->
        [both] { $parent }: ventielplaats { $position } bevat zowel { $a } als { $b }
        [modules] { $parent }: positie { $position } bevat zowel { $a } als { $b }
        [outside] { $module }: ventielplaats { $position } ligt buiten de { $count } ventielplaatsen
        [outside_free] { $module }: ventielplaats { $position } ligt buiten de { $count } ventielplaatsen; vrije ventielplaatsen: { $free }
        [outside_full] { $module }: ventielplaats { $position } ligt buiten de { $count } ventielplaatsen; elke ventielplaats is bezet
       *[empty] { $module }: ventielplaats { $position } is leeg; plaats een ventiel, of een blindplaat (rechtsklik op de plaats: Blindplaat)
    }
    .title = Bezetting
    .hint = Geef elke ventielplaats één ventiel of blindplaat (rechtsklik op de plaats: Blindplaat), en elke module een eigen positie.

rule-ew-fl-014 = slang { $tube } gaat van { $from } naar { $to } zonder schotdoorvoer
    .title = Slang verlaat een locatie
    .hint = Plaats een schotdoorvoer waar de slang de locatie verlaat, of aanvaard de overgang.

rule-ew-lay-001 = { $device } is niet geplaatst in de layout van { $location }; plaats het op een rail of op de montageplaat
    .title = Niet geplaatst apparaat
    .hint = Plaats het apparaat op een rail of op de montageplaat van de layout van zijn locatie.

rule-ew-lay-003 = de plaatsing van { $device } gebruikt artikel { $part }, dat het apparaat niet meer heeft; plaats het apparaat met een van zijn artikelen, of geef het het artikel terug
    .title = Artikel niet op het apparaat
    .hint = Plaats het apparaat opnieuw met een van zijn artikelen, of geef het het artikel van zijn plaatsing terug.

rule-ew-lay-004 = { $device } { $location ->
        [none] heeft geen locatie
       *[other] staat in { $location }
    } maar is geplaatst in de layout van { $space }; verplaats het naar de layout van zijn locatie, of corrigeer zijn locatie
    .title = Locatie wijkt af
    .hint = Verplaats het apparaat naar de layout van zijn eigen locatie, of corrigeer zijn locatie.

rule-ew-lay-006 = { $form ->
        [placements] { $device } overlapt { $other }; verplaats een van beide
        [same] twee plaatsingen van { $device } overlappen; verplaats een van beide
        [limit] meer voetafdrukken overlappen op de montageplaat van { $enclosure }; de eerste { $listed } overlappingen staan vermeld
        [duct] { $device } overlapt de kabelgoot bij { $duct }; verplaats het apparaat of de kabelgoot
       *[rail] de rail bij { $rail } overlapt de kabelgoot bij { $duct }; verplaats de rail of de kabelgoot
    }
    .title = Botsing
    .hint = Verplaats een van de overlappende objecten tot hun voetafdrukken niet meer overlappen.

rule-ew-lay-010 = { $form ->
        [on_duct] { $device } is geplaatst op de kabelgoot bij { $duct }, die geen apparaten draagt; plaats het op een rail
        [off_rail] { $device } is geplaatst op { DECIMAL($s) } mm op de rail bij { $rail }, buiten { DECIMAL("0.0") } tot { DECIMAL($length) } mm; verplaats het op de rail
        [rail_turn] { $device } is op de rail bij { $rail } gedraaid over een andere hoek dan 0° of 180°; draai het terug
        [off_axis] { $device } is naast de as van de rail bij { $rail } geplaatst; verplaats het op de rail
        [profile] { $device } (artikel { $part }) past niet op de rail bij { $rail } ({ $profile }); plaats het op een rail waar het op past
        [surface_turn] { $device } is op de montageplaat gedraaid over een andere hoek dan een veelvoud van 90°; draai het terug
        [off_surface] { $device } ligt niet in het vlak van de montageplaat; plaats het op de plaat
        [surface] { $device } (artikel { $part }) wordt niet op de plaat zelf gemonteerd; plaats het op een rail
        [overflow] { $device } beslaat { DECIMAL($from) } tot { DECIMAL($to) } mm op de rail bij { $rail }, voorbij { DECIMAL("0.0") } tot { DECIMAL($length) } mm; maak het korter of verplaats het
        [outside] { $device } ligt buiten het bruikbare vlak van de montageplaat; verplaats het naar binnen
        [bracket_overlap] De eindsteun { $side ->
            [before] vóór
           *[after] na
        } { $device } overlapt { $other_kind ->
            [bracket] de eindsteun { $other_side ->
                [before] vóór
               *[after] na
            } { $other }
           *[placement] { $other }
        } op { $rail }.
        [bracket_outside] De eindsteun { $side ->
            [before] vóór
           *[after] na
        } { $device } steekt { DECIMAL($by) } mm voorbij { $edge ->
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
        [spaces] { $location } heeft een tweede layoutruimte; een locatie heeft één layoutruimte; verwijder deze
        [enclosures] de layout van { $location } heeft een tweede kast, { $enclosure }; een layoutruimte heeft één kast; verwijder deze
       *[plates] de kast { $enclosure } heeft een tweede montageplaat; een kast heeft één montageplaat; verwijder deze
    }
    .title = Drager past niet
    .hint = Plaats het apparaat op een drager waar het op past, binnen de drager en het bruikbare vlak van de plaat.

rule-ew-lay-016 = { $form ->
        [order] de leden van { $assembly } op de rail bij { $rail } staan niet op volgorde: { $early } hoort vóór { $late }; plaats eerst de ouder, dan de leden op positie
       *[between] { $other } staat tussen leden van { $assembly } op de rail bij { $rail }; houd de leden bij elkaar
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
       *[overlap] adressen overlappen in configuratieproject { $cfg }: { $channels }{ $more ->
            [none] {""}
           *[other] ; en nog { $more } kanalen
        }; wijzig adressen tot er geen meer overlappen
    }
    .title = Adresoverlap
    .hint = Wijzig de adressen van de kanalen tot er geen meer overlappen, of laat ze een automatisch adres nemen.

rule-ew-plc-002 = { $channel }: het adres { $address } ligt buiten het I/O-bereik van { $module } ({ $range ->
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

rule-ew-plc-007 = het symbool { $symbol } wordt gebruikt door { $count } kanalen van configuratieproject { $cfg }: { $channels }; geef elk kanaal een eigen symbool
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
       *[binding] { $channel } ({ $address }) stuurt spoel { $solenoid } van { $valve } aan, die de positiekaart van artikel { $part } toewijst aan { $expected } ({ $expected_address }); koppel het kanaal van de kaart, of corrigeer de PLC-sectie van het artikel
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
        [input] { $channel }: het { $kind }-kanaal leest de spoel van { $target }; een ingang leest een sensor, controleer de bedrading of de kanaalsoort
       *[output] { $channel }: het { $kind }-kanaal stuurt het contact van { $target } aan; een uitgang stuurt een actuator aan, controleer de bedrading of de kanaalsoort
    }
    .title = Kanaalsoort past niet
    .hint = Verbind het kanaal met een draad naar een functie van zijn soort, of wijzig de kanaalsoort.

rule-ew-plc-005 = { $channel }: het symbool { $symbol } is geen IEC 61131-3-identifier, wat configuratieproject { $cfg } vereist; gebruik letters, cijfers en enkele underscores
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
        [occupied] { $module } en { $other } zitten in hetzelfde slot { $slot } van { $station }; verplaats een van beide naar een vrij slot
        [slot] { $module } zit in slot { $slot } van { $station }, maar zijn artikel { $part } staat slots { $allowed } toe; verplaats het naar een toegestaan slot
        [max_modules] { $station } heeft { $have } modules, meer dan de { $limit } die zijn artikel toestaat; verwijder modules
        [input] { $station } heeft { $have } ingangsbytes, meer dan de { $limit } die zijn artikel toestaat; verwijder modules
       *[output] { $station } heeft { $have } uitgangsbytes, meer dan de { $limit } die zijn artikel toestaat; verwijder modules
    }
    .title = Slotregels
    .hint = Plaats elke module in een eigen slot dat zijn artikel toestaat, en houd het station binnen zijn grenzen.

rule-ew-plc-013 = { $form ->
        [fe] { $cp_shared ->
            [yes] { $cp_tag } pen { $cp_pin } ({ $cp_nth }e)
           *[no] { $cp }
        } is de FE-pen van een PLC-kaart en wordt gevoed uit de PE-potentiaal { $potential }; sluit hem aan op functionele aarde, of declareer een FE–PE-verbinding
        [unconnected] { $cp }, de { $role }-pen van de { $group ->
            [electronics] elektronica-
            [load] belasting
           *[other] {""}
        }voedingsgroep, is niet aangesloten; sluit de voeding van de kaart aan
       *[wrong] { $cp }, de { $role }-pen van de { $group ->
            [electronics] elektronica-
            [load] belasting
           *[other] {""}
        }voedingsgroep, wordt gevoed uit een potentiaal van type { $type }; voed hem uit { $role }
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

rule-ew-plc-017 = { $device } staat als module in de PLC, maar zijn artikel { $part } beschrijft geen PLC-module; haal het uit het rek (een busapparaat komt als eigen station bij een PLC)
    .title = Geen PLC-module
    .hint = Haal het apparaat uit het rek; voeg een busapparaat vanuit zijn menu als station aan een PLC toe.

rule-ew-plc-018 = { $device } is getekend, maar geen van zijn kanalen is aan zijn pin gekoppeld: zijn adressen staan bij geen pin en een apparaat dat erop aangesloten is, is van geen kanaal het doel; koppel ze
    .title = Module getekend met ongekoppelde kanalen
    .hint = Koppel de kanalen met de oplossing aan hun pinnen; een module die nu getekend wordt, is meteen gekoppeld.

rule-ew-saf-001 = { $form ->
        [missing] { $module } is een F-module zonder F-doeladres; vul het in
        [range] { $module }: het F-doeladres { $address } ligt buiten { $min }..{ $max }, het bereik van zijn apparaatbeschrijving; vul een adres binnen dat bereik in
       *[duplicate] het F-doeladres { $address } wordt gebruikt door { $modules }; geef elke F-module een eigen adres
    }
    .title = F-adres
    .hint = Geef elke F-module een eigen F-doeladres binnen het bereik van zijn apparaatbeschrijving.

rule-ew-saf-002 = { $form ->
        [incomplete] { $channel } is een 1oo2-kanaal zonder partner die het terugnoemt; koppel het aan het andere kanaal van de sensor
       *[devices] het 1oo2-paar { $channel } en { $other } leest twee apparaten, { $first } en { $second }; verbind beide kanalen met draden naar dezelfde tweekanaalssensor
    }
    .title = 1oo2-paar
    .hint = Koppel elk 1oo2-kanaal aan het andere kanaal van dezelfde tweekanaalssensor.

rule-ew-saf-003 = { $channel }: de sensor { $sensor } wordt niet gevoed uit de testpulspen van de module; verbind zijn voeding met een draad op die pen
    .title = Testpuls
    .hint = Voed de sensor van een F-DI-kanaal uit de testpulspen van de module.

rule-ew-prc-001 = { $form ->
        [scheme] { $device }: het proceskenmerk { $tag } past niet in het schema { $scheme } van het project; corrigeer het kenmerk
       *[duplicate] het proceskenmerk { $tag } staat op { $devices }; houd het op het ene veldapparaat en volg het vanaf de andere
    }
    .title = Schema van proceskenmerken
    .hint = Geef elk veldapparaat een eigen proceskenmerk in het schema van het project.

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
        [cable] { $port } is verbonden met { $cable }, dat geen kabel voor zijn bus is; gebruik een buskabel
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
        }, maar het IO-Link-apparaat { $device } is aangesloten; zet de poort op IO-Link
       *[class] { $channel } is een poort van klasse { $port_class }, maar { $device } heeft klasse { $device_class } nodig; sluit het aan op een poort van klasse B
    }
    .title = Klasse van de IO-Link-poort
    .hint = Sluit elk IO-Link-apparaat aan op een poort van zijn klasse, in IO-Link-modus.

rule-ew-iol-002 = { $channel }: { $form ->
        [window] de { $have } { $dir ->
            [input] ingangs
           *[output] uitgangs
        }bits van het apparaat zijn meer dan het poortvenster van { $room } bits; gebruik een master met een groter venster
       *[cable] de kabel { $cable } is { $metres } m lang, meer dan de { $max } m die een IO-Link-poort toestaat; maak hem korter
    }
    .title = IO-Link-grenzen
    .hint = Houd elk IO-Link-apparaat binnen het venster en de kabellengte van zijn poort.

rule-ew-pot-001 = { $form ->
        [flow] { $device } voert { $from } door naar { $to }: een contact of een zekering geeft één potentiaal door en kan die niet in een andere veranderen. Verbeter het type aan één kant, of zet er een voeding of een transformator tussen.
       *[attrs] { $conflicts }{ $more ->
            [none] {""}
           *[other] {" "}Nog { $more } aansluitpunten zijn op dezelfde manier bedraad.
        }
    }
    .title = Type- of spanningsconflict
    .hint = Haal de draad weg die twee verschillende potentialen verbindt, of verbeter de definitie van een van beide. Een 0 V die op een andere manier dan met een draad recht naar een aardpunt met aarde verbonden is, heeft een vastgelegde verbinding nodig (Tekenen ▸ Verbinden ▸ Potentiaalverbinding).

pot-conflict-type = { $p0 } ({ $p1 }) is verbonden met { $p2 } ({ $p3 }){ $p4 }: een draad verbindt twee verschillende potentialen. Is een van beide verkeerd getypeerd, verbeter dan het type; anders zit de draad op de verkeerde plaats: verplaats hem of haal hem weg.
pot-conflict-voltage = { $p0 } ({ DECIMAL($p1) }) is verbonden met { $p2 } ({ DECIMAL($p3) }){ $p4 }: één geleider kan geen twee spanningen voeren. Is een spanning verkeerd ingevuld, verbeter die dan; anders zit de draad op de verkeerde plaats: verplaats hem of haal hem weg.
pot-conflict-environment = { $p0 } ({ $p1 }) is verbonden met { $p2 } ({ $p3 }){ $p4 }: één geleider kan niet allebei zijn. Is een instelling verkeerd, verbeter die dan; anders zit de draad op de verkeerde plaats: verplaats hem of haal hem weg.
pot-conflict-class = { $p0 } ({ $p1 }) is verbonden met { $p2 } ({ $p3 }){ $p4 }: één geleider kan niet in twee stroomkringklassen zitten. Is een klasse verkeerd, verbeter die dan; anders zit de draad op de verkeerde plaats: verplaats hem of haal hem weg.
pot-conflict-role = { $p0 } ({ $p1 }) is verbonden met { $p2 } ({ $p3 }){ $p4 }: een draad verbindt twee verschillende potentialen. Zit de pen op de verkeerde geleider, verplaats dan zijn draad; is { $p2 } verkeerd getypeerd, verbeter dan het type.
pot-conflict-roles = { $p0 } ({ $p1 }) is verbonden met { $p2 } ({ $p3 }){ $p4 }: een draad verbindt twee verschillende potentialen, dus een van beide pennen zit op de verkeerde geleider. Verplaats die draad naar de juiste, of haal hem weg.
pot-conflict-earth = { $p0 } (0 V) is verbonden met de beschermingsaarde { $p2 }{ $p4 }, maar niet met een draad die van de 0 V recht naar een aardpunt loopt. Selecteer de draad die ze verbindt en kies Tekenen ▸ Verbinden ▸ Potentiaalverbinding, of haal die draad weg.
pot-conflict-on-page = {" "}op pagina { $p0 }

rule-ew-pot-002 = { $form ->
        [selv] { $potential } is een SELV-stroomkring, maar de 0 V ervan is met de beschermingsaarde verbonden{ $bonds ->
            [one] {""}
           *[some] {" "}op { $count } plaatsen
        }. Een SELV-stroomkring wordt nooit geaard: haal de verbinding weg, of maak de stroomkring PELV.
       *[pelv] { $potential } is een PELV-stroomkring{ $bonds ->
            [none] , maar de 0 V ervan is niet met de beschermingsaarde verbonden. Teken één draad van de 0 V naar een symbool beschermingsaarde (PE) of naar de PE-rail; is de stroomkring niet bedoeld om geaard te zijn, maak hem dan SELV.
            [functional] , maar de 0 V ervan gaat alleen naar functionele aarde (FE), en dat is geen beschermende verbinding. Teken de draad van de 0 V in plaats daarvan naar een symbool beschermingsaarde (PE) of naar de PE-rail.
            [joined] , maar de 0 V ervan is zonder vastgelegde verbinding met de beschermingsaarde verbonden. Selecteer de draad naar aarde en kies Tekenen ▸ Verbinden ▸ Potentiaalverbinding.
           *[some] {" "}waarvan de 0 V op { $count } plaatsen met de beschermingsaarde verbonden is; een PELV-stroomkring wordt op één punt geaard. Houd één verbinding met aarde en haal de andere weg.
        }
    }
    .title = 0 V–PE-verbinding bij PELV of SELV
    .hint = Verbind de 0 V van een PELV-stroomkring op precies één punt met de beschermingsaarde (een draad van de 0 V naar een aardpunt), en de 0 V van een SELV-stroomkring nergens.

rule-ew-pot-003 = { $cp_shared ->
        [yes] { $cp_tag } pen { $cp_pin } ({ $cp_nth }e)
       *[no] { $cp }
    } is een pen voor functionele aarde (FE) en is aangesloten op de beschermingsaarde { $potential }. Sluit hem aan op functionele aarde, of, waar hij geaard moet zijn, selecteer zijn draad en kies Tekenen ▸ Verbinden ▸ Potentiaalverbinding (FE–PE).
    .title = FE-pen op PE
    .hint = Sluit de pen aan op functionele aarde, of leg de FE–PE-verbinding vast op zijn draad.

rule-ew-mfg-004 = De merker van { $wire } op { $at } past niet op zijn merkerartikel: { $text } heeft { $form ->
        [chars] { $needs } tekens nodig op een regel van { $limit }
       *[lines] { $needs } regels nodig waar het er { $limit } heeft
    }
    .title = Merkertekst past niet
    .hint = Kies een merkerartikel uit de voorraad met langere of meer regels, zet de merker van het einde met de hand, of laat de merker het draadnummer dragen.

rule-ew-mfg-008 = Het einde van { $wire } op { $at } kan niet worden gemaakt: { $reason ->
       *[fine_into_idc] een fijndradige draad in een snijklem
    }
    .title = Einde kan niet worden gemaakt
    .hint = Geef de draad een draadartikel van massieve of meerdradige draad, of leg hem op een klem die fijne aders neemt.

rule-ew-mfg-009 = Het met de hand gezette einde van { $wire } op { $at } past niet: { $form ->
        [size] een adereindhuls voor { $range } mm² op een draad van { $csa } mm²
        [twin] een twin-adereindhuls zonder tweede draad
       *[colour] een adereindhuls in de kleur { $colour } waar die uit de voorraad { $stock } is
    }
    .title = Met de hand gezet einde past niet
    .hint = Zet het einde terug op de regel, kies een artikel van de doorsnede van de draad, of geef ook de andere draad op de klem een twin-adereindhuls.

rule-ew-mfg-022 = Draad { $wire } wordt gesneden van { $part }, waarvan { $form ->
        [size] de doorsnede niet de { DECIMAL($csa) } mm² van de draad is
        [colour] de kleur niet het { $colour } van de draad is
       *[both] doorsnede en kleur niet de { DECIMAL($csa) } mm² { $colour } van de draad zijn
    }
    .title = Draadartikel past niet bij de draad
    .hint = Geef de draad een artikel van zijn doorsnede en kleur, of laat zijn artikel leeg zodat hij het eerste uit de werkplaatsvoorraad neemt; de lijsten drukken de eigen doorsnede en kleur van de draad af.

rule-ew-pot-004 = Draad { $wire } is groen-geel maar voert { $potential }{ $typed ->
        [yes] {" "}({ $type })
       *[no] {""}
    }: groen-geel is alleen voor de beschermingsaarde. Is de kleur met de hand op de draad gezet, haal die daar weg; geef de draad anders een andere kleur in zijn Eigenschappen.
    .title = GNYE op een potentiaal die geen PE is
    .hint = Gebruik groen-geel alleen voor beschermingsleiders; geef de draad een andere kleur.

rule-ew-pot-005 = Draad { $wire } is blauw maar voert { $potential }{ $type ->
        [none] {""}
       *[other] {" "}({ $type })
    }: in een project met een nulleider markeert blauw de nulleider. Is de kleur met de hand op de draad gezet, haal die daar weg; geef de draad anders een andere kleur, of gebruik een kabel waarvan de blauwe ader de nulleider voert.
    .title = BU op een geleider die geen N is
    .hint = Gebruik een andere kleur voor de draad, of een kabel waarvan de blauwe ader de nulleider voert.

rule-ew-pot-006 = PE-draad { $wire } heeft { DECIMAL($pe) } mm², minder dan de { DECIMAL($required) } mm² die nodig is voor de lijngeleiders van { DECIMAL($line) } mm² bij { $at }{ $verified ->
        [yes] {""}
       *[no] {" "}(de dimensioneringstabel is niet getoetst aan de norm)
    }. Maak hem minstens { DECIMAL($required) } mm² in zijn Eigenschappen.
    .title = PE te klein
    .hint = Vergroot de doorsnede van de beschermingsleider tot de vereiste maat.

rule-ew-pot-008 = { $form ->
        [terminal] { $terminal } is met de rail verbonden maar heeft geen locatie, en het project heeft { $locations } eigen locaties; de klem sluit op geen PE-knooppunt van een locatie aan
       *[island] De beschermingsaarde bij { $at } op pagina { $page } bereikt de inkomende PE niet en ook geen aardsymbool{ $location ->
            [main] {""}
           *[node] {" "}van { $node }
        }. Verbind hem met de PE-rail of met een aardsymbool, of zet zijn PE-klem op een geaarde rail.
    }
    .title = PE-eiland
    .hint = Elke beschermingsaarde moet de inkomende PE bereiken: verbind elke PE-geleider met de PE-rail of met een aardsymbool (een aardsymbool staat voor de aarde van de kast), of zet zijn PE-klemmen op een geaarde rail.

rule-ew-pot-009 = { $cp_shared ->
        [yes] { $cp_tag } pen { $cp_pin } ({ $cp_nth }e)
       *[no] { $cp }
    } heeft geen draad{ $unused ->
        [yes] {" "}(gemarkeerd als niet gebruikt)
       *[no] { $near ->
            [yes] , en de markering { $mark } staat er vlak naast zonder ermee te verbinden. Verbind de twee met de snelle oplossing, of teken er een draad naartoe.
           *[no] . Teken er een draad naartoe, of klik er met rechts op en kies Markeren als niet gebruikt.
        }
    }
    .title = Niet-aangesloten aansluitpunt
    .hint = Open de melding: de pagina opent met het apparaat geselecteerd. Teken een draad naar het aansluitpunt, of klik er met rechts op en kies Markeren als niet gebruikt als het bewust vrij blijft.

rule-ew-pot-010 = { $form ->
        [size] { $cp } neemt { $range } mm²; { $wire } van { $csa } mm² komt erop
       *[count] { $cp } neemt { $max ->
            [one] 1 geleider
           *[other] { $max } geleiders
        }; er komen er { $count } op
    }
    .title = Klemcapaciteit overschreden
    .hint = Leg een draad op een andere klem, geef twee draden van één doorsnede een twin-adereindhuls waar de klem die neemt, of leg een draad van een andere doorsnede op een klem die ervoor gemaakt is.

rule-ew-pot-011 = { $potential }: het is niet duidelijk welke draad { $place ->
        [junction] de aftakking { $at ->
            [cell] in { $cell } op folio { $page }
           *[folio] op folio { $page }
        } voedt (draden { $wires })
        [ip] de onderbrekingspunten { $group } voedt (draden { $wires })
       *[other] een aftakking of een groep onderbrekingspunten voedt
    }; teken één draad ernaartoe vanaf de voedingszijde
    .title = Voeding van aftakking onduidelijk
    .hint = Teken de aftakking zo dat één draad haar voedt, of stel de bedradingsvolgorde in.

rule-ew-pot-012 = { $form ->
        [ring] De draad { $wire } sluit een ring: zijn draden lopen terug naar waar ze begonnen, dus hij staat niet in de draadlijst. Haal één draad van de ring weg, of bevestig dat de ring bedoeld is.
       *[other] Een draad sluit een ring in de tekening en staat niet in de draadlijst. Haal één draad van de ring weg, of bevestig dat de ring bedoeld is.
    }
    .title = Draden in een ring
    .hint = Verwijder de draad die de ring sluit, of bevestig dat de ring bedoeld is.

rule-ew-pot-013 = { $device } { $form ->
        [isolating] is een N-scheidingsklem op { $from } → { $to }: goed zo, de nulleider wordt alleen geopend om te meten.
       *[switching] opent de nulleider { $from } → { $to } zonder de fase{ $loads ->
            [one] {" "}(belasting { $load } blijft verbonden met { $line })
            [many] {" "}(deze belastingen blijven verbonden: { $stranded })
           *[none] {""}
        }: wat erachter zit blijft onder spanning terwijl het uitgeschakeld lijkt. Zet het contact in de fase, of gebruik een apparaat dat fase en nulleider samen opent, zoals een tweepolige automaat.
    }
    .title = Schakelen in de nulleider
    .hint = Verplaats het contact naar de lijngeleider, of gebruik een apparaat dat L en N tegelijk opent.

rule-ew-pot-014 = { $form ->
        [conductor] Een deel van draad { $wire } is op een andere kabelader getekend; de draad houdt de ader waarop hij begint. Teken de hele draad op één ader.
        [override] Een deel van draad { $wire } zet een waarde die de draad al van een ander deel heeft; de eerste waarde geldt. Zet hem op één plaats.
       *[other] Een deel van een fysieke draad botst met een ander deel ervan. Geef de draad één set eigenschappen.
    }
    .title = Conflict in een fysieke draad
    .hint = Geef de fysieke draad één set eigenschappen, of splits hem in afzonderlijke draden.

rule-ew-pot-015 = Draadnummer { $number } staat op { $count } draden die niet één geleider zijn
    .title = Draadnummerconflict
    .hint = Geef elke draad een eigen nummer, of verbind de draden als ze één geleider zijn.

rule-ew-pot-016 = Draad { $wire } heeft { $member ->
        [csa] zijn doorsnede { DECIMAL($value) }
        [colour] zijn kleur { $value }
        [wire_part] zijn draadartikel
        [insulation] zijn isolatie
        [function] zijn draadfunctie
        [shielded] afgeschermd
       *[twisted] getwist
    } met de hand ingesteld, wat hij toch al krijgt. Haal die instelling weg.
    .title = Overbodige overschrijving
    .hint = Verwijder de overschrijving; ze zet de waarde die de draad toch al krijgt.

rule-ew-pot-017 = { $first } heeft meer dan één naam: { $names } zijn met elkaar verbonden. Geef ze één naam, of haal de draad weg die ze verbindt.
    .title = Twee namen voor één potentiaal
    .hint = Houd één definitie van de potentiaal, of geef de onderbrekingspunten één naam.

rule-ew-pot-021 = { $cp_shared ->
        [yes] { $cp_tag } pen { $cp_pin } ({ $cp_nth }e)
       *[no] { $cp }
    } ligt op { $potential }, dat geen voeding bereikt: de belasting wordt gevoed maar niet teruggevoerd; sluit het aan op de retourgeleider (0 V, N)
    .title = Belasting niet teruggevoerd
    .hint = Verbind de retourzijde van de belasting met de retourgeleider van de voeding (0 V, N), of met de retourpin van de module.

rule-ew-pot-022 = Draad { $wire } heeft zijn doorsnede { DECIMAL($size) } met de hand ingesteld, kleiner dan de { DECIMAL($needed) } die de nominale stroom van { DECIMAL($current) } A van { $device } volgens de stroomtabel vraagt (IEC 60204-1 tabel 6, methode B2). Neem de voorgestelde doorsnede.
    .title = Doorsnede kleiner dan de beveiliging vraagt
    .hint = Neem de voorgestelde doorsnede of een grotere, of beveilig de draad met een apparaat van een lagere nominale stroom.

rule-ew-rds-001 = { $device } is geen geldige IEC 81346-aanduiding: { $reason ->
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

rule-ew-rds-003 = { $device }: de lettercode { $letter } past niet bij de functiecategorie { $category }; de identificatieset { $set } stelt { $suggested } voor{ $verified ->
        [yes] {""}
       *[no] {" "}(de set is niet getoetst aan de norm)
    }
    .title = Lettercode tegen functie
    .hint = Gebruik de voorgestelde lettercode, of behoud het kenmerk als de classificatie bedoeld is.

rule-ew-rds-004 = `{ $tag }` wordt afgedrukt door { $count } apparaten
    .title = Dubbel kenmerk
    .hint = Geef een van de apparaten een ander kenmerk.

rule-ew-rds-007 = { $device }: de lettercode { $letter } gebruikt gereserveerde letters: { $io_form ->
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
    } { $text } van { $device } bevat niet-Latijnse tekens die op Latijnse lijken: { $chars }; zoeken en kruisverwijzingen vinden het niet; typ het opnieuw in Latijnse letters
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
           *[other] { $role }
        }); het vierogenprincipe vraagt een andere partij
        [sign-off] goedkeuring ontbreekt; de controleur keurt de opgeslagen revisie goed
       *[stale] de goedkeuring past niet bij de opgeslagen inhoud (gewijzigd na goedkeuring); keur haar opnieuw goed
    }
    .title = Vier-ogenprincipe
    .hint = Stel de auteur in en laat daarna een ander de opgeslagen revisie goedkeuren; keur opnieuw goed na een inhoudelijke wijziging.

rule-ew-rel-004 = revisie { $revision }: { $form ->
        [format] het label volgt niet de notatie van revisielabels van het project, { $pattern }
       *[order] het label van de open revisie komt niet na { $later }; revisielabels lopen op, de open revisie als laatste
    }
    .title = Volgorde van de revisielabels
    .hint = Geef de revisie een label in het labelformaat van het project dat na elk eerder label komt.

rule-ew-term-001 = een klem van { $strip } heeft geen aanduiding
    .title = Klem zonder aanduiding
    .hint = Geef de klem een aanduiding.

rule-ew-term-002 = { $form ->
        [taken] aanduiding { $designation } is al bezet op { $end ->
            [untagged] de klemmenstrook
           *[named] { $strip }
        } ({ $count } klemmen){ $remedy ->
            [free] ; gebruik { $free }
            [swap] ; verwissel { $old } ↔ { $designation } in één transactie (terminal.designate)
            [both] ; gebruik { $free } of verwissel { $old } ↔ { $designation } in één transactie (terminal.designate)
           *[none] {""}
        }
       *[used] aanduiding { $designation } wordt gebruikt door { $count } klemmen van { $end ->
            [untagged] de klemmenstrook
           *[named] { $strip }
        }
    }
    .title = Dubbele aanduiding
    .hint = Hernummer de klemmenstrook, of geef een van de klemmen een andere aanduiding.

rule-ew-term-003 = reserveklem { $terminal } is aan zijn veldzijde bedraad; bedraad hem aan de andere zijde of hef de reservestatus op
    .title = Reserveklem aan de veldzijde bedraad
    .hint = Verwijder de draad aan de veldzijde, of maak de klem een gebruikte klem.

rule-ew-term-004 = bruggroep { $group } van { $strip } is ongeldig: { $gap ->
        [yes] { $sep_gap }zijn klemmen liggen niet naast elkaar
       *[no] {""}
    }{ $shafts ->
        [yes] { $sep_shafts }hij loopt over { $shaft_count } brugschachten
       *[no] {""}
    }{ $reaches ->
        [yes] { $sep_reaches }hij reikt tot { $reached }
       *[no] {""}
    }{ $overlaps ->
        [yes] { $sep_overlaps }hij overlapt bruggroep { $overlapped } op zijn brugschacht
       *[no] {""}
    }; een bruggroep loopt over naast elkaar liggende klemmen van één klemmenstrook op één brugschacht
    .title = Ongeldige bruggroep
    .hint = Brug alleen naburige klemmen van één klemmenstrook, of zet de klemmen naast elkaar.

rule-ew-term-005 = { $terminal } is met de rail verbonden maar heeft geen PE-aansluitpunt; de klem sluit op geen PE-knooppunt van een locatie aan
    .title = Railverbinding op een klem die geen PE is
    .hint = Wis de railverbinding, of maak de klem een PE-klem.

rule-ew-term-006 = { $terminal } ligt op draad { $wire } zonder die te verbinden; sluit de klem op de draad aan of verplaats haar
    .title = Klem op een draad die zij niet verbindt
    .hint = Neem de klem op in de draad, of haal haar van de draad af.

## botsingen na samenvoegen

rule-ew-core-013 = { $collision } na samenvoegen; { $keeper ->
        [frozen] het record met het vastgezette kenmerk
       *[oldest] het oudste record
    } behoudt het, en { $others ->
        [many] de andere moeten
       *[one] het andere moet
    } wijzigen
    .title = Botsing na samenvoegen
    .hint = Hernummer het nieuwere apparaat met de snelle oplossing, of geef een van de records een ander nummer.

## EW-IMP: de importbevindingen van het migratierapport

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

rule-ew-imp-010 = { $record } is in { $file } verwijderd; hier blijft het behouden. Verwijder het als het niet meer nodig is.
    .title = In de bron verwijderd, hier behouden
    .hint = Verwijder het record als het niet meer nodig is; behoud het anders.

rule-ew-imp-011 = { $record } is zowel in { $file } als hier gewijzigd; de versie van dit project blijft behouden.
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
    .title = Namen van een folioverwijzingspaar verschillen
    .hint = Hernoem een kant van het paar, of zet de koppeling op ketenmodus.

rule-ew-imp-116 =
    .title = Slave zonder master, of master boven zijn slaves
    .hint = Koppel de slave aan zijn master, of verminder de koppelingen van de master.

rule-ew-imp-117 =
    .title = Standaardformule voor de folio-index
    .hint = Laat de folioverwijzingen het foliolabel gebruiken.

rule-ew-imp-118 =
    .title = Bedrading van een aftakking aangenomen
    .hint = Zet de bedrading van de aftakking op Keten waar de bron het zo bedraadde.

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
    .hint = Voeg de varianten samen tot één structuurcode en behoud de andere als aliassen.

rule-ew-imp-105 =
    .title = Omschrijving per folio als overschrijving behouden
    .hint = Laat de overschrijving vallen om de projectomschrijving weer af te drukken, of behoud haar.

rule-ew-imp-114 =
    .title = Geïmporteerde tekststijlen niet aan slotsoorten gebonden
    .hint = Pas de tekststijl toe die de slotsoort noemt.

# De labels van de snelle oplossingen, per sleutel (`QuickFix::key`).
fix-take-proposed-size = De voorgestelde doorsnede nemen
fix-drop-override = overschrijving verwijderen
fix-variant-own = Het eigen nummer van het artikel gebruiken
fix-next-free-address = Het volgende vrije adres geven, { $p0 }
fix-anchor-graphics-centre = anker op het midden van de grafische elementen
fix-renumber-newer = De nieuwere hernummeren
fix-unfreeze-tag = Kenmerk losmaken
fix-realise-primary-part = Het primaire artikel opnieuw plaatsen
fix-rebind-map-channel = Koppelen aan het kanaal van de kaart
fix-declare-pelv-bond = 0 V–PE-verbinding (PELV) vastleggen
fix-mark-cp-unused = Markeren als niet gebruikt
fix-reset-end = Terugzetten op de regel
fix-join-mark = Pen en markering verbinden
fix-plc-take-out = Uit het rek halen
fix-plc-bind-pins = De kanalen aan hun pinnen koppelen
fix-blank-position = Blindplaat op ventielplaats { $p0 }
fix-draw-valves = De ventielen in het ventieleiland tekenen
fix-plug-open-ports = Blindstoppen op de open poorten zetten
fix-text-style-minimum = Op 1,2 mm zetten

# De woorden die een regel afdrukt waar een record geen eigen naam heeft, per sleutel; een regel geeft ze
# bij hun sleutel aan zijn patroon door.
word-cable = kabel
word-without-a-tag = zonder kenmerk
word-assembly = samenstelling
word-a-location = een locatie
word-a-device-without-designation = een apparaat zonder aanduiding
word-a-missing-part = een ontbrekend artikel
word-an-unknown-profile = een onbekend profiel
word-a-connector = een connector
# A scope of EW-CORE-083 by its name.
word-scope-folio = folio { $p0 }
word-scope-potential = potentiaal { $p0 }
word-scope-configuration-project = configuratieproject { $p0 }
word-scope-logical-net = logisch net { $p0 }
word-scope-physical-net = fysiek net { $p0 }
word-scope-of-kind = een bereik van soort { $p0 }
# A load a contact leaves connected (EW-POT-013), and a character that looks like a Latin one (EW-RDS-009).
word-stranded-load = { $p0 } → { $p1 }
word-lookalike = { $p0 } → { $p1 }
word-none = geen
word-terminal = klem
word-strip = klemmenstrook
word-another-strip = een andere klemmenstrook
word-valve = ventiel
word-the-valve = het ventiel
word-a-module = een module
word-a-role-of-a-newer-format = een rol van een nieuwer formaat
word-a-drawn-page = een getekende pagina
word-a-section-divider = een sectiescheiding
word-a-report-page = een rapportpagina
word-the-strip = de klemmenstrook
word-a-device-that-is-not-a-terminal = een apparaat dat geen klem is
word-the-project = het project
word-a-logical-bus-net = een logisch busnet
word-a-physical-bus-net = een fysiek busnet
word-a-connection-point-of-another-page = een aansluitpunt van een andere pagina
word-its-target = zijn doel
word-the-station = het station
word-a-station = een station
word-an-f-module = een F-module
word-the-sensor = de sensor
word-the-cable = de kabel
word-the-device = het apparaat
word-current-newer = een stroomsoort van een nieuwer formaat
word-earthing-floating = zwevend
word-earthing-newer = een aarding van een nieuwer formaat

rule-ew-imp-251 =
    .title = Tekening zonder eenheid, in de gekozen eenheid gelezen
    .hint = Controleer de afmetingen van de tekening; importeer haar opnieuw in de juiste eenheid als die niet klopt.

rule-ew-imp-252 =
    .title = SHX-lettertype in een meegeleverd lettertype getekend
    .hint = Controleer de teksten: hun breedte kan afwijken van die in het SHX-lettertype.

rule-ew-imp-253 =
    .title = Tekeningelement niet geïmporteerd
    .hint = Teken wat het element liet zien met de hand, of importeer een 2D-export van de tekening.

rule-ew-imp-254 =
    .title = Extern bestand niet geopend
    .hint = Plaats het bestand waarnaar de tekening verwijst zelf als de pagina het nodig heeft.

rule-ew-imp-255 =
    .title = Arceerpatroon tot zijn omtrek teruggebracht
    .hint = Teken de vulling van de arcering met de hand als de pagina die nodig heeft.

rule-ew-imp-257 =
    .title = DWG omgezet door de ODA File Converter
    .hint = Vergelijk de pagina met de DWG: de omzetting is die van de converter.
rule-ew-plc-016 = configuratieproject { $cfg } is gewijzigd sinds de { $format ->
        [aml] AML
        [tia_xlsx] kenmerkentabel
        [plcopen] PLCopen
       *[other] PLC
    }-{ $direction ->
        [import] import
       *[export] export
    }{ $date ->
        [none] {""}
       *[other] {" "}van { $date }
    }: { $count } object(en) wijken af ({ $objects }{ $more ->
        [none] {""}
       *[other] {" "}en nog { $more }
    }); exporteer het opnieuw, of importeer het bestand van de PLC
    .title = Uitwisseling gewijzigd
    .hint = Exporteer het configuratieproject opnieuw, of importeer het bestand van de PLC-software.

rule-ew-prc-002 = { $form ->
        [device] instrumentkenmerk { $tag } heeft geen apparaat meer; pas de instrumentenlijst opnieuw toe, of haal het kenmerk eruit
       *[channel] instrumentkenmerk { $tag } ({ $signal }) heeft geen kanaal: er was geen reserve-{ $signal }-kanaal vrij; voeg een module toe of maak een kanaal vrij en pas de instrumentenlijst dan opnieuw toe
    }
    .title = Open instrumentkenmerk
    .hint = Voeg een module toe of maak een kanaal van de soort van het kenmerk vrij, en pas de instrumentenlijst dan opnieuw toe.

rule-ew-imp-206 =
    .title = Rapportkolom niet gekoppeld
    .hint = Koppel de kolom in de kolomkoppeling van het rapport en importeer het rapport opnieuw, of laat haar bewust weg.

rule-ew-imp-207 =
    .title = Klemmen met meerdere niveaus samengevoegd
    .hint = Controleer de niveaus van de klem aan de hand van het rapport.

## Words another part of the program wrote: the store's integrity findings, the text an importer stored, an importer's quick fix

words-as-written = { $p0 }
# A finding's subject in words (what the message is about), rendered where the message is.
subject-waiver = vrijstelling { $p0 }
subject-ip = onderbrekingspunt { $p0 }
subject-tbd = TBD { $p0 }
subject-wire = draad { $p0 }
subject-toc = inhoudsopgave
subject-folio-style = folio { $p0 }: { $p1 }
subject-pair = { $p0 } { $p1 }
subject-revision = revisie { $p0 }
subject-cp = { $p0 }:{ $p1 }
subject-bridge-group = { $p0 } bruggroep { $p1 }
subject-rail-at = de rail bij { $p0 }
subject-duct-at = de kabelgoot bij { $p0 }
subject-carrier-at = de drager bij { $p0 }
subject-conflict-target = { $target ->
        [project] het project
        [device] apparaat { $tag }
        [page] folio { $page }
        [function] een functie van apparaat { $tag }
        [connection-on] verbinding van { $from } naar { $to } op folio { $page }
        [connection] verbinding van { $from } naar { $to }
       *[other] { $kind }
    }
subject-conflict-file = { $file ->
        [page] het bestand van folio { $name }
        [layout] het layoutbestand van { $name }
        [a-layout] een layoutbestand
        [a-page] een paginabestand
        [import] een importbestand
       *[path] { $name }
    }

## The words that join the items of a list in a finding

join-comma = {", "}
join-and = {" en "}
join-wire = –
join-connection = {" → "}
join-sentence = {" "}
join-semicolon = {"; "}
# Between two reserved letters I and O (EW-RDS-007): the Dutch names them together.
join-reserved-letters = {", "}
