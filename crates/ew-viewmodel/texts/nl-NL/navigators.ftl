# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: GPL-3.0-or-later

# The texts of the window in nl-NL: the navigators, their columns, chips and badges, tables and the find pane. Every
# key of the en-GB file of the same name, with the same arguments; a message missing here falls back to en-GB. Product
# and file names, rule IDs, command names, key caps, identifiers and units are not translated.

nav-badge-unplaced = niet getekend
# Wat een badgewoord betekent, de zweeftekst.
nav-badge-unplaced-tip = Nog op geen pagina getekend: geen symbool of kaderaansluiting draagt het.
nav-badge-spare-tip = Een reserve: vrijgehouden, er is geen draad op aangesloten. Sluit er een draad op aan, of zet de toestand in Eigenschappen.
nav-badge-frozen = vastgezet
nav-badge-spare = reserve
nav-badge-main = hoofdfunctie
nav-badge-aux = hulpfunctie
nav-badge-external = extern
nav-badge-strip = klemmenstrook
nav-badge-assembly = samenstelling
nav-badge-unknown-size = onbekende maat
nav-badge-waived = vrijgesteld
nav-badge-fix = oplossing
nav-badge-error = fout
nav-badge-warning = waarschuwing
nav-badge-info = info
nav-badge-generated = gegenereerd
nav-badge-qet-grid = QET 10 px
nav-badge-spares = { $p0 } reserve
nav-badge-pending = wachtend
nav-badge-end = einde
nav-badge-terminated = afgesloten
nav-badge-not-terminated = niet afgesloten
nav-badge-master = master
nav-badge-passes-through = geeft het net door
nav-badge-not-drawn = niet getekend
nav-badge-earth-connection = aardverbinding
list-none = geen
list-and = en
badge-stale = Verouderd
badge-failed = Mislukt
badge-done = Klaar
badge-cancelled = Geannuleerd
empty-jobs = Nog geen taken
empty-jobs-text = Exporten die u start verschijnen hier met hun voortgang.
empty-preview-text = Hernummeren, een snelle oplossing of een import toont hier zijn wijzigingen voordat u ze toepast.
empty-history-text = Uw wijzigingen verschijnen hier; Ongedaan maken gaat er stap voor stap door terug.
empty-find-text = Probeer andere woorden, of zoek in meer velden en folio's.
empty-find-start = Zoeken in het project
empty-find-start-text = Typ wat u zoekt; de resultaten verschijnen hier.
empty-notifications-text = Exporten, herstellen en bewerkingen van veel objecten melden zich hier.
empty-messages = Geen bevindingen
empty-messages-text = Project controleren toont hier de fouten en waarschuwingen van het project.
empty-nav-text = Wat u op de pagina's tekent, staat hier.
empty-nav-pages-text = Nieuwe pagina voegt de eerste pagina van het project toe.
empty-nav-filter = Niets komt overeen met "{ $p0 }"
empty-nav-filter-text = Wijzig of wis het filter om elke rij te zien.
empty-nav-clear = Filter wissen
empty-nav-pages = Nog geen pagina's
empty-nav-devices = Nog geen apparaten
empty-nav-connections = Nog geen verbindingen
empty-nav-strips = Nog geen klemmenstroken
empty-nav-cables = Nog geen kabels
empty-nav-plc-no-cpu = Nog geen CPU
empty-nav-plc-no-cpu-text = { $p0 } wacht op een CPU: een PLC-module staat hier onder zijn station en krijgt zijn adressen ervan. Maak een station met Nieuw station…, of plaats een CPU met Tekenen ▸ Apparaat (zoek op CPU); de modules sluiten erbij aan.
empty-nav-plc = Nog geen PLC-stations of -modules
empty-nav-bus = Nog geen buspoorten
empty-nav-fluid = Nog geen slangen
empty-nav-tubes = Nog geen slangen
empty-nav-media = Nog geen media
empty-nav-potentials = Nog geen potentialen
empty-nav-ips = Nog geen onderbrekingspunten
empty-nav-structure = Nog geen structuuraanduidingen
empty-nav-parts = Nog geen artikelen in gebruik
empty-nav-blocks = Nog geen rapportblokken
badge-pending = Wachtend
badge-waived = Vrijgesteld
badge-fix = Oplossen
badge-not-undoable = Niet ongedaan te maken
find-field-text = Tekst
find-text = Zoeken
find-replacement = Vervangen door
find-regex = Reguliere expressie
find-match-case = Hoofdlettergevoelig
find-whole-word = Heel woord
find-in-selection = In selectie
find-folios = Folio's (gescheiden door komma's)
find-run = Zoeken
find-replace-all = Alles vervangen
find-close = Sluiten
find-results = { $p0 ->
        [one] 1 resultaat
       *[other] { $p0 } resultaten
    }
