# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: GPL-3.0-or-later

# The texts of the window in nl-NL: the page canvas: its tools and their hints, the HUD, the picker, previews, wires,
# boxes, pages and macros. Every key of the en-GB file of the same name, with the same arguments; a message missing
# here falls back to en-GB. Product and file names, rule IDs, command names, key caps, identifiers and units are not
# translated.

tool-place = Plaatsen
tool-exchange = Symbool wisselen
hint-place = Klik of { $p0 } om te plaatsen; { $p1 }, { $p2 }, { $p3 } en { $p4 } draaien het; { $p5 } stopt
hint-choose-strip = Kies de klemmenstrook in de lijst; Esc annuleert
hint-place-proposals = Klik of { $p0 } om te plaatsen en de voorgestelde draden te verbinden; Alt+klik of { $p1 } plaatst zonder; { $p2 } probeert het volgende doel; { $p3 } stopt
hint-select-move = Laat los, klik of { $p0 } om te verplaatsen; { $p1 } annuleert
hint-box-pins-room = Kleiner gaat de box niet: de aansluitingen en hun namen hebben de ruimte nodig, en een stap vrij bij elke hoek. Laat los, klik of { $p0 } om de maat te wijzigen; { $p1 } annuleert
hint-exchange = Klik of { $p0 } om het symbool te wisselen; { $p1 } annuleert
move-readout = dx { $p0 } dy { $p1 } mm
place-target-new = nieuw { $p0 }
place-target-join = bij { $p0 }
place-target-function = functie
place-target-part = artikel { $p0 }
place-target-variant = { $p0 } · { $p1 }
place-target-terminal = klem { $p1 } op { $p0 }
place-target-new-strip = de nieuwe klemmenstrook (nog geen kenmerk)
place-target-strip-new = { $p0 } (nieuw)
place-variant = Variant { $p0 } · { $p1 }
place-target-box = kader
picker-title = Symbool invoegen
picker-title-device = Apparaat invoegen
picker-head-symbol = Een symbool is een tekening zonder artikel. Gebruik Apparaat om een artikel met zijn functies te plaatsen.
picker-head-device = Een apparaat is een artikel uit de catalogus met zijn functies. Gebruik Symbool om zonder artikel te tekenen.
picker-title-exchange = Symbool wisselen
picker-filter-hint = Zoek op naam, artikelnummer en trefwoord
picker-no-picture = Nog geen symbool: kies er een om dit artikel te plaatsen
picker-badge-embedded = ingesloten
picker-badge-starter = starter
picker-badge-library = bibliotheek
picker-part-function = { $p0 } van { $p1 } { $p2 }
picker-main = hoofd
picker-variant = Variant
picker-comes-with = Komt met
picker-accessory = Toebehoren
picker-comes-with-line = { $p0 } { $p1 }, { $p2 } { $p3 }
picker-places = Plaatst { $p0 }
picker-drawn-as-box = Getekend als apparaatkast
picker-holds = { $p0 } functies
picker-empty = Niets voldoet aan het filter
picker-empty-parts = Nog geen artikelen in dit project
picker-empty-parts-filter = Geen artikel voldoet aan het filter. Om een geplaatst symbool zijn artikel te geven: selecteer het en kies Bewerken ▸ Artikel kiezen…
picker-part-symbol = Een symbool kiezen…
picker-title-part-symbol = Een symbool kiezen voor { $p0 }
picker-part-symbol-note = Dit artikel heeft geen eigen symbool: kies het symbool waarmee u het plaatst.
picker-empty-black-box = Geen black-boxsymbolen of artikelen om te plaatsen
picker-empty-page = Op deze pagina kan niets worden geplaatst
picker-group-project = In dit project
picker-group-library = Bibliotheek
# F28: de vakgebieden van de onderdelenbrowser.
picker-trade-electrical = Elektrisch
picker-trade-plc-bus = PLC en bus
picker-trade-pneumatics = Pneumatiek
picker-trade-enclosure = Kast en montage
picker-group-libraries = Uit bibliotheken
picker-cps = Aansluitpunten: { $p0 }
chooser-join-question = Bij apparaat voegen
chooser-new-device = Nieuw apparaat
chooser-kind-slot = positie { $p0 }
chooser-kind-auxiliary = hulp
chooser-kind-level = volgende etage
hud-keys-place-join = { $p0 } draait · { $p1 } variant · { $p2 } Bij apparaat voegen… · { $p3 } klaar
tool-strip-new = Nieuwe strook
picker-title-strip = Een klemmenstrook kiezen
picker-title-black-box = Black box invoegen
picker-strip-next = volgende klem { $p0 }
hint-select-wire = Sleep een ronde greep om een stuk of een hoek van de draad te verplaatsen; { $p0 } kiest een greep voor de pijltjestoetsen
hint-select-handle-up-down = Greep { $p0 } van { $p1 }, een horizontaal stuk: ↑ en ↓ verschuiven het één rasterstap; { $p2 } gaat naar de volgende greep, { $p3 } terug naar de draad
hint-select-handle-sideways = Greep { $p0 } van { $p1 }, een verticaal stuk: ← en → verschuiven het één rasterstap; { $p2 } gaat naar de volgende greep, { $p3 } terug naar de draad
hint-select-handle-bend = Greep { $p0 } van { $p1 }, een hoek: de pijltjestoetsen verschuiven hem één rasterstap; { $p2 } gaat naar de volgende greep, { $p3 } terug naar de draad
tool-wire = Draad
hint-wire-armed = Klik of { $p0 } op een aansluitpunt, een aftakking of een draad om te beginnen, daarna op vrije punten om de draad erlangs te leiden
hint-wire-routing = Klik op een aansluitpunt om te eindigen, op een vrij punt voor een knik; { $p0 } verwijdert de laatste knik; { $p1 } annuleert
hint-wire-direct = Geen vrije route: de draad blijft een gestreepte lijn zonder route. Klik op vrije punten om hem te leiden, of routeer hem later met Routeren.
hint-wire-full-junction = Deze aftakking verbindt al drie draden: eindig op een draad voor een nieuwe aftakking.
hint-wire-bend = Knik geplaatst: klik op een aansluitpunt om de draad te beëindigen.
hint-wire-bend-beside = De klik plaatste een knik naast een aansluitpunt, niet erop: klik op het punt zelf om de draad daar te beëindigen; { $p0 } haalt de knik weg.
hint-wire-start-beside = De klik lag naast een aansluitpunt, niet erop: een draad begint op het punt zelf; klik op de ring.
hint-wire-ended-beside = De zojuist getekende draad knikt naast een aansluitpunt zonder erop aan te sluiten; { $p0 } neemt de draad terug.
hint-wire-click-tee = Een klik hier verbindt de lijn onder de aanwijzer met een schuine aftakking; { $p0 } maakt er een punt van; Alt+klik plaatst in plaats daarvan een knik.
hint-wire-click-dot = Een klik hier verbindt de lijn onder de aanwijzer met een punt; { $p0 } maakt er een schuine aftakking van; Alt+klik plaatst in plaats daarvan een knik.
hint-wire-click-start = Een klik hier is het beginpunt: klik verder ervan af om een knik te plaatsen.
hint-wire-over-text = De route loopt door een tekst: vrij blijven van de tekst is een veel langere weg.
hint-wire-round = Op de rechte lijn tussen deze twee punten staat iets, dus de draad gaat eromheen. Klik op vrije punten om hem anders te leiden.
hint-tube-armed = Klik of { $p0 } op een aansluiting, een aftakking of een slang om te beginnen, daarna op vrije punten om de slang erlangs te leiden
hint-tube-routing = Klik op een aansluiting om de slang te beëindigen, op een vrij punt voor een knik; { $p0 } verwijdert de laatste knik; { $p1 } annuleert
hint-tube-bend = Knik geplaatst: klik op een aansluiting om de slang te beëindigen.
hint-tube-bend-beside = De klik plaatste een knik naast een aansluiting, niet erop: klik op de aansluiting zelf om de slang daar te beëindigen; { $p0 } haalt de knik weg.
hint-tube-start-beside = De klik lag naast een aansluiting, niet erop: een slang begint op de aansluiting zelf; klik op de ring.
hint-tube-ended-beside = De zojuist getekende slang knikt naast een aansluiting zonder erop aan te sluiten; { $p0 } neemt de slang terug.
wire-readout = { $p0 } mm
hud-tube = { $p0 } leiding · slang { $p1 } mm
hud-tube-no-size = { $p0 } leiding · geen standaardslang
junction-title = Aftakking { $p0 }
junction-style = Stijl
junction-style-dot = Punt
junction-style-bevel = Schuin
junction-wiring = Bedrading
junction-wiring-star = Ster
junction-wiring-chain = Keten
junction-wiring-default = Standaard van de stijl
junction-sequence = Volgorde (Alt+Up, Alt+Down)
junction-wires = Fysieke draden
junction-wire-row = { $p0 }: { $p1 } naar { $p2 }
junction-orient = Oriëntatie { $p0 } (R)
ip-choice-source = Bron
ip-choice-target = Doel
hint-supply = Klik of { $p0 } om de voedingsmarkeringen te plaatsen; { $p1 } stopt
tool-box_pin = Naam van de aansluiting
hint-box-pin-add = Typ de naam van de aansluiting; Enter voegt haar toe; { $p0 } voegt haar toe en de volgende aansluiting van deze zijde; Esc annuleert
hint-box-pin-name = Typ de naam van de aansluiting; Enter neemt hem over; Esc annuleert
hint-box-pin-armed = Aansluiting toevoegen: de aansluiting komt op de rand van de box die het dichtst bij de aanwijzer ligt; klik waar ze moet komen en typ dan haar naam; Esc annuleert
label-box-pin-new = nieuwe aansluiting
tool-ip = Onderbrekingspunt
tool-black_box = Black box
hint-ip = Klik of { $p0 } om te plaatsen; { $p1 } wijzigt de richting; typ een naam; { $p2 } stopt
hint-ip-external = Klik of { $p0 } om het externe aansluitpunt te plaatsen; typ een naam; { $p1 } stopt
box-chip-armed = Black box: klik op twee hoeken
hint-box-armed = Klik op de eerste hoek van het kader en dan op de tegenoverliggende hoek; { $p0 } stopt
hint-box-part-armed = Dit artikel heeft geen eigen symbool en wordt als kader getekend: klik op de eerste hoek en dan op de tegenoverliggende hoek; { $p0 } stopt
hint-box-place = Klik om te plaatsen; { $p0 } stopt
place-target-click = { $p0 }: klik om te plaatsen
hint-box-outline = Klik of { $p0 } op de tegenoverliggende hoek; { $p1 } begint opnieuw
hint-box-dropped = Klik of { $p0 } op de eerste hoek; nogmaals { $p1 } verlaat het gereedschap
tool-text = Tekst
tool-text_edit = Tekst bewerken
tool-graphic = Grafisch
hint-text-armed = Klik of { $p0 } waar de tekst komt; { $p1 } stopt
hint-text-typing = Typ de tekst; Enter plaatst hem; Esc annuleert
hint-text-edit = Typ de nieuwe tekst; Enter wijzigt hem; Esc annuleert
hint-graphic-armed = Klik of { $p0 } op het eerste punt; { $p1 } stopt
hint-graphic-drawing = Klik of { $p0 } op het volgende punt; { $p1 } of dubbelklikken voltooit een polylijn of veelhoek; { $p2 } verwijdert het laatste punt
chooser-accessory-part = Het artikel kiezen
tool-image = Afbeelding
hint-image-picking = Kies eerst een afbeelding van het project
hint-image-armed = Klik of { $p0 } op de eerste hoek; { $p1 } stopt
hint-image-placing = Klik of { $p0 } op de tegenoverliggende hoek; { $p1 } of { $p2 } laat de eerste hoek los
marker-mode-per_wire = Zoals per draad ingesteld
marker-mode-all = Alle draden
marker-mode-deviations = Afwijkingen
marker-mode-explicit = Expliciete overschrijvingen
marker-mode-none = Geen
box-style-dotted = Gestippeld
box-style-dash_dot = Streep-punt
tool-marker = Draadaanduiding
tool-structure_box = Structuurkader
marker-crossed = { $p0 } gekruist
hint-marker-armed = Druk en sleep (of { $p0 }, verplaats, { $p0 }) over de draden om te markeren; { $p1 } stopt
hint-marker-crossing = Laat los (of { $p0 }) om de gekruiste draden te nemen
hint-marker-typing = Stel desgewenst de doorsnede en aderkleuren in; { $p0 } plaatst de aanduiding; { $p1 } annuleert
hint-structure-box-armed = Klik of { $p0 } op de eerste hoek; { $p1 } stopt
hint-structure-box-outline = Klik of { $p0 } op de volgende hoek; { $p1 } of dubbelklikken sluit het kader (twee hoeken: een rechthoek)
hint-structure-box-typing = Stel de stijl, het bijschrift, de locatie en het externe punt in; { $p0 } plaatst het kader; { $p1 } annuleert
tool-select = Selecteren
tool-measure = Meten
hint-select = Klik om te selecteren; sleep voor een selectierechthoek
hint-select-cp = Een draad begint op dit aansluitpunt: { $p0 }, dan een klik erop
hint-measure-armed = Klik of { $p0 } op het eerste punt
hint-measure-measuring = Klik of { $p0 } op het tweede punt; { $p1 } begint opnieuw
measure-label = dx { $p0 } dy { $p1 } mm, { $p2 } mm
preview-chip = Voorbeeld: { IN_SENTENCE($p0) }
preview-chip-stale = Voorbeeld: { IN_SENTENCE($p0) } (verouderd)
preview-chip-failed = Voorbeeld: { IN_SENTENCE($p0) } mislukt: { $p1 }
preview-chip-change = { $p0 } → { $p1 }
link-preview-title = { $p0 }
link-preview-go = Klik op de koppeling of druk op { $p0 } om erheen te gaan
link-preview-node = Koppelingsvoorbeeld: pagina { $p0 }, cel { $p1 }
link-preview-node-page = Koppelingsvoorbeeld: pagina { $p0 }
tip-link = Klik om naar { $p0 } te gaan
tip-pending = Bevindingen op de pagina die u bewerkt, die wachten tot u haar verlaat of een controle uitvoert
canvas-cursor-live-cp = , aansluitpunt { $p0 }
marker-export-dxf = DXF geëxporteerd naar { $p0 }
canvas-name = Pagina { $p0 }, { $p1 }
canvas-description = { $p0 }, zoom { $p1 }
canvas-stats-fps = { $p0 } fps
canvas-stats-frame = frame p99 { $p0 } ms
canvas-stats-phase = { $p0 } p99 { $p1 } ms
canvas-stats-tiles = tegels in wachtrij { $p0 }
canvas-stats-backend = backend { $p0 }
hud-name = Gereedschap
hud-stack = { $p0 } objecten overlappen hier · klik opnieuw om het volgende te selecteren
hud-selected = { $p0 } geselecteerd · { $p2 } verwijdert · { $p1 } draait
hud-terminal-sides = aansluiting { $p0 } is de binnenzijde (de bedrading in de kast), aansluiting { $p1 } de veldzijde
hud-wire-selected = { $p0 } geselecteerd · { $p1 } kiest een greep voor de pijltjestoetsen · { $p2 } verwijdert
hud-keys-place = { $p0 } draait · { $p1 } variant · { $p2 } klaar
hud-connect-on-move-on = { $p0 } Verbinden bij verplaatsen: aan
hud-connect-on-move-off = { $p0 } Verbinden bij verplaatsen: uit
hud-place-wires = verbindt { $p0 }; Alt+klik of { $p1 } plaatst zonder
hud-ip-connects = verbindt met { $p0 }
hud-supply-connects = { $p0 } verbindt met { $p1 }
hud-ip-taken = { $p0 } is bezet: dit punt wordt { $p1 }.
hud-ip-source = bron: de potentiaal vertrekt hier naar zijn doelen; één per naam
hud-ip-target = doel: de potentiaal komt hier aan vanaf zijn bron
hud-ip-undirected = zonder richting: koppelt in beide richtingen
hud-ip-star = sterbron: voedt elk punt met deze naam
hud-ip-next-target = volgende: een doel, waar { $p0 } aankomt (de bron is getekend)
hud-ip-next-source = volgende: de bron, waar { $p0 } vertrekt (de doelen wachten erop)
hud-ip-next-target-lone = volgende: nog een doel van { $p0 }, dat nog geen bron heeft (D maakt dit punt de bron)
hud-ip-field-keys = Enter beëindigt de naam, dan { $p0 }: { $p1 }
ghost-ip-star = { $p0 } sterbron
pages-title = Pagina's
page-rendering = Pagina { $p0 } wordt getekend…
page-no-pages = Het project heeft geen pagina's.
goto-title = Naar pagina of cel
goto-hint = Folio of cel
canvas-cp-unused = { $p0 } markeren als niet gebruikt
canvas-cp-used = { $p0 } markeren als gebruikt
canvas-make-cable = Kabel maken van de selectie…
canvas-go-partner = Naar partner { $p0 }
canvas-go-partner-none = Naar partner
# A pin with no wire to select (it touches a terminal directly) is named on its device's menu.
canvas-define-potential-pin = Potentiaal definiëren op { $p0 }…
canvas-ip-direction = Richting: { $p0 }
canvas-add-terminal = Klem toevoegen
canvas-edit-strip = Klemmenstrook bewerken
canvas-add-contact = Contact { $p0 } toevoegen
canvas-add-contact-none = Contact toevoegen
canvas-go-coil = Naar spoel { $p0 }
canvas-page-properties = Pagina-eigenschappen
canvas-go-to = Naar { $p0 }
canvas-device-delete = Verwijderen
canvas-go-to-none = Naar
tip-wire-unrouted = Deze draad heeft geen route en is daarom als dunne gestreepte lijn getekend: verplaats wat in de weg staat en kies Routeren.
retag-pair = { $p0 } heet nu { $p1 }
terminal-in-sentence = { $p0 } klem { $p1 }
pin-in-sentence = { $p0 } pin { $p1 } ({ $p2 })
marker-export = PDF geëxporteerd naar { $p0 }
marker-recovered = { $p0 ->
        [one] 1 wijziging uit het journaal hersteld
       *[other] { $p0 } wijzigingen uit het journaal hersteld
    }
