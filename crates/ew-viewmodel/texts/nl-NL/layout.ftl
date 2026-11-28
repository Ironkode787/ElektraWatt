# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: GPL-3.0-or-later

# The texts of the window in nl-NL: the cabinet layout: racks, arranging, enclosures and stations. Every key of the
# en-GB file of the same name, with the same arguments; a message missing here falls back to en-GB. Product and file
# names, rule IDs, command names, key caps, identifiers and units are not translated.

layout-not-json = geen JSON: { $p0 }
layout-not-object = een werkruimtebestand is een JSON-object
layout-schema = `$schema` is niet ew.workspace/1
layout-name-missing = `name` ontbreekt
layout-root-missing = `root` ontbreekt
layout-no-page-view = de paginaweergave ontbreekt
layout-too-deep = de indeling is te diep genest
layout-too-many = de indeling heeft te veel knooppunten
layout-pane-twice = het paneel `{ $p0 }` komt twee keer voor
layout-node-object = een knooppunt is een JSON-object
layout-unknown-member = onbekend veld `{ $p0 }`
layout-pane-name = `pane` is een naam
layout-unknown-pane = onbekend paneel `{ $p0 }`
layout-tabs-list = `tabs` is een lijst
layout-tabs-empty = `tabs` is leeg
layout-split-dir = `split` is horizontal of vertical
layout-children-list = `children` is een lijst
layout-children-empty = `children` is leeg
layout-child-object = een kind is een JSON-object
layout-share = `share` is een positief geheel getal
layout-node-missing = `node` ontbreekt
layout-node-kind = een knooppunt heeft `pane`, `tabs` of `split`
rack-max-modules = Het neemt ten hoogste { $p0 } modules.
rack-slot = Slot
rack-module = Module
rack-part = Artikel
rack-free = vrij
rack-blank = blindplaat
rack-empty = leeg
rack-position = Positie { $p0 }
rack-order-code = { $p0 }: { $p1 }
rack-bad-slot = "{ $p0 }" is geen slot: typ een getal, of rek en slot als 1.3
rack-slot-taken = Slot { $p0 } is al bezet door { $p1 }: kies een vrij slot
rack-slot-not-free = Slot { $p0 } is geen vrij slot van het station: kies een vrij slot
module-safety-title = Veiligheidsgegevens
channel-settings-title = Modus en veiligheid
layout-on-rail = { $p0 } zit op { $p1 } en wordt op een rail { $p2 } gemonteerd, dus het schuift alleen langs een rail: sleep het langs { $p1 } of naar een andere rail.
layout-wrong-rail = { $p0 } wordt op een rail { $p2 } gemonteerd en past niet op { $p1 }: zet het op een rail { $p2 }.
layout-off-rail = { $p0 } wordt op een rail { $p1 } gemonteerd; zet het op een rail.
layout-wrong-turn = { $p0 } draait een plaatsing op de plaat; { $p1 } keert er een om op een rail.
layout-blocked = Geblokkeerd door { $p0 }
layout-rail-end = Aan het einde van de rail
layout-live-rail = { $p0 }, { $p1 } op { $p2 } mm, { $p3 }
layout-live-plate = { $p0 }, plaat op { $p1 }, { $p2 } mm
layout-findings = { $p0 ->
        [one] 1 bevinding
       *[other] { $p0 } bevindingen
    }
layout-findings-none = geen bevindingen
layout-mode-free = Vrij
layout-mode-push = Duwen
layout-mode-swap = Wisselen
layout-grid-5 = 5 mm
layout-grid-1 = 1 mm
layout-options-mode = Plaatsingsmodus
layout-options-grid = Rasterstap
enclosure-new-title = Kast maken
enclosure-new-ok = Maken
enclosure-new-location = Nieuwe locatie
auto-fill-title = Rail automatisch vullen
auto-fill-ok = Vullen
auto-fill-order-schematic = Schema
auto-fill-order-dt = DT
auto-fill-devices-all = Aangevinkt: de apparaten van de kast en die waarvan het artikel op een rail gaat; sensoren, cilinders, motoren en ventieleilanden zonder locatie niet. Een aangevinkt apparaat zonder locatie krijgt die van de kast.
auto-fill-devices-marked = { $p0 ->
        [one] Aangevinkt: het apparaat dat in Ongeplaatste apparaten is gemarkeerd
       *[other] Aangevinkt: de { $p0 } apparaten die in Ongeplaatste apparaten zijn gemarkeerd
    }.
