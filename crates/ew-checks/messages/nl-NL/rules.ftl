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
