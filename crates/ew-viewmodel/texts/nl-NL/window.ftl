# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: GPL-3.0-or-later

# The texts of the window in nl-NL: the window's frame: the command bar, the rail, the panes and tabs, the status bar,
# the bars of the panes, the View settings and the messages of a script. Every key of the en-GB file of the same name,
# with the same arguments; a message missing here falls back to en-GB. Product and file names, rule IDs, command
# names, key caps, identifiers and units are not translated.

# A UI scale in per cent.
scale = { $p0 } %
min-font-off = Uit
min-font-pt = { $p0 } pt
status-handle-moves-up-down = Deze greep gaat alleen omhoog en omlaag: druk op ↑ of ↓.
status-handle-moves-sideways = Deze greep gaat alleen naar links en rechts: druk op ← of →.
status-handle-cannot-move = De greep kan die kant niet op: de uiteinden van de draad blijven op hun aansluitpunten en zijn hoeken blijven haaks.
status-handle-refused = De greep is niet verschoven: { $p0 }
bar-supply = Voeding
status-cursor = x { $p0 } y { $p1 } mm
status-zoom = { $p0 } %
status-grid-qet = QET 10 px
status-grid-mm = { $p0 } mm
status-grid-off = Raster uit
message-no-canvas = De pagina kan niet worden getoond: { $p0 }
bar-properties = Eigenschappen
bar-structure = Structuur

bar-move-up = Omhoog
bar-move-down = Omlaag
bar-go-to = Ga naar
bar-follow-reference = Verwijzing volgen
bar-device = Apparaat
bar-symbol-editor = Symbooleditor
bar-terminal = Klem
bar-marker = Aanduiding
bar-wire = Draad
bar-interruption = Onderbreking
bar-rotate = Draaien
bar-mirror = Spiegelen
bar-renumber = Apparaten hernummeren
bar-navigators = Navigators
bar-workspaces = Werkruimten
commandbar-name = Opdrachtbalk
commandbar-tabs = Opdrachttabs
commandbar-app = ElektraWatt
commandbar-menu = Menu
commandbar-search = Opdracht of apparaat zoeken
commandbar-search-key = Ctrl K
commandbar-more = Meer
commandbar-variants = Varianten
zoom-control = Zoom
message-no-snapshot = De momentopname vraagt om de paginarenderer, die deze uitvoerder niet heeft.
message-expected-refusal = een weigering verwacht; de stap is aanvaard
message-expect-refused-inner = script.expect_refused vraagt om `action`, een actie-ID, en `params` als object
message-expected-failure = een mislukking verwacht; de stap is niet mislukt
message-expect-failed-inner = script.expect_failed vraagt om `action`, een actie-ID anders dan script.expect_refused en script.expect_failed, en `params` als object
window-title = { $p0 } - ElektraWatt
window-title-start = ElektraWatt
status-hint-start = Geen project open
status-page = { $p0 } ({ $p1 } van { $p2 })
status-saved = opgeslagen
status-unsaved = *
status-read-only = alleen-lezen: { $p0 }
status-language = NL
workspace-schematic = Schema
workspace-plc = PLC
workspace-fluid = Fluïdtechniek
language-en = English
language-nl = Nederlands
theme-system = Systeem volgen
theme-light = Licht
theme-dark = Donker
theme-hc-dark = Hoog contrast, donker
theme-hc-light = Hoog contrast, licht
density-comfortable = Ruim
density-compact = Compact
pane-pages = Pagina's
pane-page-view = Paginaweergave
pane-properties = Eigenschappen
bar-lists-parts = Stuklijst
bar-lists-wires = Draadlijst
bar-lists-assembly = Montagelijsten
bar-lists-cut = Zaaglijst
bar-lists-consumables = Verbruiksmaterialen
pane-jobs = Taken
status-notifications = Notificaties { $p0 }
status-job = { $p0 } { $p1 } %
status-job-phase = { $p0 }: { $p1 }
filter-clear = Het filter wissen
pane-find = Zoeken en vervangen
status-tip-zoom = Het wiel zoomt rond de aanwijzer; Shift+wiel schuift opzij; sleep met de middelste knop, of met Spatie ingedrukt, om te schuiven; { $p0 } past de pagina in. Met Beeld ▸ Muiswiel schuift het tekenvlak schuift het wiel.
status-tip-zoom-scrolls = Het wiel schuift; Ctrl+wiel zoomt rond de aanwijzer; sleep met de middelste knop, of met Spatie ingedrukt, om te schuiven; { $p0 } past de pagina in.
bar-draw-plug = Stekker tekenen
bar-plug-ports = Poorten afdoppen
dock-all-tabs = Alle tabbladen
dock-close = Het onderste dok sluiten
chips-label = Niet op deze pagina:
tab-title = { $p0 } { $p1 }
tab-close-name = { $p0 } sluiten
status-cursor-cell = { $p0 } · x { $p1 } y { $p2 } mm
status-hint-link = Klik om naar { $p0 } te gaan
status-hint-follow = Klik of { $p1 }: naar { $p0 }
status-hint-follow-none = Klik of gebruik Pagina ▸ Kruisverwijzing volgen: naar { $p0 }
status-hint-link-external = Koppeling naar een ander project
pane-preview = Voorbeeld
pane-history = Geschiedenis
status-put-away-undo = Ongedaan maken legde weg wat aan de aanwijzer hing en maakte niets ongedaan; nog eens neemt de laatste stap terug.
status-put-away-redo = Opnieuw legde weg wat aan de aanwijzer hing en deed niets opnieuw; nog eens doet de volgende stap opnieuw.
rail-project = Project
rail-symbols = Symboolkiezer
rail-parts = Apparaatkiezer
rail-pages = Pagina's
rail-devices = Apparaten
rail-views = Pagina's of apparaten
status-ready = Gereed
# The state field after Help ▸ Tutorials without an installed guide.
status-guide-missing = De handleiding is niet met deze kopie van het programma geïnstalleerd
status-selected = { $p0 } geselecteerd
status-findings-errors = { $p0 ->
        [one] 1 fout
       *[other] { $p0 } fouten
    }