auto-fill-cause-all-placed = Elk apparaat van { $p0 } is al geplaatst.
auto-fill-cause-none = Nog geen apparaat heeft de locatie { $p0 }: geef apparaten deze locatie in Eigenschappen, onder Structuur.
layout-show-item-numbers = Positienummers tonen
enclosure-new-no-part = Er staat geen kastartikel in het project of in een geïnstalleerde bibliotheek; typ een artikelnummer.
enclosure-draw-here = { $p0 } van { $p1 } op deze pagina tekenen
enclosure-draw-new-page = { $p0 } van { $p1 } op een nieuwe paneellayoutpagina tekenen
enclosure-create-new-page = Maken en op een nieuwe paneellayoutpagina tekenen
enclosure-create-only = Alleen maken
enclosure-change-part = Kast wijzigen…
enclosure-part-card-title = { $p0 } wordt { $p1 }
enclosure-part-plate-kept = { $p0 } noemt geen montageplaat: de plaat die er is blijft, op haar maat.
enclosure-part-kept-from = Wat op elk vlak staat, houdt zijn plaats gemeten vanaf het nulpunt: { $p0 }.
enclosure-part-kept-datum = Wat op elk vlak staat, houdt zijn plaats gemeten vanaf de nulpunthoek van dat vlak.
enclosure-not-layout = Deze pagina is geen paneellayoutpagina. De kast wordt getekend op een nieuwe paneellayoutpagina erna; Alleen maken maakt de kast zonder haar te tekenen.
enclosure-has-one = { $p1 } heeft de kast { $p0 } al, artikel { $p2 }: de knop tekent haar zoals ze is.
enclosure-layout-title = Paneellayout { $p0 }
auto-fill-devices = { $p0 ->
        [one] 1 apparaat
       *[other] { $p0 } apparaten
    }
rack-new-title = Nieuw station
rack-create = Maken
rack-kind = Wat het is
rack-kind-cpu = PLC (CPU)
rack-kind-remote = Decentrale I/O
rack-kind-valve-terminal = Ventieleiland
rack-kind-compact = Compact apparaat
rack-choose-head = Kies het kopartikel van het station
rack-new-part = Nieuw artikel…
rack-plc = De PLC waar het bij hoort (een decentraal station: de PLC die het bedient)
rack-new-plc = Nieuwe PLC
rack-gives = Geeft
rack-addresses = Adressen
rack-new-module = nieuw
rack-no-part-fits = Geen moduleartikel mag in dit slot
rack-addresses-moved = De adressen van de modules zijn met hun slots meegeschoven.
rack-next-overview = De overzichtspagina maken
rack-next-bus = Op een buspagina zetten…
rack-next-fluid = Op een fluïdumpagina tekenen…
# Elke kant van een station wordt op een pagina van zijn soort getekend, vanuit zijn eigen regel onder Te doen.
rack-next-electrical = Op een elektrische pagina tekenen…
rack-draw-here = Op pagina { $p0 } tekenen
rack-put-here = Op pagina { $p0 } zetten
rack-draw-new-electrical = Op een nieuwe elektrische pagina tekenen
rack-put-new-bus = Op een nieuwe buspagina zetten
rack-draw-new-fluid = Op een nieuwe fluïdumpagina tekenen
rack-todo-electrical-side = De elektrische kant van { $p0 }, de voeding en uitgangen als kader, staat nog op geen elektrische pagina.
# Het kenmerk van een station, of van een kaart, gewijzigd in het stationsoverzicht.
rack-rename-station = Kenmerk { $p0 }: klik om het te wijzigen
rack-rename-module = Het kenmerk van { $p0 } wijzigen
rack-draft-tag = Kenmerk
rack-draft-tag-field = Kenmerk van het nieuwe station
rack-draft-tag-page = volgt uit de pagina waarop het getekend wordt
rack-draft-tag-tip = Klik om het een eigen kenmerk te geven; Maken geeft het dat kenmerk vastgezet. Leeg gelaten geeft de pagina waarop het getekend wordt het zijn kenmerk.
rack-tag-chip = Kenmerk { $p0 }
rack-rename-tip = Klik om het kenmerk te wijzigen (F2)
rack-rename-field = Kenmerk van { $p0 }
rack-rename-plc = PLC { $p0 }: klik om de naam te wijzigen
rack-rename-plc-tip = Klik om de naam van de PLC te wijzigen
rack-rename-plc-field = Naam van PLC { $p0 }
rack-rename-plc-taken = { $p0 } is al de naam van een andere PLC van het project: typ een andere naam, of Esc om deze te houden.
rack-rename-plc-bad = { $p0 } kan geen PLC-naam zijn: typ letters, cijfers en - of _, zonder spaties.
rack-rename-taken = { $p0 } is al het kenmerk van een ander apparaat: typ een ander kenmerk, of Esc om dit te houden.
rack-not-described = Het artikel beschrijft zijn modules niet: voeg ze toe zoals ze gebouwd worden.
choose-station-title = Welk station neemt { $p0 }?
choose-station-text = De module is getekend op een pagina zonder station. Kies het station in wiens rek hij komt; Annuleren laat hem wachten onder Modules zonder station.
choose-station-button = { $p0 }
rack-kind-cpu-tip = Een PLC met zijn CPU, die de bus leidt
rack-kind-remote-tip = Decentraal I/O-station (interfacemodule op een bus)
rack-kind-valve-terminal-tip = Ventieleiland met busknooppunt
rack-kind-compact-tip = Compact apparaat met eigen kanalen
rack-no-head-part = Er staat geen artikel van deze soort in de bibliotheken. Nieuw artikel… maakt er een.
rack-use-device = { $p0 } gebruiken, dat al op de bus staat
rack-bus-page-title = Bus
rack-no-bus-interface = Het artikel van { $p0 } heeft geen businterface en hoort dus niet op een buspagina: geef het een artikel met een busknooppunt
rack-bus-drawn = Elke businterface van { $p0 } staat al op pagina { $p1 }
rack-next-draw-on = Tekenen op pagina { $p0 } { $p1 }
rack-draw-new-page = Nieuwe pagina…
rack-draw-page-title = PLC
rack-card-page-title = PLC { $p0 }
rack-page-choice = { $p0 } { $p1 }
put-station-title = Station op deze pagina zetten
put-station-here = { $p0 } hier zetten
put-station-new = Een nieuw station