label-paste-outside = deels buiten het tekengebied
label-paste-retag = { $p0 } wordt { $p1 }: een kenmerk volgt de pagina waarop zijn apparaat getekend staat
label-paste = { $p0 } plakken
label-paste-and = { $p0 } en { $p1 }
label-paste-symbols = { $p0 ->
        [one] 1 symbool
       *[other] { $p0 } symbolen
    }
label-paste-wires = { $p0 ->
        [one] 1 draad
       *[other] { $p0 } draden
    }
label-paste-other = { $p0 ->
        [one] 1 ander element
       *[other] { $p0 } andere elementen
    }
label-paste-dropped = { $p0 ->
        [one] ; 1 draad weggelaten: het andere eind was niet gekopieerd
       *[other] ; { $p0 } draden weggelaten: hun andere eind was niet gekopieerd
    }
tool-paste = Plakken
hint-paste = Klik of { $p0 } om te plakken; { $p1 } stopt
overview-page-title = Ventieleiland { $p0 }
canvas-solenoid-draw = Magneetspoel { $p0 } op een elektrische pagina tekenen…
canvas-solenoid-draw-here = Magneetspoel { $p0 } op deze pagina tekenen…
canvas-valve-go = Naar het ventiel { $p0 }
picker-blank = Blindplaat
picker-blank-detail = sluit de plaats af, zonder ventiel
canvas-plug-draw-here = De stekker van { $p0 } op deze pagina tekenen…
draw-plug-of = Stekker van { $p0 }
draw-page-title = Op een elektrische pagina tekenen
draw-page-new-title = Magneetspoelen
canvas-solenoid-go = Naar magneetspoel { $p0 } op { $p1 }
canvas-solenoid-output = Naar de uitgang van magneetspoel { $p0 } ({ $p1 })
canvas-solenoid-pin = Naar magneetspoel { $p0 }: pen { $p1 } van de stekker van { $p2 }
canvas-plug-draw = De stekker van { $p0 } op een elektrische pagina tekenen…
canvas-position = Positie { $p0 }: { $p1 }
canvas-position-empty = leeg
canvas-position-blank = blindplaat
canvas-medium-none-go = Geen medium: ga naar de persluchtaansluiting { $p0 } en kies er daar een
canvas-plug-go = Naar de stekker van { $p0 } op { $p1 }
canvas-place-port = Poort { $p0 } plaatsen
canvas-plc-unlock-module-starts = Modulestarts ontgrendelen
canvas-plc-unlock-symbols = Symbolen ontgrendelen
canvas-plc-unlock-plc-data = PLC-gegevens ontgrendelen
canvas-line-medium = Medium: { $p0 }
canvas-valve-add = Ventiel toevoegen op plaats { $p0 }
canvas-cell-fill = Positie { $p0 } vullen…
canvas-cell-replace = Vervangen op positie { $p0 }…
canvas-cell-empty = Positie { $p0 } leegmaken
canvas-cell-blank = Blindplaat op positie { $p0 }
canvas-valve-used = In gebruik zetten
canvas-valve-spare = Reserve zetten
canvas-valve-blank = Plaats { $p0 } afdekken
canvas-port-plug = Aansluiting { $p0 } afpluggen
canvas-line-kind = Leidingsoort: { $p0 }
marker-export-table = De tabel geëxporteerd naar { $p0 }
link-preview-go-none = Klik op de koppeling of gebruik Pagina ▸ Kruisverwijzing volgen om erheen te gaan
hint-termination = on of off, zoals de afsluitschakelaar staat; leeg voor een poort zonder
canvas-cursor-description = , cursor { $p0 } x { $p1 } y { $p2 } mm
canvas-cursor-description-no-cell = , cursor x { $p0 } y { $p1 } mm
canvas-cursor-name = Cursor van het tekenvlak
canvas-cursor-live = { $p0 }, x { $p1 } y { $p2 } mm
canvas-cursor-live-link = , koppeling naar { $p0 }
page-grid-fit-title = Verbindingsraster wijzigen
page-grid-fit-ok = Wijzigen en aanpassen
page-grid-fit-foot = Pagina { $p0 } bevat { $p1 } items en { $p2 } draden. Op het raster van { $p3 } worden er { $p4 } op het raster gezet en worden de draden opnieuw getekend; Ongedaan maken zet alles in één stap terug.
terminal-form-title = Klemmen plaatsen
terminal-levels-one = 1: een klem met één etage
terminal-levels-two = 2: een blok met twee etages, één klem en één artikel met een nummer op elke etage
canvas-choose-part-strip = { $p0 ->
        [one] Artikel kiezen voor 1 klem zonder artikel…
       *[other] Artikel kiezen voor { $p0 } klemmen zonder artikel…
    }