find-no-results = Geen resultaten
find-truncated = Meer dan 10.000 resultaten: verfijn de zoekopdracht
find-stale = verouderd
find-replaced = { $p0 } vervangen; voer opnieuw uit om te vernieuwen
find-updated-fewer = { $p0 ->
        [one] Voorbeeld bijgewerkt: 1 resultaat minder
       *[other] Voorbeeld bijgewerkt: { $p0 } resultaten minder
    }
find-updated-more = { $p0 ->
        [one] Voorbeeld bijgewerkt: 1 resultaat meer
       *[other] Voorbeeld bijgewerkt: { $p0 } resultaten meer
    }
find-updated-same = Voorbeeld bijgewerkt: dezelfde resultaten
find-not-replaced = niet vervangen: { $p0 }
# Een resultaatregel: het onderwerp, het veld en de tekst; ervoor de pagina en de cel, erna de vervanging of waarom
# niet vervangen.
find-hit = { $p0 } · { $p1 } · { $p2 }
find-hit-at = { $p0 } · { $p1 }
find-hit-replaced = { $p0 } -> { $p1 }
find-hit-not-replaced = { $p0 } ({ $p1 })
find-card-line = { $p0 } resultaten op { $p1 } records · { $p2 } kenmerken, { $p3 } kruisverwijzingsteksten
chip-authored = = ingevoerd
chip-inherited = ^ overgenomen · { $p0 }
chip-derived = f afgeleid · { $p0 }
chip-overridden = ! overschreven · { $p0 }
chip-frozen = * vastgezet · { $p0 }
chip-library = # bibliotheek · { $p0 }
chip-generated = ~ gegenereerd door blok '{ $p0 }'
chip-varies = ≠ verschilt ({ $p0 })
chip-word-page = pagina
chip-word-inherited = overgenomen
chip-word-derived = afgeleid
chip-word-override = overschrijving
chip-word-frozen = vastgezet
chip-word-library = bibliotheek
chip-word-generated = gegenereerd
chip-word-varies = verschilt
chip-word-profile = profiel
chip-word-no-supply = geen voeding
chip-word-no-protection = geen beveiliging
chip-word-potential = potentiaal
chip-word-cable = kabel
chip-word-default = standaard
# The phrases of a provenance chip's text that ew-api builds in English.
chip-via-frozen-sign-hidden = vastgezet (teken verborgen)
chip-via-frozen = vastgezet
chip-via-numbering = nummering
chip-via-main-representation = hoofdweergave
chip-via-strip-groups = klemmenstrookgroepen
chip-via-function-definition = functiedefinitie
chip-via-override-cdp = overschrijving (CDP)
chip-via-wire-numbering = draadnummering
chip-via-typed-by-hand = met de hand getypt
chip-via-no-length = geen lengte: typ die hier, of de monteur meet de draad
chip-via-set-by-hand = met de hand gezet
chip-via-rule-unverified = regel (niet bevestigd)
chip-via-rule-assumed-class = regel (niet bevestigd): fijndradige draad
chip-via-rule-assumed-clamp = regel (niet bevestigd): klemsoort onbekend
chip-via-rule-assumed-both = regel (niet bevestigd): fijndradige draad, klemsoort onbekend
chip-via-rule-verified = regel
chip-via-rule-verified-assumed-class = regel: fijndradige draad
chip-via-rule-verified-assumed-clamp = regel: klemsoort onbekend
chip-via-rule-verified-assumed-both = regel: fijndradige draad, klemsoort onbekend
chip-via-made-outside = buiten het project gemaakt
chip-via-shared-stretch = meerdere draden delen dit stuk: zet de eigen waarden van een draad in de tabel Verbindingen
chip-via-workshop-stock = werkplaatsvoorraad
chip-via-workshop-marking = de markering van het project
chip-via-cut-with-cable = geknipt met de kabel { $p0 }
chip-via-connectivity = verbindingen
chip-via-project-description = projectomschrijving
chip-via-cross-reference-format = kruisverwijzingsformaat
chip-via-project-default = projectstandaard
chip-via-override = overschrijving
chip-via-primary-part = primair artikel
chip-via-layout-label = layoutlabel
chip-via-part = artikel
chip-via-rail-length = raillengte min plaatsingen en eindsteunen
chip-via-path = pad
chip-via-override-on-another = overschrijving op een andere verbinding
chip-via-child-of = kind van { $p0 }
chip-via-terminal-of-strip = klem van klemmenstrook { $p0 }
chip-via-numbering-scheme = nummeringsschema '{ $p0 }'
chip-via-structure-box-on-page = structuurkader op pagina { $p0 }
chip-via-page = pagina { $p0 }
chip-via-override-on = overschrijving op { $p0 }
chip-via-project-description-is = projectomschrijving is '{ $p0 }'
chip-via-cable-core = van kabel { $p0 }, ader { $p1 }
chip-via-potential-definition-at = potentiaaldefinitie bij { $p0 }
chip-via-potential-definition = potentiaaldefinitie { $p0 }
chip-via-potential = potentiaal { $p0 }
chip-via-colour-profile = kleurprofiel '{ $p0 }'
chip-via-size-profile = maatprofiel '{ $p0 }'
chip-via-class = klasse { $p0 }
chip-via-no-supply-upstream = geen voeding gevonden vóór { $p0 }, dus de nominale stroom ervan wordt niet gebruikt
chip-via-no-supply = geen voeding gevonden vóór de draad, dus bepaalt geen beveiligingsapparaat de doorsnede
chip-via-no-protection = geen beveiligingsapparaat vóór de draad bepaalt de doorsnede
chip-via-built-in-default = ingebouwde standaard: het project heeft geen profiel dat deze waarde bepaalt
chip-via-medium-definition-at = medium bepaald op { $p0 }
chip-via-through-valves = via de ventielen doorgegeven
chip-via-no-medium = geen medium bepaald
chip-via-project-default-tube = de standaardslang van het project
chip-via-medium = medium { $p0 }
chip-via-tube-part = slangartikel { $p0 }
chip-via-no-tube-spec = geen standaardslang, mediumslang of slangartikel
chip-via-no-terminal = op geen ventieleiland
nav-show-solenoid = { $p0 }, magneetspoel { $p1 }
chip-via-plug = zijn stekker
chip-via-plc-output = PLC-uitgang { $p0 }
chip-via-no-plc-output = nog geen PLC-uitgang
chip-via-valve-part = het artikel van het ventiel
chip-via-earth-connection = een aardsymbool: een verbinding met aarde, geen apparaat, dus geen kenmerk
table-nothing = Er kan geen cel worden toegepast.
table-no-row = geen rij
table-no-column = geen kolom
table-not-clearable = kan niet worden gewist
table-paste-too-large = te groot: er kunnen hoogstens { $p0 } cellen van hoogstens { $p1 } tekens worden geplakt
table-paste-unclosed-quote = niet gesloten aanhalingsteken in rij { $p0 }
table-pattern-unclosed = niet gesloten accolade: een `{"{"}` heeft geen `{"}"}` (schrijf `{"{{"}` voor een letterlijke accolade)
table-pattern-stray-close = een `{"}"}` sluit niets af (schrijf `{"}}"}` voor een letterlijke accolade)
table-pattern-unknown = onbekende uitdrukking {"{"}{ $p0 }{"}"}: gebruik {"{"}n{"}"}, {"{"}n*2+1{"}"}, {"{"}n:02{"}"}, {"{"}col:<kolom>{"}"} of {"{"}prev{"}"}
table-pattern-number = het getal in {"{"}{ $p0 }{"}"} is te groot
table-pattern-column = onbekende kolom { $p0 } in {"{"}col:{ $p0 }{"}"}
table-row-overflow = de teller loopt over
table-row-negative = de teller is negatief
table-row-too-long = de tekst is langer dan 10000 tekens
table-preview-title = Voorbeeld: { $p0 } { $p1 } { $p2 } op { $p3 } { $p4 }
table-preview-invalid = { $p0 } · { $p1 } ongeldig
table-cell = cel
table-cells = cellen
table-import-title = { $p0 } importeren
table-import-nothing = Niets te importeren: het bestand komt overeen met het project.
table-import-counts = { $p0 }, { $p1 }, { $p2 }, { $p3 }, { $p4 }, { $p5 }
table-import-changed = { $p0 ->
        [one] 1 cel gewijzigd
       *[other] { $p0 } cellen gewijzigd
    }
