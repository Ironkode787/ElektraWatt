# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: GPL-3.0-or-later

# The texts of the window in nl-NL: import, export, print, external changes and the exchange with other programs.
# Every key of the en-GB file of the same name, with the same arguments; a message missing here falls
# back to en-GB. Product and file names, rule IDs, command names, key caps, identifiers and units are not translated.

external-none = geen (maak er een met Project ▸ Inrichting ▸ Externe verwijzing…)
external-none-key = geen (maak er een met Project ▸ Inrichting ▸ Externe verwijzing…, { $p0 })
report-source-sql = SQL-rapport { $p0 }
# What gives a report block of each page kind its rows, said when the block makes no page yet.
report-rows-toc = De pagina's van het project geven het rijen
report-rows-structure-overview = Een structuuraanduiding of een apparaatkenmerk geeft het rijen
report-rows-parts-summary = Een apparaat met een artikel geeft het rijen
report-rows-parts-list = Een apparaat met een artikel geeft het rijen
report-rows-device-list = Een apparaat geeft het rijen
report-rows-terminal-diagram = Een klemmenstrook met klemmen geeft het rijen
report-rows-terminal-overview = Een klemmenstrook geeft het rijen
report-rows-cable-overview-diagrams = Een kabel geeft het rijen
report-rows-wire-list = Een draad geeft het rijen
report-rows-plc-overview = Een PLC-kaart met kanalen geeft het rijen
report-rows-io-list = Een PLC-kanaal geeft het rijen
report-rows-plc-card-list = Een PLC-kaart geeft het rijen
report-rows-network-address-list = Een buspoort met een adres geeft het rijen
report-rows-bus-overview = Een busstation op een net geeft het rijen
report-rows-bus-cable-list = Een buskabel geeft het rijen
report-rows-iolink-port-list = Een poort van een IO-Link-master geeft het rijen
report-rows-safety-io-list = Een veiligheidskanaal geeft het rijen
report-rows-potential-overview = Een potentiaal geeft het rijen
report-rows-revision-overview = Een revisie geeft het rijen
report-rows-open-items = Een openstaand punt van de controle, zoals een waarde die nog moet worden bepaald, geeft het rijen
report-rows-numbering-report = Een vastgezet kenmerk of een vastgezet draadnummer geeft het rijen
report-rows-enclosure-legend = Een apparaat op een montagevlak geeft het rijen
report-rows-check-report = Een bevinding van de controle geeft het rijen
report-rows-tube-list = Een slang geeft het rijen
report-rows-tube-cut-list = Een slang geeft het rijen
report-rows-tube-labels = Een slang geeft het rijen
report-rows-valve-terminal-config = Een ventieleiland geeft het rijen: plaats er een via Apparaat ▸ Ventieleilanden
report-rows-medium-overview = Een medium op een slang geeft het rijen
# The export toast names its pages by kind.
export-pages-drawing = { $p0 ->
        [one] 1 tekeningpagina
       *[other] { $p0 } tekeningpagina's
    }
export-pages-divider = { $p0 ->
        [one] 1 scheidingspagina
       *[other] { $p0 } scheidingspagina's
    }
export-pages-report = { $p0 ->
        [one] 1 rapportpagina
       *[other] { $p0 } rapportpagina's
    }
print-title = Afdrukken
print-range = Bereik
print-range-all = Alles
print-range-current = Huidige pagina
print-range-sections = Secties
print-range-pages = Pagina's
print-section-row = Sectie { $p0 }: { $p1 }
print-paper = Papier
print-paper-sheet = Bladformaat
print-fit = Passend schalen
print-mono = Zwart-wit
print-preview = Voorbeeld
print-page-of = { $p0 } van { $p1 }
print-previous = Vorige
print-next = Volgende
print-first-page = Dit is de eerste pagina; Volgende gaat verder
print-last-page = Dit is de laatste pagina; Vorige gaat terug
print-button = Afdrukken…
print-viewer-button = In PDF-viewer openen
print-empty-title = Geen pagina's om af te drukken
print-empty-text = Vink een sectie aan of typ de pagina's die u wilt afdrukken
print-building = Afdrukset wordt opgebouwd… { $p0 } %
print-writing = Afdrukset wordt geschreven…
print-failed = De afdrukset kon niet worden opgebouwd: { $p0 }
print-pages-error = De pagina's konden niet worden gelezen. Typ folio's of bereiken, gescheiden door komma's.
print-unavailable = Afdrukken is in dit venster niet beschikbaar. Exporteer een PDF en druk die af vanuit uw PDF-viewer.
plc-export-title = Naar PLC-software exporteren
plc-count-stations = { $p0 ->
        [one] 1 station
       *[other] { $p0 } stations
    }
plc-count-modules = { $p0 ->
        [one] 1 module
       *[other] { $p0 } modules
    }