node-candidate-under = { $p0 } (onder { $p1 })
node-candidate-top = { $p0 } (hoogste niveau)
preview-none = Er is geen voorbeeld open
preview-header = Voorbeeld: { IN_SENTENCE($p0) } · vanaf revisie { $p1 }
preview-title = Voorbeeld: { IN_SENTENCE($p0) }
preview-stale = verouderd
preview-records = { $p0 ->
        [one] 1 element
       *[other] { $p0 } elementen
    }
preview-parts-new = { $p0 ->
        [one] 1 nieuw artikel
       *[other] { $p0 } nieuwe artikelen
    }
preview-parts-skipped = { $p0 ->
        [one] 1 al in het project
       *[other] { $p0 } al in het project
    }
preview-import-summary = { $p0 } · { $p1 }
preview-records-made-of = { $p0 }: { $p1 }
preview-move-lead = { $p0 ->
        [one] Verplaatst 1 symbool naar pagina { $p1 }
       *[other] Verplaatst { $p0 } symbolen naar pagina { $p1 }
    }
preview-move-lead-new = { $p0 ->
        [one] Verplaatst 1 symbool naar de nieuwe pagina { $p1 }
       *[other] Verplaatst { $p0 } symbolen naar de nieuwe pagina { $p1 }
    }
