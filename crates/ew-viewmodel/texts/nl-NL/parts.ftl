# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: GPL-3.0-or-later

# The texts of the window in nl-NL: parts, part cards, libraries and accessories. Every key of the en-GB file of the
# same name, with the same arguments; a message missing here falls back to en-GB. Product and file names, rule IDs,
# command names, key caps, identifiers and units are not translated.

# F22: de gegevens van een onderdeel in één oogopslag.
card-type = Type
card-order-number = Bestelnummer
card-size = Afmetingen
card-gives = Levert
card-from = Herkomst
card-interface-ports = { $p0 }, { $p1 } poorten
card-cps = { $p0 ->
        [one] 1 aansluitpunt
       *[other] { $p0 } aansluitpunten
    }
card-project-only = Alleen in dit project
card-library-version = { $p0 }, versie { $p1 }
card-source-checked = { $p0 } · gecontroleerd
card-source-unchecked = { $p0 } · niet gecontroleerd
card-no-size = Geen afmetingen opgegeven
card-size-beside-rail = Op schaal naast een rail van 35 mm
# F23: Afmetingen en montage in de onderdeeleditor.
part-size-hint-dimensions = Gezien van voren als het gemonteerd is: zo groot tekent de layout het artikel.
part-size-hint-mount = Hoe het vastzit: op een rail, op de montageplaat geschroefd, in een deuruitsparing of niet gemonteerd.
part-size-hint-mass = Het gewicht, voor het totaal van de kast.
part-size-hint-clearance = De ruimte die de layout rond het artikel vrijhoudt, voor warmte of bedrading.
part-size-mount-none = Niet opgegeven
accessory-at-terminal = klem { $p1 } van { $p0 }
origin-eplan = EPLAN
origin-qet = QElectroTech
origin-kicad = KiCad
origin-other = Een ander programma
card-line = { $p0 } { $p1 } · { $p2 }
card-apply = Toepassen
card-cancel = Annuleren
card-failed = mislukt: { $p0 }
card-strip-failed = { $p0 } mislukt: { $p1 }
card-as-of = vanaf revisie { $p0 }
card-title-reorder = Pagina's herschikken
card-title-fix = Snelle oplossing { $p0 }
card-title-find = Vervangen
card-title-properties = Voorbeeld van { IN_SENTENCE($p0) }
parts-map-title = Kolommen toewijzen
parts-map-source = Bron
parts-map-saved = Opgeslagen toewijzing
parts-map-column = Kolom in het bestand
parts-map-sample = Eerste waarde
parts-map-member = Gaat naar
parts-map-none = Niet geïmporteerd
parts-map-import = Importeren
parts-map-cancel = Annuleren
parts-map-twice = `{ $p0 }` is voor twee kolommen gekozen
parts-map-twice-text = { $p0 } is voor twee kolommen gekozen
parts-map-missing = Wijs een kolom toe aan { $p0 }
parts-map-mixed = Wijs één beschrijving toe, of één per taal, niet beide
parts-member-description-lang = { $p0 }, { $p1 }
parts-member-in-group = { $p0 } ▸ { $p1 }
parts-member-like = Zoals (het nummer van een artikel waar het mee begint)
parts-member-tech-data = Technische gegevens: { $p0 }
parts-member-free-prop = Vrije eigenschap: { $p0 }
card-table-drop = Weglaten
card-reorder = { $p0 } · { $p1 }
card-failed-rule = Regel { $p0 }
library-copy-kind-key = { $p0 } · { $p1 }
library-copy-not-installed = geen geïnstalleerde bibliotheek
part-choose-fits = Alleen passende artikelen
part-choose-view-all = Alle
part-choose-view-project = In dit project
part-choose-view-favourites = Favorieten
part-choose-view-recent = Recent
part-choose-inactive = Verouderde en ingetrokken tonen
part-choose-empty = Geen artikel komt overeen met “{ $p0 }”
part-choose-nothing = Geen artikel om te kiezen; wijzig de zoekopdracht.
part-edit-fork-note = Opslaan maakt een projectkopie van dit artikel; de bibliotheek wordt niet gewijzigd
part-card-field = { $p0 }: { $p1 } → { $p2 }
part-card-devices = Gebruikt door { $p0 }
part-card-removed = Stuklijstregel verwijderd: { $p0 }
part-card-added = Stuklijstregel toegevoegd: { $p0 }
part-card-changed = Stuklijstregel gewijzigd: { $p0 }
part-card-lost = Een buslijn eindigt op een aansluiting die het artikel niet meer heeft: { $p0 }. Hij houdt zijn eind daar; teken hem naar een andere aansluiting.
part-card-box = Opnieuw getekend: { $p0 } op pagina { $p1 }, met de pennen die het artikel nu heeft
part-card-box-moved = Opnieuw getekend: { $p0 } op pagina { $p1 }, teruggezet binnen het tekengebied
part-card-symbol = Opnieuw getekend: { $p0 } op pagina { $p1 }, de aansluitingen van het bussymbool zoals de poorten van het artikel ze nu geven
part-card-symbol-added = Opnieuw getekend: { $p0 } op pagina { $p1 }, het bussymbool met { $p2 } erbij
part-card-own-symbol-lacks = Uw bussymbool van { $p0 } tekent { $p1 } niet: teken het erbij, of gebruik weer het symbool van het programma
part-card-own-symbol-extra = Uw bussymbool van { $p0 } tekent { $p1 }, dat de netwerkpoorten niet meer hebben
part-card-symbol-removed = Opnieuw getekend: { $p0 } op pagina { $p1 }, het bussymbool zonder { $p2 }
part-change-interface = Interface { $p0 }
part-change-ports = { $p0 ->
        [one] 1 poort
       *[other] { $p0 } poorten
    }