plc-count-channels = { $p0 ->
        [one] 1 kanaal
       *[other] { $p0 } kanalen
    }
plc-count-symbols = { $p0 ->
        [one] 1 symbool
       *[other] { $p0 } symbolen
    }
plc-import-title = { $p0 } importeren
plc-import-summary = { $p0 } gewijzigd, { $p1 } in conflict, { $p2 } verwijderd, { $p3 } geparkeerd. Er verandert niets tot u toepast.
plc-import-tab-changed = Gewijzigd ({ $p0 })
plc-import-tab-conflicts = Conflicten ({ $p0 })
plc-import-tab-removed = Verwijderd ({ $p0 })
plc-import-tab-parking = Geparkeerd ({ $p0 })
plc-import-keep-ours = Die van het project houden
plc-import-take-theirs = Die van het bestand nemen
plc-import-ambiguous = meerdere objecten passen
plc-import-unmatched = geen object past
plc-import-create = Ongeplaatst aanmaken
plc-import-complete = Het bestand is volledig: wat erin ontbreekt, wordt verwijderd
plc-import-sides = project: { $p0 } → bestand: { $p1 }
plc-import-no-value = niets
plc-import-empty = Niets op dit tabblad.
plc-import-row = { $p0 } · { $p1 }
plc-import-object = { $p0 } { $p1 }
external-records = { $p0 ->
        [one] 1 record
       *[other] { $p0 } records
    }
external-changes = { $p0 ->
        [one] 1 wijziging
       *[other] { $p0 } wijzigingen
    }
external-absent = (geen)
external-whole-record = Het hele record
external-deleted = verwijderd
external-changed = gewijzigd
external-present = aanwezig
external-title = Uw wijzigingen of die op schijf behouden?
external-text = Hier en op schijf gewijzigd: { $p0 }. Kies voor elk uw versie of die op schijf; opslaan wacht tot u kiest.
external-text-one = Hier en op schijf gewijzigd: { $p0 }. Kies uw versie of die op schijf; opslaan wacht tot u kiest.
external-fields = { $p0 ->
        [one] 1 veld
       *[other] { $p0 } velden
    }
external-apply = Toepassen
external-all-mine = Alles van mij
external-all-theirs = Alles van schijf
external-later = Later beslissen
external-base = Basis
external-mine = Mijn versie
external-theirs = Versie op schijf
external-row = { $p0 } · { $p1 }
external-side = { $p0 }: { $p1 }

# The QElectroTech import card, its checkpoint confirmation and the front door's refusals.
import-card-title = { $p0 } importeren
import-card-does = Project maken maakt een nieuw project in de map { $p0 }, met { $p1 }, { $p2 } en { $p3 }.
import-card-verdict-clean = Alles in het bestand is geïmporteerd.
import-card-verdict-warnings = Het bestand is geïmporteerd. { $p0 } zijn het bekijken waard; geen ervan houdt u op.
import-card-verdict-errors = Het bestand is geïmporteerd, maar { $p0 } moeten worden hersteld voordat de tekeningen kloppen.
import-card-points = { $p0 ->
        [one] 1 punt
       *[other] { $p0 } punten
    }
import-card-group-errors = Herstellen
import-card-group-warnings = Het bekijken waard
import-card-group-infos = Ter informatie
import-card-row = { $p0 } × { $p1 }
import-card-more-kinds = { $p0 ->
        [one] en nog 1 soort
       *[other] en nog { $p0 } soorten
    }
import-card-where = Het nieuwe project opent op deze lijst, punt voor punt, in Controle ▸ Meldingen onder Migratierapport.
import-card-pages = { $p0 ->
        [one] 1 pagina
       *[other] { $p0 } pagina's
    }
import-map-rows = { $p0 ->
        [one] 1 regel
       *[other] { $p0 } regels
    }
import-card-devices = { $p0 ->
        [one] 1 apparaat
       *[other] { $p0 } apparaten
    }
import-card-connections = { $p0 ->
        [one] 1 verbinding
       *[other] { $p0 } verbindingen
    }
# The import banner's count of the migration report's findings.
import-card-findings = { $p0 ->
        [one] 1 bevinding
       *[other] { $p0 } bevindingen
    }