# A station's view: its rack as a tab beside the page tabs (doc 10 §6).
station-tab-title = { $p0 } · rek
station-discard-title = Het nieuwe station sluiten?
station-discard-text = Het station is nog niet gemaakt. Als u het tabblad sluit, gaan het gekozen kopartikel en de modules verloren.
station-discard-close = Sluiten zonder te maken
rack-move-to = Naar pagina { $p0 } { $p1 }
rack-move-to-new-page = Naar een nieuwe pagina
rack-valve-on-slot = Een ventiel hoort op een ventielplaats, niet in een slot
rack-module-on-position = Een module hoort in een slot, niet op een ventielplaats
rack-part-not-in-slot = Niet toegestaan in slot { $p0 }: het hoort in slot { $p1 }
rack-part-no-module = { $p0 } is de kop van een station, geen module: in een slot hoort een module
rack-no-free-position = Elke ventielplaats van { $p0 } is gevuld: maak er eerst een leeg
rack-full = Elk slot van het station is bezet: haal eerst een module weg
rack-takes-no-modules = Dit station neemt geen modules op: zijn artikel heeft er geen slots voor
rack-part-no-functions = Niet aangeboden: niets tekent dit artikel, want het heeft geen pennen. Geef het in Artikelen onder PLC een Penpatroon of zijn voedingspennen en sla het op
rack-todo = Te doen
rack-todo-none = Niets te doen: de slots voldoen aan de regels van het station en elke module is getekend.
rack-todo-create = Maken maakt het station met deze modules. Daarvoor wordt niets geschreven.
rack-todo-show = Tonen
rack-todo-no-part = { $p0 } heeft geen artikel.
rack-todo-undrawn = Nog op geen elektrische pagina: { $p0 }. Een kenmerk krijgt zijn nummer als het op een pagina getekend wordt.
rack-todo-card-rest = De rest van { $p0 } staat nog op geen pagina: de module is breder dan één pagina.
rack-card-rest = De rest van { $p0 } past niet op deze pagina: Rek… toont de regel onder Te doen, die hem op een pagina naar keuze tekent.
rack-card-no-room = Pagina { $p1 } heeft geen plaats voor { $p0 }: Rek… toont de regel onder Te doen, die hem op een andere pagina tekent.
rack-tag-unnumbered = { $p0 } krijgt zijn nummer als het op een pagina getekend wordt: de regel ervan onder Te doen tekent het.
rack-todo-bus = De buskant van { $p0 }, de buspoorten, staat nog op geen buspagina.
rack-todo-fluid = De pneumatische kant van { $p0 }, de ventielen op hun posities, staat nog op geen fluïdumpagina.
rack-todo-valves = Ventielen van { $p0 } staan niet getekend op hun posities, of de voedingspoorten ontbreken.
rack-todo-overview = Een overzichtspagina toont zijn posities en ventielen.
rack-group-cpu = CPU's
rack-group-interface = Interfacemodules
rack-group-valve-terminal = Ventieleilanden
rack-group-di = Digitale ingangen
rack-group-do = Digitale uitgangen
rack-group-analogue = Analoog
rack-group-other = Overige modules
rack-group-valve = Ventielen
rack-catalogue = Catalogus
rack-catalogue-filter = De catalogus filteren
rack-catalogue-hint = Dubbelklik op een artikel om het in het gekozen slot te zetten, of sleep het naar een slot.
rack-catalogue-heads = Kies de kop van het station: dubbelklik op een artikel.
rack-catalogue-empty = Geen artikel uit de bibliotheken past hier. Nieuw artikel… maakt er een.
rack-catalogue-no-hit = Geen artikel past bij het filter.
rack-belongs-to = Hoort bij PLC: { $p0 }
rack-drawn-on = Getekend op
rack-page = Pagina { $p0 }
rack-no-part = Geen artikel
rack-choose-part = Artikel kiezen…
rack-draw-own-pages = Elke kaart op een eigen pagina tekenen
rack-draw-one-page = Alles op één pagina…
rack-draw-on-page = Op een pagina tekenen…
rack-head-row = Kop
rack-word-cpu = CPU
rack-word-interface = IM
rack-word-valve-terminal = VE
rack-word-valve = Ventiel
rack-word-blank = Blind
rack-free-slot = Vrij slot { $p0 }
rack-free-position = Lege positie { $p0 }
rack-place-slot = Slot { $p0 }, { $p1 }
rack-place-position = Positie { $p0 }, { $p1 }
rack-place-head = Kop, { $p0 }
rack-channels-of = Kanalen van { $p0 }
rack-channels-none = { $p0 } heeft geen kanalen.
rack-channels-select = Kies een module om zijn kanalen te zien.
rack-channels = Kanalen
rack-channel = Kanaal
rack-channel-address = Adres
rack-channel-symbol = Symboolnaam
rack-channel-wired = Bedraad naar
rack-channel-goto = Naar { $p0 }
rack-channel-function = Functietekst
rack-channel-symbol-of = Symboolnaam van { $p0 }
rack-channel-function-of = Functietekst van { $p0 }
rack-channel-edit-tip = Klik om hem te typen: Enter neemt hem over, Tab gaat door naar het volgende kanaal, Esc laat hem staan.
rack-channel-function-tip = De functietekst van het apparaat dat erop bedraad is. Klik om hem te typen: Enter neemt hem over, Tab gaat door naar het volgende kanaal, Esc laat hem staan.
rack-channel-bad = "{ $p0 }" kan hier niet worden overgenomen.
rack-put-blank = Blindplaat
rack-draw-on-new-page = Op een nieuwe pagina tekenen
arrange-lead = Schikken en routeren: { $p0 }. { $p1 }; { $p2 }. Er is nog niets veranderd.
arrange-symbols = { $p0 ->
        [one] 1 symbool verschuift
       *[other] { $p0 } symbolen verschuiven
    }