part-change-other = Overige artikelgegevens
part-change-other-changed = gewijzigd
part-field-functions = Functies
part-field-cps = Aansluitpunten
part-field-source = Bron
part-choose-search = Artikelen zoeken
part-choose-list = Artikelen
part-choose-favourite = Favoriet
part-choose-detail = { $p0 } · versie { $p1 } · { $p2 }
part-choose-detail-successor = { $p0 } · versie { $p1 } · { $p2 }, opvolger { $p3 }
part-choose-detail-functions = { $p0 } · functies { $p1 }
part-choose-target-bridge = de brug { $p0 }
part-choose-target-accessory = het { $p0 } { $p1 }
part-choose-target-devices = { $p0 } apparaten
part-none = geen
part-form-message = { $p0 }: { $p1 }
part-placeholder-description = Wat het is
part-placeholder-note = De stuklijst noemt het TBD met deze woorden; kies later voor een van zijn apparaten het echte artikel en elk apparaat dat het heeft krijgt dat artikel.
part-placeholder-use = Als plaatsvervanger gebruiken
part-placeholder-empty = Typ eerst wat het artikel is
part-hint-ports = Eén rij per interface van het gegevensblad. Controller of master leidt de bus (een CPU); apparaat of slave is een knooppunt met een adres (een kopstation, een aandrijving, een managed switch); passief heeft geen adres en geeft het net door (een busstekker, een unmanaged switch). Poorten: het aantal aansluitbussen van één interface, zoals 2 of 8; de interface houdt één adres voor allemaal.
part-bus-interface = Interface
part-bus-port_label = Poortlabel
part-bus-bus = Bus
part-bus-role = Rol
part-bus-ports = Poorten
part-bus-termination = Afsluitschakelaar
part-bus-termination-none = Geen
part-bus-add = Interface toevoegen
part-bus-remove = Interface { $p0 } verwijderen
part-bus-none = Nog geen businterface: voeg er een toe voor een apparaat aan een bus.
part-drawn-with = Getekend met
stock-summary = Werkplaatsvoorraad: { $p0 } artikelen
parts-stock-chip = Werkplaatsvoorraad
parts-stock-chip-hint = Toont de werkplaatsvoorraad van dit project in haar volgorde. Ze wordt hier gewijzigd: sleep een rij naar een andere plaats, of open een artikel en gebruik Omhoog, Omlaag, Eruit halen of Aan de voorraad toevoegen.
parts-stock-count = { $p0 } in de voorraad van dit project
parts-stock-group = { $p0 } · { $p1 }
parts-stock-rule-wire = de eerste van zijn maat en kleur
parts-stock-rule-ferrule = de eerste die past
parts-stock-rule-marker = de eerste die ervoor gemaakt is
parts-stock-rule-gland = de eerste die afdicht
parts-stock-in = In de werkplaatsvoorraad van dit project: nummer { $p0 } van de { $p1 } van zijn soort, genomen vóór die erna.
parts-stock-out = Niet in de werkplaatsvoorraad van dit project.
parts-stock-add = Aan de voorraad toevoegen
parts-stock-up = Omhoog
parts-stock-down = Omlaag
parts-stock-first = Het is al de eerste van zijn soort.
parts-stock-last = Het is al de laatste van zijn soort.
parts-stock-take-out = Eruit halen
parts-stock-title = De werkplaatsvoorraad van dit project
parts-stock-text = De draad-, adereindhuls-, merker- en wartelartikelen die de werkplaats houdt, in de volgorde van voorkeur. Sleep een rij in de lijst naar een andere plaats om de volgorde te wijzigen.
parts-stock-add-mine = Uit mijn artikelen toevoegen…
parts-stock-default = Deze voorraad gebruiken voor nieuwe projecten
parts-stock-cut = Afkorten op (de lengte waarop een stuk naar boven wordt afgerond) is een instelling van het project: Projecteigenschappen ▸ Werkplaats.
parts-stock-kept = { $p0 ->
    [one] Deze voorraad, { $p0 } artikel, wordt bewaard voor nieuwe projecten.
   *[other] Deze voorraad, { $p0 } artikelen in zijn volgorde, wordt bewaard voor nieuwe projecten.
    }