import-confirm-create = Project maken
reimport-match-title = Opnieuw importeren in die import?
reimport-match-text = { $p0 } komt overeen met de eerdere import van { $p1 } ({ $p2 } % van de items).
reimport-match-accept = Opnieuw importeren
reimport-card-title = { $p0 } opnieuw importeren
reimport-card-summary = Herimport van { $p0 }: { $p1 } toegevoegd, { $p2 } gewijzigd in de bron, { $p3 } behouden met uw wijzigingen, { $p4 } aan beide kanten gewijzigd en behouden, { $p5 } verwijderd in de bron en behouden, { $p6 } ongewijzigd.
reimport-card-nothing = Niets opnieuw te importeren: het bestand is niet gewijzigd sinds de laatste import.
reimport-card-nothing-kept = Niets opnieuw te importeren: het bestand is niet gewijzigd sinds de laatste import; { $p0 } behouden met uw wijzigingen.
reimport-card-more = en nog { $p0 } wijzigingen
reimport-card-apply = Toepassen
reimport-row-added = Toegevoegd: { $p0 }
reimport-row-source = Gewijzigd in de bron: { $p0 }
reimport-row-both = Aan beide kanten gewijzigd, behouden: { $p0 }
reimport-row-removed = Verwijderd in de bron, behouden: { $p0 }
reimport-row-member = { $p0 }: hier "{ $p1 }", in { $p2 } "{ $p3 }"
report-block-new-title = Nieuw rapportblok
eplan-macro-incomplete = Sommige macrorecords konden niet worden geconverteerd. Controleer de geometrie en wijs elke verbindingsrichting toe.
eplan-styles-review = Lijn- en tekststijlen moeten worden vergeleken met de oorspronkelijke macro.
eplan-generated-body = Deze omtrek is gegenereerd uit artikelverbindingen. Het is niet de oorspronkelijke EPLAN-tekening.
eplan-function-review = Controleer de gegenereerde functiedefinitie en aansluittoewijzing vóór gebruik.
eplan-archive-skipped = Sommige bestanden van het archief bevatten geen ondersteunde symbooltekening en zijn niet geïmporteerd.
eplan-slk-subset = Variant A is uit een SLK-bibliotheek geïmporteerd. ElektraWatt genereert de andere oriëntaties. Controleer de tekening voor gebruik.
eplan-slk-pins-review = De aansluitpuntrecords konden niet worden gelezen. Voeg alle elektrische verbindingen toe en controleer ze voor gebruik.
eplan-style-review = Tekenstijlen zijn naar de standaardlijnstijl omgezet. Controleer lijndiktes, vullingen en tekst.

eplan-slk-graphics-incomplete = Sommige bibliotheekelementen konden niet worden omgezet. Vergelijk de tekening voor gebruik met de bron.
eplan-title = { $p0 } importeren
eplan-terms-title = Voorwaarden van het EPLAN Data Portal
eplan-terms-text = De voorwaarden van het EPLAN Data Portal beperken de gegevens die je daar downloadt tot EPLAN-producten. Zorg dat jouw voorwaarden dit gebruik toestaan; de download van de fabrikant zelf is de betere bron.
eplan-terms-local = ElektraWatt leest het bestand alleen van je schijf. Het downloadt of deelt nooit artikelgegevens, en legt van elk artikel het bestand en de SHA-256 ervan vast.
eplan-continue = Doorgaan
eplan-parts-text = { $p0 } bevat { $p1 } artikelen. Het project heeft een kopie van { $p2 } ervan; { $p3 } van die kopieën zouden veranderen.
eplan-parts-note = Bijwerken toont de gewijzigde velden voordat er iets verandert. Een artikel dat het project niet heeft, wordt er nooit aan toegevoegd: voeg de artikelen toe aan je artikelbibliotheek en wijs ze vandaar toe.
eplan-update = { $p0 ->
        [one] 1 artikelkopie bijwerken
       *[other] { $p0 } artikelkopieën bijwerken
    }
eplan-library = Aan mijn artikelbibliotheek toevoegen
eplan-report-text = { $p0 } is een EPLAN-rapport. Het wordt een nieuw project zonder tekeningen.
eplan-report-none = Geen koppeling past bij de kolommen van dit bestand. Koppel de kolommen zelf, of importeer het bestand als artikeltabel.
eplan-report-devices = Importeren als apparatenlijst
eplan-report-terminals = Importeren als klemmenplan
eplan-report-connections = Importeren als verbindingenlijst
eplan-report-saved = { $p0 }, met je opgeslagen koppeling
eplan-report-map = De kolommen koppelen…
eplan-report-parts = Importeren als artikeltabel
eplan-map-intro = Kies wat elke kolom van { $p0 } in het EPLAN-rapport is. De koppeling wordt bewaard voor het volgende bestand van dit rapport.
eplan-map-list = Rapport
eplan-target-devices = Apparatenlijst
eplan-target-terminals = Klemmenplan
eplan-target-connections = Verbindingenlijst
eplan-member-strip = Klemmenstrook
eplan-member-designation = Aanduiding
eplan-member-level = Niveau
eplan-member-part = Artikelnummer
eplan-member-description = Omschrijving
eplan-member-from = Van
eplan-member-to = Naar
eplan-member-from-level = Niveau van
eplan-member-to-level = Niveau naar
plc-count-addresses = { $p0 ->
        [one] 1 kanaaladres
       *[other] { $p0 } kanaaladressen
    }