arrange-across = { $p0 }, waarvan { $p1 } over het blad
arrange-wires = { $p0 ->
        [one] 1 draad wordt gerouteerd
       *[other] { $p0 } draden worden gerouteerd
    }
arrange-yours = { $p0 }, waarvan { $p1 } door u getekend
arrange-figures = { $p0 }, { $p1 }, { $p2 }, { $p3 }, { $p4 }
arrange-crossings = { $p0 ->
        [one] 1 kruising
       *[other] { $p0 } kruisingen
    }
arrange-bends = { $p0 ->
        [one] 1 knik
       *[other] { $p0 } knikken
    }
arrange-straight = { $p0 ->
        [one] 1 draad recht
       *[other] { $p0 } draden recht
    }
arrange-through = { $p0 ->
        [one] 1 draad door een tekst
       *[other] { $p0 } draden door een tekst
    }
arrange-faults = { $p0 ->
        [one] 1 fout
       *[other] { $p0 } fouten
    }
arrange-now = Nu: { $p0 }
arrange-then = Daarna: { $p0 }
arrange-heading-parts = Symbolen
arrange-heading-wires = Draden die u zelf tekende
arrange-heading-left = Blijft zoals het is
arrange-right = { $p0 ->
        [one] 1 stap naar rechts
       *[other] { $p0 } stappen naar rechts
    }