part-deleted = { $p0 } verwijderd. Ongedaan maken brengt het terug.
part-removed-own = { $p0 } is uit uw artikelbibliotheek verwijderd: volgende projecten bieden het niet meer aan.
part-removed-own-kept = { $p0 } is uit uw artikelbibliotheek verwijderd: volgende projecten bieden het niet meer aan. Dit project houdt zijn kopie voor { $p1 }.
part-hint-plc = Wat het artikel in de PLC is: een station (een CPU, een kopstation zoals een IM 152, een ventieleiland) heeft een rek; een module staat in een slot van het rek van een station. Een station waarvan de businterfaces de bus leiden is een CPU; een station dat alleen op de bus antwoordt is een kopstation.
part-plc-role = PLC-rol
part-plc-role-none = Geen
part-plc-kind = Kanaalsoort
part-plc-kind-none = Geen
part-plc-count = Kanalen
part-plc-kind2 = Tweede kanaalsoort
part-plc-count2 = Zijn kanalen
part-plc-data_type = Gegevenstype
part-plc-designation = Kanaalnaam
part-plc-pin_of = Penpatroon
part-plc-pin_of-hint = De pen van kanaal n: {"{"}n+1{"}"} geeft pennen 1, 2, 3 …; X2:{"{"}n+1{"}"} geeft X2:1, X2:2 …
part-plc-l_plus = Voeding L+-pen
part-plc-m = Voeding M-pen
part-plc-slots = Toegestane slots
part-plc-slots-hint = De slots waar de module in mag staan, als getallen en reeksen, zoals 1-8, 10; leeg: elk slot. Meeste modules is hoeveel modules een station neemt.
part-plc-supply-hint = Een lijst geeft een pen van elk, met komma's of spaties, zoals 1L+, 2L+; één M-pen dient elke L+-pen.
part-plc-supply-pins = Voedingspennen: { $p0 }
part-plc-max_modules = Meeste modules
part-plc-pins = Kanaal { $p0 } → pen { $p1 } … kanaal { $p2 } → pen { $p3 }
part-plc-pins-none = Pennen: zodra het aantal er is
part-plc-io-in = Neemt { $p0 } ingangsbits in
part-plc-io-out = Neemt { $p0 } uitgangsbits in
part-plc-io-none = Bits: zodra soort en aantal er zijn
card-paste-versions-title = De geplakte records gebruiken andere artikelversies
library-state-tip-current = De kopie in het project is de versie van de bibliotheek.
library-state-tip-update_available = De bibliotheek heeft een nieuwere versie van dit item. Bijwerken… toont eerst de wijzigingen; Toepassen voert ze door.
library-state-tip-deprecated = De bibliotheek biedt dit item niet meer aan; waar ze een opvolger noemt, gebruik die dan.
library-state-tip-source_missing = Dit item bestaat alleen in dit project: geen geïnstalleerde bibliotheek bevat het, zoals een geïmporteerd kader of een artikel dat in het project is gemaakt. Dat is geen fout, maar er is niets om het mee bij te werken.
library-state-tip-local_fork = De kopie in het project is hier gewijzigd en staat los van het bibliotheekitem; geen update vervangt haar.
libraries-update-count = { $p0 ->
        [one] 1 item bijwerken…
       *[other] { $p0 } items bijwerken…
    }