preview-move-stays = { $p0 ->
        [one] 1 geselecteerde tekst of kader blijft op pagina { $p1 }
       *[other] { $p0 } geselecteerde teksten en kaders blijven op pagina { $p1 }
    }
preview-tags-count = { $p0 ->
        [one] 1 kenmerk
       *[other] { $p0 } kenmerken
    }
preview-xrefs-count = { $p0 ->
        [one] 1 kruisverwijzingstekst
       *[other] { $p0 } kruisverwijzingsteksten
    }
preview-changes-count = { $p0 ->
        [one] 1 wijziging
       *[other] { $p0 } wijzigingen
    }
preview-pages-count = { $p0 ->
        [one] 1 pagina
       *[other] { $p0 } pagina's
    }
preview-entities = Entiteiten
preview-notices = Wijzigingen
preview-tags = Kenmerken
preview-xrefs = Kruisverwijzingen
preview-wires = Draadnummers
preview-pages = Pagina's
preview-open-chip = Voorbeeld open: { $p0 }
preview-show = Tonen
preview-apply = Toepassen
preview-cancel = Annuleren
canvas-scroll-horizontal = Pagina zijwaarts schuiven
canvas-scroll-vertical = Pagina op en neer schuiven
marker-saved = Opgeslagen
renumber-pages-title = Pagina's hernummeren
renumber-devices-title = Apparaten hernummeren
page-copy-title = Pagina's kopiëren
page-copy-lead = { $p0 ->
        [one] Pagina { $p1 } kopiëren
       *[other] { $p0 } pagina's kopiëren: { $p1 }
    }