table-import-kept = { $p0 ->
        [one] 1 cel behouden
       *[other] { $p0 } cellen behouden
    }
table-import-refused = { $p0 ->
        [one] 1 cel geweigerd
       *[other] { $p0 } cellen geweigerd
    }
table-import-unknown = { $p0 ->
        [one] 1 onbekende rij
       *[other] { $p0 } onbekende rijen
    }
table-import-missing = { $p0 ->
        [one] 1 rij niet in het bestand
       *[other] { $p0 } rijen niet in het bestand
    }
table-import-conflicts = { $p0 } conflicten
table-import-conflict-kept = 1 conflict: de waarde van het project blijft behouden
table-import-conflicts-kept = { $p0 } conflicten: de waarden van het project blijven behouden
table-import-conflict-taken = 1 conflict: de waarde uit het bestand wordt overgenomen
table-import-conflicts-taken = { $p0 } conflicten: de waarden uit het bestand worden overgenomen
table-import-cell = { $p0 } · { $p1 } · "{ $p2 }" → "{ $p3 }" · { $p4 }
table-import-cell-applied = Toepassen neemt de waarde uit het bestand over
table-import-cell-kept = behouden: alleen in het project gewijzigd
table-import-cell-conflict = conflict: de waarde van het project blijft behouden
table-import-cell-conflict-taken = conflict: Toepassen neemt de waarde uit het bestand over
table-import-cell-read-only = alleen-lezen
table-import-cell-refused = geweigerd
table-import-cells = Cellen
table-import-missing-rows = Rijen die niet in het bestand staan
table-toggle = Tabel
table-name = Tabel { $p0 }
table-columns = Kolommen
table-move-left-name = { $p0 } naar links
table-move-right-name = { $p0 } naar rechts
table-column-first = De kolom staat al vooraan
table-column-last = De kolom staat al achteraan
table-sort-by = Sorteren op { $p0 }
table-derived = Afgeleid; alleen-lezen
table-cell-row = , rij { $p0 } van { $p1 }
table-cell-read-only = , alleen-lezen
table-pattern-title = Patroon vullen
table-pattern-hint = Patroon
table-commit-all-hint = Ctrl+Enter vult alle { $p0 } gemarkeerde rijen.
table-pattern-values = Waarden: { $p0 }
table-resize = Kolom { $p0 } van breedte veranderen
nav-title-devices = Apparaten
nav-title-connections = Verbindingen
nav-title-strips = Klemmenstroken
nav-title-cables = Kabels
nav-title-plc = PLC
nav-title-plc-grid = PLC-tabel
nav-title-bus = Bus
nav-title-fluid = Fluïdum
nav-title-tubes = Slangentabel
nav-title-media = Mediumtabel
nav-title-potentials = Potentialen
nav-title-ips = Onderbrekingspunten
nav-title-structure = Structuuraanduidingen
nav-title-parts = Gebruikte artikelen
nav-title-blocks = Rapportblokken
nav-title-messages = Meldingen
nav-tree-name = Navigator { $p0 }
nav-mode-flat = Platte lijst
nav-mode-section = Per sectie
nav-mode-structure = Per structuur
nav-sort-folio = Sorteren op folio
nav-sort-type = Sorteren op type
nav-badge-consumers = { $p0 ->
        [one] 1 verbruiker
       *[other] { $p0 } verbruikers
    }