libraries-update-tip = Toont eerst de wijzigingen in het voorbeeld; Toepassen voert ze door.
part-pins-and-box-how = Waar de aansluitingen staan op de kast waarmee het apparaat wordt getekend
part-pins = Pennen
part-pin-add = Pen toevoegen
part-pin-name = Naam van pen { $p0 }
part-pin-role = Potentiaal van pen { $p0 }
part-pin-remove = Pen { $p0 } verwijderen
part-save-another = Opslaan en nog een maat
part-cable-cores = Aantal aders
part-cable-csa = Doorsnede (mm²)
part-cable-pe = Met beschermingsleiding (groen-geel)
part-cable-marking = Aders gemerkt met
part-cable-marking-hd308 = Kleuren (HD 308)
part-cable-marking-numbers = Nummers
part-cable-marking-own = Eigen indeling
part-cable-core = Ader
part-cable-colour = Kleur
part-cable-row = { $p0 } van ader { $p1 }
part-cable-shield = Afgeschermd
part-cable-diameter = Buitendiameter (mm)
part-cable-reads = Leest als
part-bus-part-name = Naam
part-cable-bus = Gemaakt voor de bus
part-cable-bus-none = Geen bus
part-plug-outlet = Kabeluitgang
part-plug-straight = rechte kabeluitgang
part-plug-angled = haakse kabeluitgang
part-plug-outlet-straight = Recht
part-plug-outlet-angled = Haaks (90°)
part-plug-socket = Met programmeeraansluiting
part-plug-resistor = Met afsluitweerstand (schakelbaar)
part-plug-description = { $p0 }-stekker
part-plug-with-socket = programmeeraansluiting
part-plug-with-resistor = schakelbare afsluitweerstand
part-section-name = Sectie { $p0 }
part-hint-accessories = Wat dit artikel meebrengt. Per stuk: voor elke klem of elk apparaat van dit artikel. Per klemmenstrook: eenmaal voor elke strook waarvan de klemmen dit artikel gebruiken, na de laatste. Per railgroep: de eindsteunen van een rij stroken op één rail, één ervoor en één na elke strook.
part-hint-variants = De kleuren of uitvoeringen van dit artikel die een apparaat kiest in Eigenschappen. Elk heeft een eigen artikelnummer, bestelnummer of omschrijving, zodat een lijst ze uit elkaar houdt.
part-hint-connections = De klemmen van de aansluitingen: de draden die elk opneemt. Een klem zonder bereik neemt elke draad.
part-hint-cabinet = De voorkant zoals de paneelpagina hem tekent, en de gaten die een deur of een plaat voor het artikel krijgt: X en Y zijn het midden van een gat, vanaf de linkeronderhoek van de voorkant.
part-hint-gland = De schroefdraad van de wartel en de kabels die hij afdicht: een kabel met een buitendiameter daartussen krijgt deze wartel als het de eerste zulke in de werkplaatsvoorraad is.
part-front-outline = Contour
part-front-outline-none = Geen: de vorm wordt getekend
part-front-outline-lines = { $p0 ->
    [one] geïmporteerd uit een DXF-tekening, 1 lijn
   *[other] geïmporteerd uit een DXF-tekening, { $p0 } lijnen
    }
part-front-import = Contour importeren…
part-front-remove = Verwijderen
part-front-preview = Voorbeeld van de voorkant en de gaten
part-table-add-accessories = Toebehoren toevoegen
part-table-add-variants = Variant toevoegen
part-table-add-mech-holes = Gat toevoegen
part-table-add-sections-enclosure-doors = Deur toevoegen
part-table-add-configured-components = Samenstellend artikel toevoegen
part-table-add-sections-fluid-ports = Poort toevoegen
part-table-remove = Rij { $p0 } verwijderen
part-table-empty = Nog geen

## Artikelen, de ene plek voor artikelen