status-findings-warnings = { $p0 ->
        [one] 1 waarschuwing
       *[other] { $p0 } waarschuwingen
    }
status-findings-infos = { $p0 ->
        [one] 1 info
       *[other] { $p0 } info's
    }
status-findings-pending = { $p0 ->
        [one] 1 wachtend op deze pagina
       *[other] { $p0 } wachtend op deze pagina
    }
status-renderer-why = Het tekenvlak tekent op de CPU
status-bar-saved = Opgeslagen
status-bar-unsaved = Niet-opgeslagen wijzigingen
splitter-name = { $p0 } en { $p1 } van grootte veranderen
bar-rail = Rail
bar-duct = Kabelgoot
bar-auto-fill = Rail vullen
tab-title-list-page = { $p0 } · pagina { $p1 } van { $p2 }
bar-enclosure = Kast
language-name = Nederlands
pane-glossary = Woordenlijst
status-driver-missing = Samenvoegdriver niet ingesteld
status-driver-tip = Samenvoegdriver instellen: git voegt de bestanden van dit project dan per record samen
bar-cable = Kabel
bar-report = Rapport

# A card that waits for Apply, in the status bar and on the Preview tab.
status-waiting = { $p0 ->
        [one] 1 wijziging wacht op Toepassen
       *[other] { $p0 } wijzigingen wachten op Toepassen
    }
status-waiting-open = Er staat een voorbeeld open
status-waiting-tip = Toon het voorbeeld, waar Toepassen de wijzigingen doorvoert
bar-new-station = Nieuw station
bar-rack = Rek
bar-attach-cards = Modules bevestigen
bar-plc-export = Export naar PLC
bar-plc-import = Import uit PLC
bar-addresses = Adressen
bar-addresses-at-terminals = Adressen bij klemmen
bar-instrument-list = Instrumentenlijst
bar-bus-page = Nieuwe buspagina
bar-fluid-page = Fluïdumpagina
bar-fluid-symbol = Fluïdumsymbool
bar-guide = Handleiding
commandbar-workspaces = Werkruimte
status-panel-front = Het paneel { $p0 } stond al open: het staat nu vooraan.
bar-plc-module-replace = Vervangen
bar-plc-module-remove = Verwijderen
bar-plc-module-draw = Op pagina tekenen
bar-plc-module-move = Naar andere pagina
bar-plc-module-goto = Naar pagina
bar-plc-module-place = Hier plaatsen
bar-plc-module-left = Naar links
bar-plc-module-right = Naar rechts

# New project, the templates and the check level.
window-title-template = Sjabloon: { $p0 } - ElektraWatt
bar-templates = Sjablonen
bar-save-as-template = Opslaan als sjabloon
bar-check-level = Controleniveau

# Tekststijlen (Project ▸ Tekststijlen…).
bar-text-styles = Tekststijlen

# F1 opent de uitleg bij wat onder de hand ligt; de statusbalk zegt wat het opent.
status-help = F1: { $p0 }

# Een naam met zijn sneltoets, in een tooltip ("Terug (Alt+Links)").
name-with-key = { $p0 } ({ $p1 })
# Een knop die niet kan, in zijn tooltip: zijn naam, dan waarom.
name-refused = { $p0 }: { $p1 }
# Een naam met hoeveel hij bevat ("Tags (11)").
name-with-count = { $p0 } ({ $p1 })
# Een uitgeschakelde regel van het palet zoals een voorlezer hem noemt: het label, dan waarom.
palette-entry-refused = { $p0 }. { $p1 }
# De toegankelijke naam van de punt naast een opdracht.
commandbar-variants-of = Varianten van { $p0 }
# De regel van een stap bij de eerste start: het nummer, hoeveel er zijn, en de titel.
firstrun-step = { $p0 } / { $p1 }: { $p2 }
# De toegankelijke naam van het naamveld van een rij in een navigator.
navigator-rename-row = { $p0 } hernoemen
# De kop van een taak: wat voor taak, en wat hij maakt.
job-name = { $p0 } { $p1 }
# De cel "waar" van de sneltoetsen met wat ze opmerkt ("Tekenblad · Gewijzigd").
keymap-where-note = { $p0 } · { $p1 }
# Een geweigerde toets: de toets, dan de actie waarvoor hij was.
keymap-chord-action = { $p0 } ({ $p1 })
# De toegankelijke naam van een ongedaan gemaakte stap in de geschiedenis.
history-entry-redo = { $p0 }, { $p1 }
# De regel van een gewijzigd record: het woord van de wijziging, de soort, het label en de gewijzigde velden tussen haakjes.
preview-change-line = { $p0 } { $p1 } { $p2 }{ $p3 }
# De toegankelijke naam van een tabelcel: de kolom en de tekst; bij een handmatig adres dat het handmatig is.
table-cell-name = { $p0 } { $p1 }
table-cell-manual = { $p0 }, { $p1 }
# De strook van een kaart in de lijst: de titel en de samenvatting.
card-strip = { $p0 } · { $p1 }

## The file dialogs of Rework list…

prompt-open-archive-title = Vergelijken met een archief
prompt-save-rework-title = De wijzigingslijst exporteren
prompt-rework-name = wijzigingslijst.xlsx
prompt-label-folder-title = De map voor de labelbestanden
