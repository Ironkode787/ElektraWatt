# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: GPL-3.0-or-later

# The rule messages in nl-NL (doc 15 §3.8; WP-S1.16 D6). A rule or attribute missing here falls back to en-GB; until
# 1.0 the message test lists the missing ones as warnings. Shipped first for the rules that report on Golden A
# (doc 20 §18.1); the other texts follow (the de-scope order of WP-S1.16 §13.1).

rule-ew-core-033 = { $cable } heeft geen gerouteerde, handmatige of artikellengte
    .title = Kabel zonder lengte
    .hint = Voer een handmatige lengte in, routeer de kabel in de lay-out of wijs een artikel met een vaste lengte toe.

rule-ew-core-034 = de externe verbinding { $connection } heeft geen kabel: { $count ->
        [one] draad { $wires } is
       *[many] draden { $wires } zijn
    } geen kabelader; voeg de kabel toe en wijs de aders toe
    .title = Externe verbinding zonder kabel
    .hint = Voeg de kabel toe die de locatie verlaat en wijs de aders toe aan de draden.

rule-ew-core-041 = { $device } heeft geen artikel; wijs er een toe of markeer het als geleverd door derden
    .title = Apparaat zonder artikel
    .hint = Wijs een artikel uit de bibliotheek toe of markeer het apparaat als geleverd door derden.

rule-ew-core-043 = plaatsvervangend artikel "{ $part }" (artikelnummer TBD) wordt gebruikt door { $users }; kies het artikel vóór de vrijgave
    .title = Plaatsvervangend artikel
    .hint = Vervang het plaatsvervangende artikel vóór de vrijgave door het echte artikel.

rule-ew-core-061 = tekststijl { $style } drukt af met een hoofdletterhoogte van { $height } mm, onder het minimum van { $minimum } mm
    .title = Tekst onder de minimale hoogte
    .hint = Vergroot de tekststijl of geef de teksten een stijl die minstens op de minimale hoogte afdrukt.

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
        [duct] { $device } overlapt de goot bij { $duct }; verplaats het apparaat of de goot
       *[rail] de rail bij { $rail } overlapt de goot bij { $duct }; verplaats de rail of de goot
    }
    .title = Botsing
    .hint = Verplaats een van de overlappende objecten tot hun voetafdrukken niet meer overlappen.