parts-tab-part = Artikel
parts-tab-drawing = Tekening
parts-title = Artikelen
parts-new = Nieuw artikel
parts-like-this = Zoals dit
parts-own = Mijn eigen maken
parts-own-from = Mijn eigen ervan maken
parts-keep = In mijn artikelen bewaren
parts-remove = Verwijderen
parts-save = Opslaan
parts-save-keep = In mijn artikelen bewaren en opslaan
parts-save-use = Opslaan en gebruiken voor { $p0 }
parts-discard = Verwerpen
parts-back = Terug
parts-back-without-choosing = Terug zonder te kiezen
parts-back-without-it = Terug zonder artikel
parts-to-the-part = Naar het artikel
parts-done = Klaar
parts-save-place = Opslaan en plaatsen op { $p0 }
parts-strip-symbol-page = Een nieuw symbool voor pagina { $p0 }. Opgeslagen wordt het daar geplaatst.
parts-strip-symbol-like = Een symbool zoals dat van { $p0 }, voor pagina { $p1 }. Opgeslagen neemt het de plaats in van dat van { $p0 }.
parts-strip-symbol-piece = U wijzigt het symbool van het deel { $p0 } van { $p1 }. Het symbool waar het van komt blijft zoals het is; het uwe wordt bij uw artikelen bewaard en tekent alleen dit deel.
parts-strip-bus-symbol = U tekent uw eigen bussymbool voor { $p0 }. Het tekent elk apparaat van het artikel op een buspagina; artikelen zonder eigen symbool houden dat van het programma.
parts-strip-box = De kast van { $p0 } schikken. Sleep een aansluiting langs zijn rand of naar een andere rand; sleep een rand om de kast groter of kleiner te maken.
parts-symbol-lock-starter = Een startsymbool. Het komt met het programma en is op elke pc hetzelfde, dus het kan hier niet gewijzigd worden.
parts-symbol-lock-library = Een symbool van { $p0 }, die u niet kunt schrijven, dus het kan hier niet gewijzigd worden.
parts-symbol-lock-project = Een symbool dat alleen in dit project staat, uit geen bibliotheek die u hebt, dus het kan hier niet gewijzigd worden.
reason-no-drawing-in-hand = Er is geen tekening onder handen.
reason-no-symbol-saved = U hebt nog geen symbool opgeslagen.
reason-no-symbol-selected = Selecteer eerst een symbool op de pagina.
reason-piece-no-symbol = Nog geen symbool tekent dit deel: teken een nieuw.
reason-symbol-is-yours = Het symbool is al van u.
parts-strip-choosing = U kiest een artikel voor { $p0 }.
parts-strip-form = Een artikel voor { $p0 }.
parts-back-to-messages = Terug naar Meldingen
parts-strip-finding = Geopend vanuit een bevinding.
parts-strip-finding-page = Geopend vanuit een bevinding op pagina { $p0 }.
parts-new-title = Nieuw artikel
parts-what-is-it = Wat is het? Het antwoord bepaalt wat het blad vraagt, en verder wordt niets gevraagd.
parts-start-from = Of begin vanaf
parts-like-card = Een artikel zoals dit…
parts-like-card-line = Kies een artikel in de lijst: het uwe opent met alles behalve het nummer.
parts-like-which = Open in de lijst het artikel om vanaf te beginnen: het nieuwe artikel heeft alles wat het heeft, behalve het nummer.
parts-none-open = Open een artikel uit de lijst, of maak een nieuw.
parts-search-hint = Zoek op nummer, fabrikant, beschrijving, waarvoor het is
parts-search-name = Artikelen zoeken
parts-scope-all = Alle
parts-scope-project = In dit project
parts-scope-mine = Mijn artikelen
parts-scope-my-symbols = Mijn symbolen
parts-shelf-parts = Artikelen
parts-shelf-symbols = Symbolen
parts-group-drawn-lately = Pas getekend
parts-group-symbols = Symbolen
parts-symbols-count = { $p0 ->
    [one] { $p0 } symbool
   *[other] { $p0 } symbolen
    }
parts-new-symbol = Nieuw symbool
parts-piece-change = Wijzig het voor dit artikel…
parts-piece-draw-new = Teken een nieuw symbool…
parts-arrange = Schikken…
parts-drawn-as-head = Getekend als
parts-piece-mine-not-saved = van mij, niet opgeslagen
parts-use-for-all = Gebruik het voor elk artikel met dit deel…
parts-use-for-all-none = Geen ander artikel van u heeft dit deel.
parts-use-for-all-card = Het tekent dit deel van: { $p0 }.
parts-use-for-all-held = { $p0 ->
    [one] Dit project bevat er { $p0 }: de kopie volgt, in één stap om terug te draaien.
   *[other] Dit project bevat er { $p0 }: de kopieën volgen, in één stap om terug te draaien.
    }