page-copy-drawn-elsewhere = Ook getekend op pagina's die niet gekopieerd worden, dus hun kopieën tekenen alleen wat op deze pagina's staat: { $p0 }
page-copy-frozen-cleared = Vastgezette kenmerken die het hernoemen laat zoals ze zijn; de kopieën krijgen het kenmerk van het schema op hun nieuwe pagina: { $p0 }
page-copy-continued = Onderbrekingspunten waarvan de kopieën de punten buiten de kopie voortzetten: { $p0 }
page-copy-texts = { $p0 ->
        [one] 1 tekst die een gekopieerd apparaat of onderbrekingspunt noemt, krijgt de naam van de kopie
       *[other] { $p0 } teksten die een gekopieerd apparaat of onderbrekingspunt noemen, krijgen de naam van de kopie
    }
page-copy-texts-left = Teksten die blijven zoals ze zijn, omdat gekopieerde apparaten het kenmerk deelden dat ze noemen: { $p0 }
page-copy-titles = Paginatitels die wijzigen: { $p0 }
page-copy-bus = Busapparaten waarvan de kopieën een eigen buslijn en adres nodig hebben: { $p0 }
page-copy-title-renamed = { $p0 } → { $p1 }
page-copy-refused = Pagina's kopiëren: { $p0 }
tool-macro = Macro
hint-macro = Klik of { $p0 } om de macro in te voegen; { $p1 } stopt
label-macro = { $p0 } invoegen
macro-slots-default = zoals het symbool
macro-slots-slot = tekstvak
macro-slots-at = op
macro-slots-turned = gedraaid
macro-slots-hidden = verborgen
macro-slots-shown = zichtbaar
macro-conflict-tag = Het kenmerk { $p0 } wordt door een ander apparaat afgedrukt
macro-conflict-pairs = { $p0 ->
        [one] { $p1 } vormt een paar met een bestaand onderbrekingspunt
       *[other] { $p1 } vormt een paar met { $p0 } bestaande onderbrekingspunten
    }
macro-conflict-address = Het PLC-adres { $p0 } is bezet
macro-conflict-process-tag = Het proceskenmerk { $p0 } is in gebruik
macro-conflicts = Conflicten
macro-no-conflicts = Geen conflicten
macro-failed = { $p0 ->
        [one] De macro botst met dit project en kan hier niet worden ingevoegd (1 conflict)
       *[other] De macro botst met dit project en kan hier niet worden ingevoegd ({ $p0 } conflicten)
    }
macro-failed-other = De macro kan hier niet worden ingevoegd: { $p0 }
macro-value-set = Waardenset
macro-value-set-defaults = Standaardwaarden
macro-no-preview = Geen voorbeeld
macro-preview-name = Voorbeeld van { $p0 }
macro-devices = Apparaten
macro-variables = Variabelen
macro-version = Versie { $p0 } · { $p1 }
hud-paste-versions-one = Er staat een andere versie van { $p0 } in het project; kies in de kaart hieronder
hud-paste-versions-many = Er staan andere versies van { $p0 } in het project; kies in de kaart hieronder
hud-paste-update = Pas de update toe in de kaart hieronder; het plakken volgt
preview-field-content = inhoud
preview-change-added = Toegevoegd
preview-change-changed = Gewijzigd
preview-change-removed = Verwijderd
paste-class-patch = een gegevenswijziging
paste-class-minor = een compatibele wijziging
paste-class-major = een ingrijpende wijziging
paste-adapt = De kopie van het project gebruiken
paste-update = De kopie van het project bijwerken
paste-conflict = { $p0 }: dit project { $p1 }, geplakt { $p2 }, { $p3 }
macro-values = Waarden
macro-value-label = { $p0 } ({ $p1 })
# De regel van een variabele van een macro met zijn standaardwaarde.
macro-variable-default = { $p0 } = { $p1 }
var-type-text = tekst
var-type-ident = identificatie
var-type-number = getal
var-type-bool = true of false
var-type-state = status
var-type-part = artikelverwijzing
var-type-process-tag = proceskenmerk
var-type-channel-binding = kanaalnummer
var-type-address = adres
var-type-tag-counter = kenmerkteller
var-type-device-ref = apparaat
macro-failed-value = De macro kan niet worden ingevoegd met de waarde die voor { $p0 } is getypt.
tool-layout_place = Apparaat plaatsen
hint-layout-place = Klik of laat los om te plaatsen; { $p0 } stopt
hint-layout-idle = Sleep een apparaat uit Ongeplaatste apparaten naar de pagina
hud-layout-place = { $p0 } plaatsen · Ctrl duwt · Alt wisselt · Shift zonder snappen · { $p1 } klaar
hud-layout-selected = { $p0 } geselecteerd · pijltoetsen verschuiven · { $p1 } verwijdert
hud-layout-selected-plate = { $p0 } geselecteerd · pijltoetsen verschuiven · { $p1 } draait · { $p2 } verwijdert
hud-layout-selected-rail = { $p0 } geselecteerd · pijltoetsen verschuiven · { $p1 } keert om · { $p2 } verwijdert
hud-layout-place-next = Volgende stap: sleep een apparaat uit Ongeplaatste apparaten op de plaat, of vul een rail in één keer
hud-layout-route-next = Volgende stap: Routeren geeft elke draad zijn loop door de kabelgoot en zijn lengte
hud-layout-ways-stale = { $p0 ->
        [one] De loop van 1 draad klopt niet meer: Routeren geeft een nieuwe
       *[other] De lopen van { $p0 } draden kloppen niet meer: Routeren geeft nieuwe
    }