nav-badge-cpu = CPU
nav-badge-head-station = kopstation
nav-badge-io-device = I/O-apparaat
nav-badge-valve-terminal = ventieleiland
nav-badge-io-link-master = IO-Link-master
nav-badge-drive = aandrijving
nav-badge-external-partner = externe partner
nav-badge-station = station
nav-filter-hint = Filter
badge-generated = gegenereerd
badge-qet-grid = QET 10 px
badge-current = huidig
nav-page-name = { $p0 } { $p1 }, pagina{ $p2 }
nav-node-name = { $p0 }, { $p1 }
table-sorted-ascending = { $p0 }, oplopend gesorteerd
table-sorted-descending = { $p0 }, aflopend gesorteerd
nav-title-macros = Macro's
nav-title-libraries = Bibliotheken
empty-nav-macros = Nog geen macro's
empty-nav-macros-text = Opslaan als macro… in het menu Bewerken bewaart een selectie in een bibliotheek; de macro's ervan staan hier.
empty-nav-libraries = Nog geen bibliotheken
badge-update-available = Update beschikbaar
badge-index-failed = Index mislukt
badge-profile-unreadable = PLC-profiel onleesbaar
badge-fits-function = Past bij de functie van { $p0 }
badge-same-category = Zelfde categorie
badge-clamps-unsized = klemmaten niet opgegeven
nav-title-unplaced = Ongeplaatste apparaten
empty-nav-unplaced = Nog geen paneellayout
empty-nav-unplaced-text = Apparaten staan hier zodra de locatie een kast heeft.
empty-nav-unplaced-done = Alle apparaten geplaatst
empty-nav-unplaced-done-text = Elk apparaat van { $p0 } is geplaatst.
list-continues-row = De lijst gaat verder op pagina { $p0 } ›
empty-nav-terminals = Nog geen klemmen
empty-nav-terminals-text = Plaats een klem met Tekenen ▸ Klem…; hij komt in de klemmenstrook die u daar kiest.