parts-apply = Toepassen
parts-cancel = Annuleren
parts-drawn-with = Artikelen die ermee getekend worden
parts-drawn-with-none = Nog geen.
reason-no-piece-drawing = Teken eerst het symbool van een deel: Teken een nieuw symbool… of Wijzig het voor dit artikel… in de cel Symbool.
toast-symbol-used-for = Het symbool tekent nu dit deel van { $p0 } { $p0 ->
    [one] ander artikel
   *[other] andere artikelen
    }.
parts-strip-other-grid = Dit symbool staat op { $p0 }; pagina { $p1 } staat op { $p2 }: het wordt geplaatst als een kopie getekend op het raster van de pagina.
parts-piece-symbol-name = { $p0 } van { $p1 }
parts-bus-symbol-name = { $p0 } · bussymbool
parts-add-group = + { $p0 }
parts-new-part-with-it = Nieuw artikel dat ermee getekend wordt…
parts-filter-kind = Soort
parts-filter-kind-all = Elke soort
parts-filter-lacking = Mist iets
parts-count = { $p0 ->
    [one] { $p0 } artikel
   *[other] { $p0 } artikelen
    }
parts-used-by = { $p0 ->
    [one] nog { $p0 } gebruikt het
   *[other] nog { $p0 } gebruiken het
    }
parts-uses-it = { $p0 } gebruikt het
parts-changed = Gewijzigd, niet opgeslagen
parts-lives-mine = Van mij · in Mijn artikelen
parts-lives-starter = Startartikel
parts-lives-here = Alleen in dit project
parts-lives-new = Nieuw, niet opgeslagen
parts-lock-starter = Een startartikel. Het komt met het programma en is op elke pc hetzelfde, dus het kan hier niet gewijzigd worden.
parts-lock-library = Een artikel van een bibliotheek die u niet kunt schrijven, dus het kan hier niet gewijzigd worden.
parts-ready-for = Klaar voor
parts-ready-all = Niets wat een functie leest ontbreekt.
parts-schematic-picture = Hoe het artikel op een schemapagina wordt getekend
parts-no-schematic = Nog geen symbool tekent het.
parts-on-a-bus-page = Op een buspagina
parts-bus-symbol-picture = Hoe het artikel op een buspagina wordt getekend
parts-bus-symbol-lacks = Bussymbool: { $p0 } is niet getekend
parts-bus-symbol-extra = Bussymbool: { $p0 } is getekend, maar de netwerkpoorten hebben het niet meer
parts-make-own-bus-symbol = Mijn eigen ervan maken…
parts-use-program-bus-symbol = Weer het symbool van het programma gebruiken
parts-no-front = Geef het een breedte en een hoogte om zijn voorkant te zien.
parts-saved-mine = { $p0 } is opgeslagen in Mijn artikelen.
reason-no-parts-place = Artikelen is niet open.
reason-no-part-open = Er is geen artikel open in Artikelen.
reason-part-not-yours = Het artikel is niet van u: maak uw eigen ervan.
reason-part-no-bus-ports = Het artikel heeft geen netwerkpoorten: voeg ze eerst toe onder Netwerkpoorten.
reason-part-not-changed = Er is niets gewijzigd sinds het artikel werd geopend of opgeslagen.
reason-part-own-already = Het artikel is al van u: Zoals dit maakt er nog een zoals het.
reason-parts-unsaved = Het artikel heeft wijzigingen die niet zijn opgeslagen: sla ze eerst op of verwerp ze.
reason-parts-card-waits = Wat Opslaan in dit project wijzigt, wacht op de kaart naast het artikel: pas die eerst toe of annuleer hem.
reason-part-remove-used = { $p0 ->
    [one] { $p0 } apparaat van dit project gebruikt het artikel.
   *[other] { $p0 } apparaten van dit project gebruiken het artikel.
    }