arrange-left = { $p0 ->
        [one] 1 stap naar links
       *[other] { $p0 } stappen naar links
    }
arrange-up = { $p0 ->
        [one] 1 stap omhoog
       *[other] { $p0 } stappen omhoog
    }
arrange-down = { $p0 ->
        [one] 1 stap omlaag
       *[other] { $p0 } stappen omlaag
    }
arrange-a-symbol = een symbool zonder kenmerk
arrange-unticked = blijft staan (u hebt het uitgevinkt)
arrange-his = krijgt een nieuwe route
arrange-his-unticked = houdt zijn route (u hebt hem uitgevinkt)
arrange-kept = op zijn plaats gehouden
arrange-place-up = boven zijn aansluiting
arrange-place-down = onder zijn aansluiting
arrange-place-left = links van zijn aansluiting
arrange-place-right = rechts van zijn aansluiting
arrange-in-way-room = geen plaats { $p0 } heeft er ruimte voor; blijft staan
arrange-in-way-part = { $p1 } staat op zijn plaats { $p0 }; blijft staan
arrange-in-way-edge = zijn plaats { $p0 } ligt buiten het tekenvlak; blijft staan
arrange-in-way-text-of = op zijn plaats { $p0 } zou een draad door een tekst van { $p1 } lopen; blijft staan
arrange-in-way-text = op zijn plaats { $p0 } zou een draad door een tekst lopen; blijft staan
arrange-in-way-wire = op zijn plaats { $p0 } zou een draad door een aansluitpunt of langs een andere draad lopen; blijft staan
arrange-in-way-texts = op zijn plaats { $p0 } zou een tekst op een andere tekst of een symbool liggen; blijft staan
arrange-in-way-crossings = op zijn plaats { $p0 } zouden de draden meer kruisen dan nu; blijft staan
arrange-in-way-no-gain = op zijn plaats { $p0 } zou de pagina niet beter worden; blijft staan
arrange-hint = Een regel uitvinken plant de pagina opnieuw met dat symbool waar het staat, of die draad zoals hij loopt: wat u ziet, doet Toepassen.
arrange-hold-b = Houd B ingedrukt: de pagina zoals hij nu is.
arrange-moved = { $p0 ->
        [one] 1 symbool verschoven
       *[other] { $p0 } symbolen verschoven
    }
arrange-routed = { $p0 ->
        [one] 1 draad gerouteerd
       *[other] { $p0 } draden gerouteerd
    }
arrange-blocked = Niets te schikken: { $p0 }
arrange-blocked-more = { $p0 ->
        [one] En nog 1: wijs hier aan, of zie Notificaties
       *[other] En nog { $p0 }: wijs hier aan, of zie Notificaties
    }.
arrange-blocked-symbol = { $p0 }: { $p1 }.
arrange-blocked-unrouted = { $p0 } heeft geen route: hij is als gestreepte lijn getekend.
arrange-held-order = De strook bij { $p0 } blijft zoals hij staat: zijn klemmen zijn in een andere volgorde op de kaart aangesloten dan ze staan.
arrange-held-room = De strook bij { $p0 } blijft zoals hij staat: de kanalen van zijn kaart laten te weinig ruimte voor de klemmen ertussen.
arrange-held-part = De strook bij { $p0 } blijft zoals hij staat: { $p1 } staat waar hij zou komen.
arrange-held-sheet = De strook bij { $p0 } blijft zoals hij staat: geopend zou hij buiten het tekenvlak komen.
arrange-held-worse = De strook bij { $p0 } blijft zoals hij staat: geopend in zijn volgorde zouden zijn draden meer kruisen dan nu. Kijk naar de klemmen die buiten die volgorde zijn aangesloten.
arrange-blocked-faults = { $p0 ->
        [one] 1 fout blijft op de pagina; Project controleren noemt hem
       *[other] { $p0 } fouten blijven op de pagina; Project controleren noemt ze
    }.