hud-list-page = Pagina { $p0 } van { $p1 } van deze lijst
hud-list-next = Volgende pagina { $p0 }
hud-list-previous = Vorige pagina { $p0 }
hud-layout-locate-first = Volgende stap: geef de apparaten van de kast de locatie { $p0 }, sleep ze dan uit Ongeplaatste apparaten op de plaat
hud-layout-give-location = Locatie { $p0 } geven…
locate-title = De apparaten van de kast hun locatie geven
locate-ok = Locatie geven
locate-row-prefixed = { $p0 }: het kenmerk wordt afgedrukt met { $p1 } ervoor, omdat de pagina een andere locatie tekent
hud-layout-start = Begin met een kast: Kast op het tabblad Tekenen zet hem met zijn montageplaat op deze pagina
tool-rail_draw = Rail
tool-duct_draw = Kabelgoot
hint-rail-draw = Klik op het begin van de rail, dan op het einde; { $p0 } annuleert
hint-rail-draw-end = Klik op het einde van de rail; { $p0 } laat het begin los; { $p1 } annuleert
hint-duct-draw = Klik op het eerste punt van de kabelgoot, dan op elk volgend punt; dubbelklikken of { $p0 } voltooit; { $p1 } annuleert
hint-duct-draw-next = Klik op het volgende punt; dubbelklikken of { $p0 } voltooit; { $p1 } verwijdert het laatste punt
draw-length = { $p0 } mm
draw-duct-size = { $p0 } × { $p1 } mm
draw-options-part = Artikel
draw-options-width = Breedte (mm)
draw-options-height = Hoogte (mm)
draw-options-no-part = Geen artikel
canvas-labels-tags = Labels: Kenmerken
canvas-labels-item-numbers = Labels: Positienummers
canvas-labels-both = Labels: Beide
canvas-labels-tags-or-numbers = Labels: Kenmerken of nummers
devices-location-title = Locatie van de apparaten
devices-location-ok = Locatie instellen
strip-no-tag-yet = Nieuwe klemmenstrook · nog geen kenmerk
tip-line-on-cp = Een lijn maakt geen verbinding. Teken een draad.
tip-derived-tag = Kenmerken volgen uit de structuur en de teller.
tip-generated-page = Deze pagina wordt gegenereerd uit een rapportblok.
tip-wheel-zooms = Het muiswiel zoomt de pagina. Het kan de pagina ook schuiven, Ctrl met het wiel zoomt dan (Beeld ▸ Muiswiel schuift het tekenvlak).
cable-new-title = Nieuwe kabel
connect-cores-title = Aders verbinden
cable-wires-count = { $p0 ->
        [one] 1 draad
       *[other] { $p0 } draden
    }
bus-cable-lines-count = { $p0 ->
        [one] de buslijn
       *[other] { $p0 } buslijnen
    }
picker-symbol-other-grid = Dit symbool is getekend op het verbindingsraster { $p0 }, en deze pagina gebruikt { $p1 }. Plaatsen zet het hier neer met zijn aansluitpunten op het raster van deze pagina.
route-folio = folio { $p0 }
route-preview-lead = Routeren: { $p0 }. { $p1 } draden krijgen een nieuwe route, { $p2 } daarvan door u getekend en onderbroken. Er is nog niets veranderd.
route-reason-wires = Geen weg voor { $p0 }: andere draden staan in de weg.
route-reason-symbols = Geen weg voor { $p0 }: symbolen staan in de weg.
route-reason-points = Geen weg voor { $p0 }: aansluitpunten van andere draden staan in de weg.
route-reason-four-way = Geen weg voor { $p0 }: het zou een vierde arm aan een aftakking zijn.
route-his = U tekende deze draad en hij is onderbroken; hij krijgt een nieuwe route: { $p0 }.
route-against-order = De draad { $p0 } en de andere van zijn potentiaal zijn in een andere volgorde bedraad dan hun symbolen staan: bedraad ze in de volgorde van de symbolen voor een rechte rail.
route-selection-hint = Selecteer niets om de hele pagina te routeren.
route-kept = { $p0 ->
        [one] 1 draad houdt de route die hij heeft.
       *[other] { $p0 } draden houden de route die ze hebben.
    }
route-preview-lead-his = { $p0 ->
        [one] Routeren: { $p1 }. 1 door u getekende draad krijgt een nieuwe route. Er is nog niets veranderd
       *[other] Routeren: { $p1 }. { $p0 } door u getekende draden krijgen een nieuwe route. Er is nog niets veranderd
    }.