## Het blad van Artikelen (doc 16a §6.27): de ene regel van een groep naast zijn kop, en de ruimte die vrij blijft
part-line-schematic = Hoe de pagina's het tekenen: zijn delen of zijn blok, en zijn aansluitingen
part-line-cabinet = Zijn maten, hoe het gemonteerd wordt, zijn voorkant en zijn gaten
part-line-connections = Aansluitingen met dezelfde klem staan op één rij; open een rij om één aansluiting te wijzigen
part-line-ports = Eén rij per interface van het datablad
part-line-plc = Wat het in de PLC is: een station met een rek, of een module in een sleuf
part-line-technical = De waarden waarmee het programma rekent
part-line-goes-with = Wat het meebrengt, en de vormen waarin het verkocht wordt
parts-free = Vrij houden
parts-free-top = boven
parts-free-bottom = onder
parts-free-left = links
parts-free-right = rechts
parts-count-line = { $p0 } · { $p1 } van mij · { $p2 } in dit project
# The count line of Parts opened with no project.
parts-count-line-alone = { $p0 } · { $p1 } van mij
parts-group-recent = Pas geopend
parts-chip-mine = van mij
parts-chip-starter = start
parts-chip-here = alleen dit project
parts-filter-maker = Fabrikant
parts-filter-maker-all = Elke fabrikant
parts-lock-read-only = Dit project is alleen-lezen geopend: de artikelen staan hier, en geen ervan wordt gewijzigd.

## Op het schema als tabellen (doc 07 §5.4): de delen van een artikel en de niveaus van een klem
parts-pieces-none = Nog geen deel: voeg toe waarmee het artikel getekend wordt, een spoel, een contact, een lamp, elk met zijn aansluitingen.
parts-piece-what = Wat het is
parts-piece-pins = Aansluitingen
parts-piece-symbol = Symbool
parts-piece-choose = Kies wat het is
parts-piece-what-of = Wat deel { $p0 } is
parts-piece-pins-of = Aansluitingen van deel { $p0 }
parts-piece-symbol-of = Symbool van deel { $p0 }
parts-piece-starter = { $p0 } · start
parts-piece-add = Een deel toevoegen
parts-levels = Niveaus
parts-levels-n = { $p0 } niveaus
parts-level = Niveau
parts-level-left = Linker klem
parts-level-right = Rechter klem
parts-level-joined = Binnen verbonden
parts-level-left-of = Linker klem van niveau { $p0 }
parts-level-right-of = Rechter klem van niveau { $p0 }
parts-levels-note = Elk niveau verbindt op zichzelf door. Het klemmenschema drukt per niveau een rij.
parts-filter-maker-name = Filteren op fabrikant
parts-strip-in-passing = Een nieuw artikel voor { $p0 }, de { $p1 }.
parts-new-plate = Nieuwe plaat…
parts-size-mm = { $p0 } mm
parts-size-mount-depth = { $p0 } · { $p1 } mm diep
parts-size-depth = { $p0 } mm diep
parts-connections-pins = Aansluitingen
parts-connections-open = De rij van aansluitingen { $p0 } openen
parts-connections-close = De rij van aansluitingen { $p0 } sluiten
parts-unit-mm = mm
parts-unit-mm2 = mm²
part-face-drawing = Volgens de tekening
part-face-drawn = { $p0 } · tekening
parts-import = Importeren
parts-scope-import = Deze import
parts-set-strip = Een import is een deur naar Artikelen: wat hij brengt staat in de lijst, niet opgeslagen, tot u het opslaat in Mijn artikelen.
parts-set-project = Dit project heeft kopieën van { $p0 } van deze artikelen; { $p1 } daarvan zouden veranderen.
parts-set-update = Ze bijwerken…
parts-set-group = Uit { $p0 }
parts-set-row = Rij { $p0 }
parts-set-rows = { $p0 ->
    [one] { $p0 } rij
   *[other] { $p0 } rijen
    }