# Surface… (doc 12 §3.8)
surface-chooser-title = Welk vlak?
feature-form-title = Een gat toevoegen
feature-form-title-of = Een gat toevoegen aan { $p0 }
feature-form-add = Toevoegen
feature-form-kind = Soort
feature-form-size = Grootte
feature-form-x = X (mm)
feature-form-y = Y (mm)
feature-form-purpose = Voor
feature-form-xy = Geef X en Y in millimeter vanaf de hoek waarvandaan het vlak gemeten wordt.
feature-form-size-missing = Geef de grootte van het gat: een diameter voor een gat, M3 tot M8 voor een bevestiging, 16, 22 of 30 voor een uitsparing, M12 tot M63 voor een wartel, breedte × hoogte voor een rechthoek.
reason-no-hole-table = Het project heeft nog geen gatentabel: voeg er een toe met Rapportblok… (Gatentabel).
reason-no-glands = Geen kabelwartel van deze kast wacht op plaatsing.
surface-chooser-title-of = Welk vlak van { $p0 }?
surface-chooser-text = Een vlak wordt op een eigen pagina getekend, gezien vanaf de kant waar eraan gewerkt wordt.
surface-chooser-field = Vlak
surface-chooser-size = Afmetingen
surface-chooser-page-head = Pagina
surface-chooser-new-page = Tekenen op een nieuwe pagina
surface-chooser-go = Naar pagina { $p0 }
surface-chooser-hinged = scharnier { $p0 }
surface-chooser-page = pagina { $p0 }
surface-chooser-new = nieuwe pagina
surface-chooser-line = Het wordt getekend op een nieuwe pagina na de pagina van de montageplaat, op de schaal die past, met zijn legendapagina. Eén stap ongedaan te maken.
surface-chooser-shown = Pagina { $p0 } toont het.
toast-surface-drawn = { $p0 } is getekend op pagina { $p1 }, met zijn legendapagina.
canvas-surface-entry = Vlak: { $p0 }
reason-surface-no-plate-page = De montageplaat van de kast staat nog op geen pagina: teken die eerst met Kast….
error-no-surface = Geen vlak van de kast heet “{ $p0 }”.
layout-cutout = { $p0 } komt in een uitsparing van een deur of een wand: plaats het op de pagina van de deur.
layout-open-door = Deur openen
reason-layout-cutout = Het artikel komt in een uitsparing van een deur of een wand.
hud-layout-door = Deur
# De pagina van een deur waar de controle van de gesloten deur niet kan oordelen of de deur sluit (doc 12 §7).
hud-door-no-room = De gesloten deur van { $p0 } wordt niet beoordeeld: het artikel geeft geen diepte tussen de plaat en de deur
hud-door-unjudged = { $p0 ->
        [one] De gesloten deur wordt niet beoordeeld voor 1 apparaat zonder diepte: { $p1 }
       *[other] De gesloten deur wordt niet beoordeeld voor { $p0 } apparaten zonder diepte, eerst { $p1 }
    }
hud-layout-cutout = { $p0 ->
        [one] 1 apparaat komt in een uitsparing
       *[other] { $p0 } apparaten komen in een uitsparing
    }
hud-layout-bottom = Onderkant
hud-layout-glands = { $p0 ->
        [one] 1 wartel te plaatsen
       *[other] { $p0 } wartels te plaatsen
    }
drilling-title = Boortekeningen exporteren
drilling-export = Exporteren
drilling-template = Boormal om op ware grootte te printen (PDF)
drilling-dxf = DXF voor een machine
drilling-paper = Papier
drilling-dir = Map
drilling-surfaces = Vlakken met gaten
drilling-surface = Vlak
drilling-tick = Exporteren
drilling-row = { $p0 } · pagina { $p1 } · { $p2 } · { $p3 } · { $p4 }
drilling-written-both = een boormal en een DXF
drilling-written-template = een boormal
drilling-written-dxf = een DXF
drilling-written-each = elk van { $p0 }
drilling-holes = { $p0 ->
        [one] 1 gat
       *[other] { $p0 } gaten
    }
drilling-sheets = { $p0 ->
        [one] 1 blad
       *[other] { $p0 } bladen
    }
drilling-nothing = Kies de boormal, de DXF of allebei.
drilling-none-ticked = Vink ten minste één vlak aan.
reason-no-holes = Geen vlak van dit project heeft al gaten.
choice-a4 = A4
choice-a3 = A3
canvas-keep-as-hole = Als gat houden