rule-ew-lay-010 = { $form ->
        [on_duct] { $device } is geplaatst op de goot bij { $duct }, die geen apparaten draagt; plaats het op een rail
        [off_rail] { $device } is geplaatst op { $s } mm op de rail bij { $rail }, buiten 0.0 tot { $length } mm; verplaats het op de rail
        [rail_turn] { $device } is op de rail bij { $rail } gedraaid over een andere hoek dan 0° of 180°; draai het terug
        [off_axis] { $device } is naast de as van de rail bij { $rail } geplaatst; verplaats het op de rail
        [profile] { $device } (artikel { $part }) past niet op de rail bij { $rail } ({ $profile }); plaats het op een rail waar het op past
        [surface_turn] { $device } is op de montageplaat gedraaid over een andere hoek dan een veelvoud van 90°; draai het terug
        [off_surface] { $device } ligt niet in het vlak van de montageplaat; plaats het op de plaat
        [surface] { $device } (artikel { $part }) wordt niet op de plaat zelf gemonteerd; plaats het op een rail
        [overflow] { $device } beslaat { $from } tot { $to } mm op de rail bij { $rail }, voorbij 0.0 tot { $length } mm; maak het korter of verplaats het
        [outside] { $device } ligt buiten het bruikbare vlak van de montageplaat; verplaats het naar binnen
        [path] het pad van de { $carrier ->
            [rail] rail bij { $at } is geen twee punten op één horizontale of verticale lijn
            [duct] goot bij { $at } is geen lijn van horizontale en verticale stukken
           *[other] drager bij { $at } is geen lijn van horizontale en verticale stukken
        }; pas het pad aan
        [carrier_outside] de { $carrier ->
            [rail] rail
            [duct] goot
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

rule-ew-num-001 = { $frozen }: bevroren, wijkt af van het schema (afgeleide waarde { $derived })
    .title = Tag wijkt af van het schema
    .hint = Ontdooi de tag om de afgeleide waarde over te nemen, of laat hem bevroren als de afwijking bedoeld is.

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
       *[other] er is geen adres voor af te leiden; controleer de start van de module en de PLC-sectie van het onderdeel
    }
    .title = Gebruikt kanaal zonder adres
    .hint = Geef het kanaal een adres, of markeer het als ongebruikt.

rule-ew-plc-012 = { $form ->
        [manual] { $channel }: het handmatige adres { $address } wijkt af van het afgeleide adres { $derived }; zet het adres op automatisch, of houd de afwijking bewust aan
       *[binding] { $channel } ({ $address }) stuurt spoel { $solenoid } van { $valve } aan, die de positiekaart van onderdeel { $part } toewijst aan { $expected } ({ $expected_address }); koppel het kanaal van de kaart, of corrigeer de PLC-sectie van het onderdeel
    }
    .title = Koppeling wijkt af van de onderdeelkaart
    .hint = Koppel het kanaal dat de kaart van het onderdeel toewijst, of corrigeer de PLC-sectie van het onderdeel.

rule-ew-plc-015 = { $subject }: { $what ->
        [input] de start van de ingangen
        [output] de start van de uitgangen
       *[address] het kanaaladres
    } is TBD; vul het in vóór de vrijgave
    .title = Tbd-adres
    .hint = Vul het adres in vóór de vrijgave.

rule-ew-bus-001 = { $form ->
        [duplicate] { $ports }: stationsadres { $address } komt meer dan eens voor in logisch net { $net }; geef elke poort een eigen adres
       *[tbd] { $port }: het stationsadres is TBD; vul het in vóór de vrijgave
    }
    .title = Busadres ontbreekt of dubbel
    .hint = Vul vóór de vrijgave een stationsadres in dat uniek is in het logische net.

rule-ew-bus-004 = { $port }: { $form ->
        [external] de afsluiting aan het externe einde is onbekend; controleer dat de partner het segment afsluit
        [missing] geen afsluitend element aan dit segmenteinde; sluit het segment hier af
        [end] de afsluiting staat uit aan het segmenteinde; zet haar aan
        [middle] de afsluiting staat aan binnen het segment; zet haar uit
       *[unknown] de afsluitstand is van een nieuwer formaat; controleer haar
    }
    .title = Afsluiting
    .hint = Zet de afsluiting aan op beide segmenteinden en uit overal daartussen.

rule-ew-pot-001 = { $conflicts }{ $more ->
        [none] {""}
       *[other] ; en nog { $more } aansluitpunten
    }{ $hint ->
        [bond] ; declareer de verbinding met Draw ▸ Bond
       *[none] {""}
    }
    .title = Type- of spanningsconflict
    .hint = Scheid de potentialen, corrigeer de definitie of declareer de verbinding die ze samenbrengt.

rule-ew-pot-003 = { $cp_shared ->
        [yes] { $cp_tag } pen { $cp_pin } ({ $cp_nth }e)
       *[no] { $cp }
    } heeft de rol FE en is aangesloten op de PE-potentiaal { $potential }; declareer een FE–PE-verbinding of sluit het aan op functionele aarde
    .title = FE-pen op PE
    .hint = Declareer een FE–PE-verbinding of sluit de pen aan op functionele aarde.

rule-ew-pot-005 = draad { $wire } is BU op de potentiaal { $potential }{ $type ->
        [none] {""}
       *[other] {" "}(type { $type })
    }; in een project met een nulleider markeert BU de nulleider
    .title = BU op een geleider die geen N is
    .hint = Gebruik een andere kleur voor de draad, of een kabel waarvan de blauwe ader de nulleider voert.

rule-ew-pot-006 = PE-draad { $wire } heeft { $pe } mm², minder dan de { $required } mm² die nodig is voor de lijngeleiders van { $line } mm² bij { $at }{ $verified ->
        [yes] {""}
       *[no] {" "}(de dimensioneringstabel is niet getoetst aan de norm)
    }
    .title = PE te klein
    .hint = Vergroot de doorsnede van de beschermingsleider tot de vereiste maat.

rule-ew-pot-009 = { $cp_shared ->
        [yes] { $cp_tag } pen { $cp_pin } ({ $cp_nth }e)
       *[no] { $cp }
    } is niet aangesloten{ $unused ->
        [yes] {" "}(gemarkeerd als ongebruikt)
       *[no] ; sluit het aan of markeer het als ongebruikt
    }
    .title = Niet-aangesloten aansluitpunt
    .hint = Sluit het aansluitpunt aan of markeer het als ongebruikt.

rule-ew-pot-011 = op { $potential } is niet duidelijk welke draad een knooppunt of een groep onderbrekingspunten voedt; teken één draad ernaartoe vanaf de voedingszijde
    .title = Voeding van knooppunt onduidelijk
    .hint = Teken het knooppunt zo dat één draad het voedt, of stel de bedradingsvolgorde in.

rule-ew-pot-013 = { $device } schakelt de nulleider { $from } → { $to }{ $loads ->
        [one] ; belasting { $load } blijft verbonden met { $line }
       *[none] {""}
    }.
    .title = Schakelen in de nulleider
    .hint = Verplaats het contact naar de lijngeleider, of gebruik een apparaat dat L en N tegelijk opent.

rule-ew-pot-021 = { $cp_shared ->
        [yes] { $cp_tag } pen { $cp_pin } ({ $cp_nth }e)
       *[no] { $cp }
    } ligt op { $potential }, dat geen voeding bereikt: de belasting wordt gevoed maar niet teruggevoerd; sluit het aan op de retourgeleider (0 V, N)
    .title = Belasting niet teruggevoerd
    .hint = Verbind de retourzijde van de belasting met de retourgeleider van de voeding (0 V, N), of met de retourpin van de module.

rule-ew-rds-003 = { $device }: de lettercode { $letter } past niet bij de functiecategorie { $category }; de identificatieset { $set } stelt { $suggested } voor{ $verified ->
        [yes] {""}
       *[no] {" "}(de set is niet getoetst aan de norm)
    }
    .title = Lettercode tegen functie
    .hint = Gebruik de voorgestelde lettercode, of behoud de tag als de classificatie bedoeld is.