parts-set-count-line = { $p0 } · { $p1 } missen iets wat een functie leest
parts-set-columns = Kolommen
parts-set-read = Wat er gelezen is
parts-set-columns-head = Welke kolom wat is
parts-set-columns-line = Eén keer toegewezen; wijzig een rij en de lijst links volgt
parts-set-saved-as-label = Opgeslagen als
parts-set-saved-as = De volgende tabel van deze bron wordt net zo gelezen.
parts-set-sizes = Breedte, Hoogte en Diepte gaan naar het veld met die naam dat de soort van de rij heeft: de buitenmaat van een kast, de maat van een montageplaat, de maat van een apparaat in de kast.
parts-set-all-read = Elke rij maakt een artikel.
parts-set-new = { $p0 } nieuw
parts-set-changed = { $p0 } gewijzigd
parts-set-same = { $p0 } hetzelfde
parts-set-refused = { $p0 } geweigerd
parts-set-chip-new = nieuw
parts-set-chip-changed = gewijzigd
parts-set-chip-same = hetzelfde
parts-set-chip-refused = geweigerd
parts-set-save = { $p0 } opslaan in Mijn artikelen
parts-set-as-it-will-be = zoals het wordt · { $p0 }
parts-set-ready-for = { $p0 } · Klaar voor
parts-set-refused-line = Rij { $p0 } is geweigerd: { $p1 }
parts-set-same-line = Rij { $p0 }, { $p1 }, is hetzelfde als het artikel dat u hebt: er wordt niets van geschreven.
parts-set-changed-line = Rij { $p0 }, { $p1 }, wijzigt: { $p2 }.
parts-set-like-unused = Rij { $p0 }, { $p1 }: Zoals wordt niet gebruikt, het artikel bestaat.
parts-set-starter = een startartikel: maak er uw eigen van met Mijn eigen maken van { $p0 }, en lees de tabel opnieuw.
parts-set-library = een artikel van { $p1 }: maak er uw eigen van met Mijn eigen maken van { $p0 }, en lees de tabel opnieuw.
parts-set-here = alleen in dit project: bewaar het eerst in Mijn artikelen, en lees de tabel opnieuw.
parts-set-changes = Wat wijzigt
parts-set-change = { $p0 } → { $p1 }
parts-set-repeats-line = Rij { $p0 }, { $p1 }, herhaalt rij { $p2 }.
parts-set-no-number = hij heeft geen artikelnummer. Geef hem er een in het bestand en lees het opnieuw, of laat hem weg.
parts-set-no-maker = hij heeft geen fabrikant. Geef hem er een in het bestand en lees het opnieuw, of laat hem weg.
parts-set-error-cell = de cel van kolom { $p0 } bevat de fout { $p1 }.
parts-set-maker-ambiguous = zijn fabrikant { $p0 } noemt meer fabrikanten: { $p1 }.
parts-set-maker-letters = zijn fabrikant { $p0 } heeft geen letters of cijfers.
parts-set-number-letters = zijn artikelnummer { $p0 } heeft geen letters of cijfers.
parts-set-repeats-other = hij heeft de fabrikant en het nummer van rij { $p0 }, met andere waarden.
parts-set-no-version = het artikel in Mijn artikelen heeft geen volgende versie om het te nemen.
parts-set-unread = hij leest niet als een artikel.
parts-set-field = in kolom { $p0 }: { $p1 }
parts-set-like = zijn kolom Zoals noemt { $p0 }, wat geen artikel van Mijn artikelen of van een bibliotheek is.
parts-export = Exporteren
parts-exported = { $p0 ->
    [one] { $p0 } artikel geschreven naar { $p1 }.
   *[other] { $p0 } artikelen geschreven naar { $p1 }.
    }
parts-exported-saved = { $p0 ->
    [one] { $p0 } artikel geschreven naar { $p1 } zoals het is opgeslagen: wat getypt en niet opgeslagen is, staat er niet in.
   *[other] { $p0 } artikelen geschreven naar { $p1 } zoals ze zijn opgeslagen: wat getypt en niet opgeslagen is, staat er niet in.
    }
parts-many-title = { $p0 ->
    [one] { $p0 } artikel
   *[other] { $p0 } artikelen
    }
parts-many-line = { $p0 } artikelen geselecteerd. Een getypte waarde gaat naar alle; een veld waarin ze verschillende waarden hebben leest gemengd, en Opslaan laat het zoals elk het heeft.
parts-many-mixed = gemengd
parts-many-refused = { $p0 } is niet opgeslagen: { $p1 }
parts-many-saved = { $p0 ->
    [one] { $p0 } artikel opgeslagen in Mijn artikelen.
   *[other] { $p0 } artikelen opgeslagen in Mijn artikelen.
    }
parts-many-parts = De geselecteerde artikelen
parts-many-own = Mijn eigen maken van { $p0 }
parts-many-cells = Ze hebben dezelfde aansluitingen. Een cel waarin ze verschillen is leeg en blijft zoals elk het heeft.
parts-many-cell = Aansluiting { $p0 } · { $p1 }