route-more = nog { $p0 }
drawing-title = { $p0 } importeren
drawing-dxf-text = { $p0 } is een DXF-tekening. Voeg haar toe als nieuwe grafische pagina, of leg haar als onderlegger onder de getoonde pagina om over te tekenen.
drawing-dwg-text = { $p0 } is een DWG-tekening: ze wordt gelezen via de ODA File Converter, daarna als DXF.
drawing-pdf-text = { $p0 } is een PDF. De eerste pagina wordt als onderlegger onder de getoonde pagina gelegd om over te tekenen.
drawing-unitless = De tekening noemt geen eenheid en wordt daarom in millimeters gelezen.
drawing-converter = ElektraWatt levert de ODA File Converter niet mee. Installeer hem van de Open Design Alliance; zijn programma wordt eenmalig gekozen, bij het importeren.
drawing-underlay-text = Een onderlegger blijft buiten het project, tenzij hij wordt ingesloten. Een tekening van een klant kan vertrouwelijk zijn: sluit haar alleen in als het project haar mag bevatten.
drawing-page = Als pagina toevoegen
drawing-underlay = Onder pagina { $p0 } leggen
drawing-embed = Leggen en insluiten
canvas-plc-of = PLC: { $p0 }
canvas-plc-add-to = Aan { $p0 } toevoegen
canvas-plc-add-new = Aan een nieuwe PLC toevoegen
canvas-plc-add = Aan PLC toevoegen
hint-date = Een datum, zoals { $p0 }
hint-select-legend = Hier past de legenda: klik met rechts en kies Legenda toevoegen om de apparaten naast de layout op te sommen
keep-in-place-untagged = een symbool zonder kenmerk
canvas-medium-none = Geen medium: kies er een bij de persluchtaansluiting
hud-bus-page-steps = 1 Zet de stations hier (rechtsklik ▸ Station op deze pagina zetten…) · 2 Verbind hun poorten met Draad · 3 Geef hun adressen in het Bus-paneel
box-side-top = Boven
box-side-bottom = Onder
box-side-left = Links
box-side-right = Rechts
box-side-invalid = Zet de pen op de boven-, onder-, linker- of rechterrand.
box-new-part = Nieuw artikel
box-what-line = Apparaatkast · { $p0 }
box-line = Een kast met pennen, zoals een apparaat van dit artikel wordt getekend
box-pins-from-channels = Dit artikel is een PLC-kaart: de namen van de pennen en wat ze voeren komen van de kanalen en voedingspennen, te wijzigen in de artikeleditor onder PLC. Waar elke pen op de kast staat, en de maat van de kast, wijzigt u in zijn apparaatkast (Pennen en kast…).
box-pins-from-ports = Dit artikel is een busapparaat: de namen van de pennen en wat ze voeren zijn van het artikel, en de kast op een buspagina komt van de interfaces en poorten, te wijzigen in de artikeleditor onder Businterfaces. Waar elke pen op zijn eigen kast staat, en de maat van die kast, wijzigt u in zijn apparaatkast (Pennen en kast…).
box-pins-from-symbol = Dit artikel wordt met een symbool getekend: de aansluitpunten zijn die van het symbool. Wijzig ze in het symbool.
box-unsaved = Deze apparaatkast heeft wijzigingen die niet zijn opgeslagen. Sla ze op of verwerp de wijzigingen.
box-pin-unknown = Het artikel heeft die pen niet.
box-pin-on-step = De pen staat op { $p0 } mm: een pen staat op een stap van { $p1 } mm, vrij van de hoeken, en { $p2 } mm is er geen.
box-todo-number = Geef het artikel een artikelnummer.
box-todo-manufacturer = Noem de fabrikant van het artikel.
box-identity-in-part-editor = De identiteit van het artikel wijzigt u in de artikeleditor: Artikelgegevens…
box-pin-name = Naam
box-pin-side = Rand
box-pin-at = Langs de rand (mm)
box-pin-at-help = De afstand van de pen tot de linkerbovenhoek van de kast langs de rand, in stappen van het verbindingsraster van 4 mm.
box-pin-title = Pen { $p0 }
box-pin-line = Een pen van het apparaat: waar een draad eindigt
box-pins-selected = { $p0 } pennen geselecteerd
box-saved = De apparaatkast van { $p0 } is opgeslagen. Tekenen ▸ Apparaat plaatst hem met één klik.
box-saved-redraw = De apparaatkast van { $p0 } is opgeslagen. Kasten ervan die nog op de oude manier zijn getekend: { $p1 }.
box-redraw = Opnieuw tekenen
box-redrawn = Kasten opnieuw getekend: { $p0 } zoals hun artikel ze nu indeelt, de draden zijn meegegaan.
box-test-armed = Klik op de pagina om een apparaat van { $p0 } te plaatsen.
box-outline = Apparaatkast
box-pins = Pennen
box-add-pin = Pen toevoegen
box-add-pin-how = Voegt een pen toe aan de onderrand; sleep hem naar zijn plaats, of naar een andere rand
box-add-bus = Businterface toevoegen…
box-bus-library = Een artikel uit een bibliotheek houdt de businterfaces die de bibliotheek het geeft; maak er een eigen artikel van om er een toe te voegen
box-pins-empty = Nog geen pennen. Pen toevoegen zet de eerste op de kast.
box-canvas = Apparaatkast van { $p0 }
box-part-data = Artikelgegevens…
box-part-data-how = De artikeleditor: identiteit, technische gegevens, afmetingen
box-select-how = Klik op een pen om hem te selecteren; sleep hem langs zijn rand of naar een andere; sleep de randen van de kast om hem groter of kleiner te maken; de pijltoetsen verplaatsen een pen een rasterstap, Delete verwijdert hem
box-hint = Sleep een pen langs zijn rand of naar een andere. Sleep een rand van de kast om hem groter of kleiner te maken.
box-close = Kast sluiten
box-ready = Klaar om op te slaan
box-save-how = Slaat het artikel op met zijn pennen en zijn kast
box-test-how = Slaat de kast op en activeert Apparaat met dit artikel: een klik op de pagina plaatst het
box-copy-new = Een eigen kopie van { $p0 }: het origineel blijft zoals het is. De kopie houdt het artikelnummer { $p0 } tot u haar een eigen nummer geeft in Artikelgegevens…; Opslaan bewaart haar in uw eigen bibliotheek.
box-copy-fork = { $p0 } kunt u niet wijzigen: Opslaan maakt een eigen kopie in dit project, en de apparaten van dit artikel nemen die over.
terminal-spacing-one = 1 rasterstap · { $p0 } mm
terminal-spacing-steps = { $p0 } rasterstappen · { $p1 } mm
terminals-spacing-title = Tussenafstand van klemmen
terminals-spacing-ok = Verdelen
tool-frame = Kader bewerken
hint-frame = Klik een vak, een lijn of de nummers · Dubbelklik een vak om te typen · Esc stopt
hint-frame-typing = Typ de tekst van het vak · Enter neemt hem over · Esc laat het vak zoals het was
frame-card = Pagina { $p0 } gebruikt { $p1 }. Elke wijziging komt op elke pagina die ze gebruikt.
frame-card-project = het kader en titelblok van het project
frame-card-own = een eigen kader en titelblok van het project
frame-split-side = Naast elkaar splitsen
frame-split-above = Boven elkaar splitsen
frame-join = Samenvoegen
frame-picture = Afbeelding…
frame-done = Klaar
frame-align-left = Links
frame-align-centre = Midden
frame-align-right = Rechts
frame-row-text = Tekst
frame-row-shows = Toont
frame-row-size = Tekstgrootte
frame-row-align = Uitlijning
frame-row-typed-in = Ingevuld in
frame-typed-in = Titelblokvelden…, Project-/Pagina-eigenschappen
frame-row-size-mm = Breedte × hoogte
frame-size-mm = { $p0 } × { $p1 } mm
frame-cell-empty = Leeg vak
frame-cell-where = { $p0 ->
        [one] Vak van het titelblok · komt op { $p0 } pagina
       *[other] Vak van het titelblok · komt op { $p0 } pagina's
    }