# The strip editor's scope in the filter row.
nav-scope-strip = Strook: { $p0 }
nav-scope-clear = Alle klemmenstroken tonen
nav-plc-addresses = Adressen
nav-badge-manual = handmatig
nav-badge-not-drawn-tip = Nog op geen pagina getekend, daarom toont het kenmerk ? waar het paginanummer hoort. Teken het om het zijn plaats te geven.
nav-guide = Handleiding openen
chip-via-port-no-medium = geen medium: kies er een in de rij Medium van de bron, de persluchtaansluiting
chip-via-no-medium-yet = nog geen medium: kies er hier een
nav-badge-in-no-plc = in geen PLC
nav-badge-fix-all = Alles oplossen
chip-go-page = Naar pagina { $p0 }
chip-go-page-cell = Naar pagina { $p0 }, { $p1 }
chip-put-here = Hier zetten
list-columns-title = Lijstkolommen
list-rows-title = Eigen rijen
list-columns-table = Kolommen
list-columns-column = Kolom
list-columns-print = Afdrukken
list-columns-heading = Kop
list-columns-width = Breedte
list-columns-order = Volgorde
list-rows-table = Rijen die het model niet heeft, opgenomen bij de eigen rijen van de lijst; lege rijen vallen weg
list-rows-row = Rij
