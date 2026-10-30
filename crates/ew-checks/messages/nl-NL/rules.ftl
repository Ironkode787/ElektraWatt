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

rule-ew-num-001 = { $frozen }: bevroren, wijkt af van het schema (afgeleide waarde { $derived })
    .title = Tag wijkt af van het schema
    .hint = Ontdooi de tag om de afgeleide waarde over te nemen, of laat hem bevroren als de afwijking bedoeld is.

rule-ew-plc-001 = { $form ->
        [alignment] { $channel }: het adres { $address } begint niet op een veelvoud van { $align } bytes, de woorduitlijning van het profiel (een niet-geverifieerde conventie)
       *[overlap] { $channel } ({ $address }) overlapt { $other } ({ $other_address }) in configuratieproject { $cfg }; geef een van beide een ander adres
    }
    .title = Adresoverlap
    .hint = Geef een van de overlappende kanalen een ander adres, of laat beide een automatisch adres nemen.

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

rule-ew-pot-001 = { $conflicts }{ $more ->
        [none] {""}
       *[other] ; en nog { $more } aansluitpunten
    }{ $hint ->
        [bond] ; declareer de verbinding met Draw ▸ Bond
       *[none] {""}
    }
    .title = Type- of spanningsconflict
    .hint = Scheid de potentialen, corrigeer de definitie of declareer de verbinding die ze samenbrengt.

rule-ew-pot-003 = { $cp } heeft de rol FE en is aangesloten op de PE-potentiaal { $potential }; declareer een FE–PE-verbinding of sluit het aan op functionele aarde
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

rule-ew-pot-009 = { $cp } is niet aangesloten{ $unused ->
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

rule-ew-pot-021 = { $cp } ligt op { $potential }, dat geen voeding bereikt: de belasting wordt gevoed maar niet teruggevoerd; sluit het aan op de retourgeleider (0 V, N)
    .title = Belasting niet teruggevoerd
    .hint = Verbind de retourzijde van de belasting met de retourgeleider van de voeding (0 V, N), of met de retourpin van de module.

rule-ew-rds-003 = { $device }: de lettercode { $letter } past niet bij de functiecategorie { $category }; de identificatieset { $set } stelt { $suggested } voor{ $verified ->
        [yes] {""}
       *[no] {" "}(de set is niet getoetst aan de norm)
    }
    .title = Lettercode tegen functie
    .hint = Gebruik de voorgestelde lettercode, of behoud de tag als de classificatie bedoeld is.