frame-title = Kader
frame-pages = { $p0 ->
        [one] Gebruikt door { $p0 } pagina
       *[other] Gebruikt door { $p0 } pagina's
    }
frame-row-paper = Papier
frame-row-title-block = Titelblok
frame-block-shared = Gedeeld met de andere kaders
frame-block-own = Eigen
frame-row-rulers = Kolommen en rijen
frame-axis-numbers = { $p0 } genummerd vanaf { $p1 }
frame-axis-letters = { $p0 } met letters vanaf { $p1 }
shows-notice = Mededeling
shows-previous = Vorige
shows-next = Volgende
shows-function = = Functie
shows-location = + Plaats
shows-changed-in = Gewijzigd in revisie
shows-page-count = Aantal pagina's
shows-state = Status
shows-name = Naam
shows-group-project = Project
shows-group-page = Pagina
shows-group-revision = Revisie
shows-group-people = Personen
shows-group-own = Eigen velden
shows-group-other = Overig
shows-company-logo = Bedrijfslogo
shows-picture = Een afbeelding…
shows-text-only = Alleen tekst
toast-frame-forked = Kader en titelblok zijn nu van dit project; die van het programma blijven zoals ze waren.
toast-frame-join-dropped = Samenvoegen liet de tekst { $p0 } vallen.
frame-section-cell = Vak
frame-section-columns = Kolommen, langs de bovenkant
frame-section-rows = Rijen, langs de linkerkant
frame-section-both = Beide
frame-section-frame = Kader
frame-how-many = Aantal
frame-counted-with = Geteld met
frame-numbers = Cijfers
frame-letters = Letters
frame-first = Eerste
frame-skip-io = I en O overslaan
frame-drawn = Getekend
frame-yes = Ja
frame-no = Nee
frame-block-height = Hoogte titelblok (mm)
frame-row-own-frame = Deze pagina
frame-own-frame-give = Deze pagina een eigen kader geven
frame-row-adopt = Sjablonen
frame-template = Sjabloon

# Route on a panel page: the card of a cabinet's plan (doc 12 §12.5).
cabinet-route-lead = { $p0 ->
        [one] Routeren: kast { $p1 }. 1 draad krijgt een loop door de kabelgoot; { $p2 } worden ter plaatse gemaakt. Er is nog niets veranderd.
       *[other] Routeren: kast { $p1 }. { $p0 } draden krijgen een loop door de kabelgoot; { $p2 } worden ter plaatse gemaakt. Er is nog niets veranderd.
    }
cabinet-route-lead-one-site = { $p0 ->
        [one] Routeren: kast { $p1 }. 1 draad krijgt een loop door de kabelgoot; 1 wordt ter plaatse gemaakt. Er is nog niets veranderd.
       *[other] Routeren: kast { $p1 }. { $p0 } draden krijgen een loop door de kabelgoot; 1 wordt ter plaatse gemaakt. Er is nog niets veranderd.
    }
cabinet-route-lead-gone = { $p0 ->
        [one] Routeren: kast { $p1 }. 1 loop van een draad die deze kast niet meer legt, wordt verwijderd. Er is nog niets veranderd.
       *[other] Routeren: kast { $p1 }. { $p0 } lopen van draden die deze kast niet meer legt, worden verwijderd. Er is nog niets veranderd.
    }
cabinet-route-ducts = Per kabelgoot
cabinet-route-lengths = Lengtes
cabinet-route-typed = Met de hand getypt
cabinet-route-site-heading = Ter plaatse gemaakt
cabinet-route-without = Zonder loop
cabinet-route-duct-wires = { $p0 ->
        [one] 1 draad · volste stuk { $p1 }
       *[other] { $p0 } draden · volste stuk { $p1 }
    }
cabinet-route-duct-limit = maximale vulgraad { $p0 } %
way-chip = { $p0 } · { $p1 }
way-to-door = naar { $p0 } in de deur
cabinet-route-typed-row = houdt { $p0 }; Routeren geeft { $p1 }
cabinet-route-to-door = { $p0 ->
        [one] 1 draad naar de deur
       *[other] { $p0 } draden naar de deur
    }
cabinet-route-chip = { $p0 } %
cabinet-route-chip-over = { $p0 } % · boven { $p1 } %
cabinet-route-all = { $p0 ->
        [one] Alle draden opnieuw routeren (1 meer, waarvan de loop klopt)
       *[other] Alle draden opnieuw routeren ({ $p0 } meer, waarvan de lopen kloppen)
    }
cabinet-route-all-on = Alle draden opnieuw routeren (0 meer: elke draad is gepland)
