# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: GPL-3.0-or-later

# The texts of the window in nl-NL: toasts, banners, announcements, notes, jobs, recovery and history. Every key of
# the en-GB file of the same name, with the same arguments; a message missing here falls back to en-GB. Product and
# file names, rule IDs, command names, key caps, identifiers and units are not translated.

announce-place = { $p0 } plaatsen: { $p1 }, variant { $p2 }, op { $p3 }
announce-place-no-cell = { $p0 } plaatsen: { $p1 }, variant { $p2 }
banner-host-commit = { $p0 } mislukt: { $p1 }
announce-wire-routing = Draad: routeren vanaf { $p0 }. Klik op een aansluitpunt; { $p1 } verwijdert de laatste knik; { $p2 } annuleert
toast-tube-no-size = Dit project heeft geen standaardslang: Fluïdum ▸ Instellingen fluïdtechniek… stelt de maat in, en de rij Slang van elke slang wijzigt die.
toast-wire-numbers-on = Draadnummers worden nu afgedrukt op elke pagina van het project, in de PDF en bij afdrukken.
toast-wire-numbers-off = Draadnummers worden niet meer afgedrukt, op geen enkele pagina van het project.
toast-wire-numbering-potential = Elke draad van een potentiaal krijgt nu hetzelfde nummer, dat van zijn potentiaal, in het hele project.
toast-wire-numbering-own = Elke draad krijgt nu weer een eigen nummer, in het hele project.
toast-wire-numbers-hidden = Zet Draadnummers op de pagina's tonen aan om ze te zien.
toast-box-pin-part = De aansluitingen van de kast van een artikel zijn die van het artikel: ze worden geschikt in Artikelen, dat nu opengaat.
toast-box-pin-not-yours = De aansluitingen van deze kast zijn die van zijn artikel, en het artikel is niet van u: Mijn eigen ervan maken, dan Schikken… om ze te wijzigen.
toast-box-pin-side-full = Deze zijde van de box heeft geen vrij punt voor nog een aansluiting; maak de box groter of zet de aansluiting op een andere zijde.
toast-drawn-as-box = { $p0 } heeft geen eigen symbool en wordt als kader getekend, met het gereedschap Black box: klik op de eerste hoek en dan op de tegenoverliggende hoek.
toast-accessory-left-off = { $p0 } weggelaten op { $p1 }. De artikelen van de klemmen noemen het nog voor de andere klemmenstroken.
notice-ip-not-placed = Niet geplaatst: { $p0 }
notice-ip-on-ip = Niet geplaatst: het onderbrekingspunt { $p0 } staat hier al; plaats het op een pin of een vrij rasterpunt
notice-ip-too-close = Niet geplaatst: te dicht bij het onderbrekingspunt { $p0 }, hun namen en verwijzingen zouden over elkaar heen staan. De twee moeten { $p1 } rasterstappen ({ $p2 } mm) uit elkaar staan, de een boven de ander, of houd Alt ingedrukt om het toch te plaatsen
toast-ip-beside-pin = { $p0 } staat naast { $p1 } zonder ermee te verbinden: plaats het op de pin, of trek er een draad naartoe.
notice-ip-on-wire = Niet geplaatst: het punt zou op een draad liggen zonder ermee te verbinden; plaats het op een pin of een vrij rasterpunt
notice-ip-into-box-source = Niet geplaatst: de draad van een bron vertrekt links, bij de staart van de pijl, dus op { $p0 } zou de pijl in de box liggen. Maak er een doel van, of houd Alt ingedrukt om het naast de box te plaatsen en er een draad naartoe te trekken
notice-ip-into-box-target = Niet geplaatst: de draad van een doel vertrekt rechts, bij de punt van de pijl, dus op { $p0 } zou de pijl in de box liggen. Maak er een bron van, of houd Alt ingedrukt om het naast de box te plaatsen en er een draad naartoe te trekken
notice-ip-second-source = Niet geplaatst: { $p0 } heeft al een bron op pagina { $p1 }; druk op { $p2 } voor een doel, of markeer één bron als de ster
notice-ip-second-source-unbound = Niet geplaatst: { $p0 } heeft al een bron op pagina { $p1 }; gebruik { $p2 } voor een doel, of markeer één bron als de ster
toast-strip-inserted = { $p0 ->
        [one] 1 klem ingevoegd vóór { $p1 }
       *[other] { $p0 } klemmen ingevoegd vóór { $p1 }
    }.
toast-strip-appended = { $p0 ->
        [one] 1 klem ingevoegd aan het einde van { $p1 }
       *[other] { $p0 } klemmen ingevoegd aan het einde van { $p1 }
    }.
toast-strip-moved = { $p0 ->
        [one] { $p1 } verplaatst
       *[other] { $p0 } klemmen van { $p1 } verplaatst
    }.
toast-accessory-added = { $p0 } toegevoegd na { $p1 }.
toast-accessory-added-first = { $p0 } toegevoegd aan het begin van { $p1 }.
toast-accessory-added-before = { $p0 } toegevoegd vóór { $p1 }.
toast-accessories-removed = { $p0 ->
        [one] 1 toebehoren verwijderd
       *[other] { $p0 } toebehoren verwijderd
    }.
toast-unbridged = { $p0 ->
        [one] 1 brug verwijderd
       *[other] { $p0 } bruggen verwijderd
    }.
toast-cores-swapped = Aders { $p0 } en { $p1 } van { $p2 } verwisseld.
announce-tool = { $p0 }: { $p1 }
announce-selected = { $p0 } geselecteerd
announce-device = { $p0 }, { $p1 }, folio { $p2 }
announce-device-unplaced = { $p0 }, { $p1 }
announce-device-connected = { $p0 }, { $p1 }, { $p2 }
announce-device-kind = Apparaat
announce-cps = { $p0 ->
        [one] 1 aansluitpunt
       *[other] { $p0 } aansluitpunten
    }
announce-wires = { $p0 ->
        [one] 1 draad
       *[other] { $p0 } draden
    }
announce-wire = { $p0 }, { $p1 }, { $p2 } naar { $p3 }
announce-wire-no-potential = { $p0 }, { $p1 } naar { $p2 }
announce-wire-unnumbered = Ongenummerde draad
announce-end-junction = een aftakking
announce-item = { $p0 }, { $p1 }
announce-item-graphic = Grafisch
announce-item-free_text = Tekst
announce-item-wire_marker = Draadaanduiding
announce-item-cable_mark = Markering van de kabel
announce-item-image = Afbeelding
announce-item-surface_view = Plaataanzicht
announce-item-unknown = Pagina-element
announce-junction = Aftakking, folio { $p0 }, { $p1 }
announce-junction-unplaced = Aftakking, { $p0 }
announce-ip = Onderbrekingspunt { $p0 }, folio { $p1 }
announce-ip-unplaced = Onderbrekingspunt { $p0 }
announce-structure-box = Structuurkader { $p0 }
announce-structure-box-plain = Structuurkader
announce-on-folio = { $p0 }, folio { $p1 }
announce-end-point = een aansluitpunt
announce-end-unknown = een uiteinde van een nieuwere indeling
job-kind-dxf = DXF-export
toast-export-dxf = DXF-set geëxporteerd, één bestand per pagina, naar { $p0 }: { $p1 }, { $p2 } KiB.
toast-export-dxf-again = DXF-set opnieuw geëxporteerd, één bestand per pagina, naar { $p0 }: { $p1 }, { $p2 } KiB.
banner-gpu-init = Het GPU-tekenvlak kon niet starten ({ $p0 }); het tekenvlak tekent op de CPU.
banner-gpu-lost = Het GPU-tekenvlak is gestopt ({ $p0 }); het tekenvlak tekent de rest van deze sessie op de CPU.
banner-gpu-no-software-adapter = --no-gpu: er is geen software-adapter gevonden, dus het venster gebruikt { $p0 }; het tekenvlak tekent op de CPU.
banner-recovery = Dit project heeft niet-opgeslagen wijzigingen uit een sessie die onverwacht is beëindigd. Het is alleen-lezen geopend. Bestand, Niet-opgeslagen wijzigingen van de eerdere sessie verwerpen verwijdert ze en opent het project om te bewerken; om ze te herstellen opent u het project opnieuw en kiest u Herstellen in de herstelvenster.
banner-read-only = Dit project is alleen-lezen geopend: { $p0 }.
# A read-only project whose damaged library copy a device uses.
banner-damaged-copy = Dit project is alleen-lezen geopend: het apparaat { $p0 } gebruikt de bibliotheekkopie van artikel { $p1 } uit { $p2 }, die beschadigd is. Werk de kopie eerst bij vanuit de bibliotheek.
banner-damaged-copy-unnamed = Dit project is alleen-lezen geopend: het apparaat { $p0 } gebruikt de bibliotheekkopie van zijn artikel, die beschadigd is. Werk de kopie eerst bij vanuit de bibliotheek.
banner-damaged-copy-record = Dit project is alleen-lezen geopend: { $p0 } is beschadigd. Werk de kopie eerst bij vanuit de bibliotheek.
banner-damaged-copy-left-out = Dit project is alleen-lezen geopend: { $p0 } is beschadigd. Werk de kopie bij vanuit de bibliotheek, of maak met Opslaan als een projectkopie zonder deze bibliotheekkopie.
# Two or more damaged copies that a Save As leaves out.
banner-damaged-copies-left-out = Dit project is alleen-lezen geopend: { $p0 } zijn beschadigd. Werk de kopieën bij vanuit hun bibliotheken, of maak met Opslaan als een projectkopie zonder deze bibliotheekkopieën.
# The same banners when no library on this computer holds a damaged copy: no update is offered.
banner-damaged-copy-unnamed-no-library = Dit project is alleen-lezen geopend: het apparaat { $p0 } gebruikt de bibliotheekkopie van zijn artikel, die beschadigd is. Geen bibliotheek op deze computer bevat het, dus de kopie kan niet vanuit een bibliotheek worden bijgewerkt.
banner-damaged-copy-record-no-library = Dit project is alleen-lezen geopend: { $p0 } is beschadigd. Geen bibliotheek op deze computer bevat het, dus de kopie kan niet vanuit een bibliotheek worden bijgewerkt.
banner-damaged-copy-left-out-no-library = Dit project is alleen-lezen geopend: { $p0 } is beschadigd. Geen bibliotheek op deze computer bevat het; maak met Opslaan als een projectkopie zonder deze bibliotheekkopie.
banner-damaged-copies-left-out-no-library = Dit project is alleen-lezen geopend: { $p0 } zijn beschadigd. Geen bibliotheek op deze computer bevat ze; maak met Opslaan als een projectkopie zonder deze bibliotheekkopieën.
banner-findings = { $p0 ->
        [one] Bij het openen van het project is 1 integriteitsbevinding gevonden
       *[other] Bij het openen van het project zijn { $p0 } integriteitsbevindingen gevonden
    }.
banner-settings-bad = Het instellingenbestand kon niet worden gelezen; de standaardwaarden worden gebruikt en het oude bestand is bewaard als { $p0 }.
banner-settings-write = De instellingen konden niet worden geschreven.
banner-recent-removed = { $p0 } kon niet worden geopend en is uit de recente projecten verwijderd.
banner-dismiss = Sluiten
toast-save-as-left-out = Opgeslagen als { $p0 } zonder { $p1 }, die beschadigd is.
toast-save-as-left-out-many = Opgeslagen als { $p0 } zonder { $p1 }, die beschadigd zijn.
toast-damaged-copies-updated = Bijgewerkt vanuit bibliotheek: { $p0 }.
toast-damaged-copies-not-updated = Bijgewerkt vanuit bibliotheek: { $p0 }. Niet bijgewerkt, omdat geen bibliotheek op deze computer een leesbare versie heeft: { $p1 }.
toast-saved-unreleased-format = Opgeslagen in een formaat dat nog niet is uitgebracht. Uitgebrachte versies van ElektraWatt openen dit project alleen-lezen.
banner-no-file-dialog = De bestandsdialoog is niet beschikbaar: hij vraagt om de dienst xdg-desktop-portal op de sessiebus. Start de portal van uw bureaublad (bijvoorbeeld xdg-desktop-portal-gtk of -kde) of open het project als argument.
banner-sample = Dit is het voorbeeldproject. Het is alleen-lezen geopend; Opslaan als maakt er een eigen kopie van.
banner-tutorial-copy = Voor de zelfstudies is een nieuwe kopie van het voorbeeldproject gemaakt.
banner-imported = { $p0 } geïmporteerd: { $p1 }, { $p2 }, { $p3 } in het migratierapport.
banner-import-no-content = De starterbibliotheek is niet gevonden; symbolen waarnaar het project verwijst, kunnen ontbreken.
banner-import-map = { $p0 } naast het bestand is gelezen ({ $p1 }); het migratierapport zegt wat elke regel heeft gekoppeld.
banner-journal-discarded = De niet-opgeslagen wijzigingen van de eerdere sessie zijn verworpen. Het project is open om te bewerken.
banner-workspace-bad = Het werkruimtebestand { $p0 } kon niet worden gelezen; de ingebouwde indeling wordt gebruikt en het bestand is gelaten zoals het is.
banner-workspace-write = Het werkruimtebestand kon niet worden geschreven.
banner-wizard-done = Het sjabloon is geschreven naar { $p0 }. Nieuwe projecten beginnen ermee.
toast-set = { $p0 } ingesteld op { $p1 } { $p2 }.
toast-tag-proposal = Kenmerk { $p0 }: online nummering gaf teller { $p1 }.
toast-joined = Bij { $p0 } ({ $p1 }) gevoegd op { $p2 }.
toast-joined-dropped = { $p0 }; { $p1 }.
toast-strip-new = Nieuwe klemmenstrook; de eerste klem geeft haar kenmerk, zoals { $p0 }: Tekenen ▸ Klem… plaatst haar klemmen.
toast-strip-new-tagged = Nieuwe klemmenstrook { $p0 }: Tekenen ▸ Klem… plaatst haar klemmen.
toast-strip-new-no-example = Nieuwe klemmenstrook; de eerste klem geeft haar kenmerk: Tekenen ▸ Klem… plaatst haar klemmen.
toast-strip-new-for-terminal = De klem komt op een nieuwe klemmenstrook: klik waar hij moet staan.
toast-export-pdf = { $p0 } geëxporteerd: { $p1 }, { $p2 } KiB, in { $p3 }.
toast-export-pdf-again = { $p0 } opnieuw geëxporteerd: { $p1 }, { $p2 } KiB, in { $p3 }.
toast-structure-new = Knooppunt { $p0 } { $p1 } gemaakt.
toast-external-ref-new = Externe verwijzing { $p0 } gemaakt; het gereedschap Onderbrekingspunt koppelt een extern punt eraan via dit label.
toast-ip-deleted-wire = Onderbrekingspunt { $p0 } en de draad die erop aansloot verwijderd.
toast-ip-deleted-wires = Onderbrekingspunt { $p0 } en de { $p1 } draden die erop aansloten verwijderd.
toast-ips-deleted-wire = Onderbrekingspunten { $p0 } en de draad die erop aansloot verwijderd.
toast-ips-deleted-wires = Onderbrekingspunten { $p0 } en de { $p1 } draden die erop aansloten verwijderd.
toast-device-unplaced = { $p0 } heeft nu geen symbool op een pagina; het blijft in de navigator Apparaten en de stuklijst.
toast-device-unplaced-no-part = { $p0 } heeft nu geen symbool op een pagina; het blijft in de navigator Apparaten.
toast-deleted-wires = { $p0 ->
        [one] 1 draad die op de verwijderde symbolen eindigde, is mee verwijderd
       *[other] { $p0 } draden die op de verwijderde symbolen eindigden, zijn mee verwijderd
    }.
toast-cut-terminals = { $p0 ->
        [one] 1 klem geknipt
       *[other] { $p0 } klemmen geknipt
    }.
toast-cut-symbols = { $p0 ->
        [one] 1 symbool geknipt
       *[other] { $p0 } symbolen geknipt
    }.
toast-cut-wires = { $p0 ->
        [one] 1 draad die erop eindigde, is mee verwijderd; plakken houdt alleen de draden tussen de geknipte delen
       *[other] { $p0 } draden die erop eindigden, zijn mee verwijderd; plakken houdt alleen de draden tussen de geknipte delen
    }.
toast-terminals-undrawn = { $p0 ->
        [one] 1 klem is nergens meer getekend; hij blijft in het project en in de lijsten
       *[other] { $p0 } klemmen zijn nergens meer getekend; ze blijven in het project en in de lijsten
    }.
toast-devices-unplaced = { $p0 } hebben nu geen symbool op een pagina; ze blijven in de navigator Apparaten en de stuklijst.
toast-devices-unplaced-no-part = { $p0 } hebben nu geen symbool op een pagina; ze blijven in de navigator Apparaten.
toast-paste-ip-continues = Het geplakte onderbrekingspunt { $p0 } houdt zijn naam en zet daarom { $p0 } voort bij { $p1 }.
toast-paste-fitted = { $p0 ->
        [one] Het plakken is aangepast aan het raster van { $p1 } van deze pagina: één item is erop gezet en de draden zijn opnieuw getekend. Ongedaan maken zet alles in één stap terug
       *[other] Het plakken is aangepast aan het raster van { $p1 } van deze pagina: { $p0 } items zijn erop gezet en hun draden zijn opnieuw getekend. Ongedaan maken zet alles in één stap terug
    }.
toast-page-grid-set = Pagina { $p0 } heeft nu het raster van { $p1 }.
toast-page-grid-fitted-none = Pagina { $p0 } heeft nu het raster van { $p1 }: elk item stond er al op, en de draden zijn opnieuw getekend. Ongedaan maken zet alles in één stap terug.
toast-page-grid-fitted = Pagina { $p0 } heeft nu het raster van { $p1 }: { $p2 } items zijn erop gezet en de draden zijn opnieuw getekend. Ongedaan maken zet alles in één stap terug.
toast-wires-unrouted = { $p0 ->
        [one] Eén draad vond geen weg en is als dunne gestreepte lijn getekend: zet de elementen verder uit elkaar en kies Routeren
       *[other] { $p0 } draden vonden geen weg en zijn als dunne gestreepte lijnen getekend: zet de elementen verder uit elkaar en kies Routeren
    }.
toast-device-retagged = { $p0 } (nu { $p1 })
toast-placed-connected = { $p0 ->
        [one] { $p1 } en { $p3 } verbonden met { $p4 }
       *[other] { $p1 } en { $p2 } verbonden: { $p3 } met { $p4 } en nog { $p5 }
    }.
toast-place-without-wires = Alt+klik of { $p0 } plaatst zonder te verbinden.
toast-placed = { $p0 } geplaatst op { $p1 }.
toast-placed-no-cell = { $p0 } geplaatst.
toast-placed-boxes = Geplaatst als { $p0 } kaders, één per businterface.
toast-retagged = { $p0 }: een apparaat krijgt zijn kenmerk van de pagina met zijn eigen kader, niet van een buspagina, die zijn interfaces toont.
toast-paste-retagged = { $p0 }: een kenmerk volgt de pagina waarop zijn apparaat getekend staat, en het plakken zette de geknipte apparaten op deze terug.
toast-paste-notices = Het plakken: { $p0 }.
toast-paste-moved-terminals = { $p0 } klemmen van { $p1 } verplaatst naar { $p2 }.
toast-paste-moved-terminal = 1 klem van { $p1 } verplaatst naar { $p2 }.
toast-paste-moved = { $p0 } symbolen verplaatst naar { $p2 }.
toast-paste-moved-symbol = 1 symbool verplaatst naar { $p2 }.
toast-placed-in-wire = { $p0 }, verbonden in de draad waarop het is neergezet.
toast-placed-moved = { $p1 } rasterstappen naar rechts: { $p0 } heeft een klem op het klikpunt.
toast-placed-and-moved = { $p0 }, { $p1 }.
toast-ip-connected = { $p0 } geplaatst op { $p1 }. Alt+klik of { $p2 } plaatst het los.
toast-ip-external-connected = Extern punt { $p0 } geplaatst op { $p1 }. Alt+klik of { $p2 } plaatst het los.
toast-ip-placed-source = { $p0 } op { $p1 } gaat verder op { $p2 }.
toast-ip-placed-source-no-cell = { $p0 } gaat verder op { $p2 }.
toast-ip-placed-source-lone = { $p0 } op { $p1 } heeft nog geen doel: plaats er een waar de potentiaal aankomt.
toast-ip-placed-source-lone-no-cell = { $p0 } heeft nog geen doel: plaats er een.
toast-ip-placed-target = { $p0 } op { $p1 } loopt door vanaf { $p2 }.
toast-ip-placed-target-no-cell = { $p0 } loopt door vanaf { $p2 }.
toast-ip-placed-target-lone = { $p0 } op { $p1 } heeft nog geen bron: plaats er een waar de potentiaal vertrekt, of rechtsklik erop (Voeding komt hier binnen…).
toast-ip-placed-target-lone-no-cell = { $p0 } heeft nog geen bron: plaats er een, of rechtsklik (Voeding komt hier binnen…).
toast-undo = Ongedaan maken
toast-close = Sluiten
# Een toast die opnieuw zegt wat de zichtbare toast van zijn sleutel zegt, met het aantal keren op rij.
note-said-again = { $p0 } ({ $p1 }×)
toast-open = Openen
toast-export-cancelled = Export geannuleerd; geen bestand geschreven.
toast-export-failed = Exporteren mislukt: { $p0 }; controleer of de map beschrijfbaar is en het bestand niet in een ander programma open is.
toast-export-internal = Exporteren mislukt: een interne fout.
toast-recovered = { $p0 ->
        [one] 1 wijziging hersteld; sla op om haar te bewaren
       *[other] { $p0 } wijzigingen hersteld; sla op om ze te bewaren
    }.
toast-bridged = { $p0 ->
        [one] 1 brug toegevoegd tussen { $p1 } klemmen
       *[other] { $p0 } bruggen toegevoegd tussen { $p1 } klemmen
    }.
toast-check = Controle: { $p0 }.
toast-marker-count = { $p0 ->
        [one] 1 draad draagt een aanduiding
       *[other] { $p0 } draden dragen een aanduiding
    }.
job-kind-pdf = PDF-export
job-kind-print = Afdrukset
toast-print-printed = { $p0 } naar de printer gestuurd.
toast-print-opened = { $p0 } geopend in uw PDF-viewer.
toast-print-fallback-printer = Afdrukken via de afdrukdialoog is mislukt, dus { $p0 } is naar de standaardprinter gegaan.
toast-print-fallback-viewer = Afdrukken kon niet starten, dus { $p0 } is in uw PDF-viewer geopend om daar af te drukken.
toast-print-failed = Afdrukken mislukt: { $p0 }. Exporteer een PDF en druk die af vanuit uw PDF-viewer.
toast-print-cancel-late-printer = De printer had { $p0 } al toen u annuleerde; annuleer het in de afdrukwachtrij van het systeem.
toast-print-cancel-late-viewer = { $p0 } was al geopend in uw PDF-viewer toen u annuleerde; sluit het daar.
job-cause-in-use = het bestand is in gebruik
job-cause-no-folder = de map bestaat niet
job-cause-no-file = het bestand bestaat niet
job-cause-denied = toegang tot de map of het bestand is geweigerd
job-cause-full = de schijf is vol
job-cause-internal = een interne fout
job-phase-building = pagina's opbouwen
job-phase-drawing = pagina's tekenen
job-phase-writing = het bestand schrijven
job-as-of = vanaf revisie { $p0 }
job-progress = pagina { $p0 } van { $p1 }
job-elapsed = { $p0 } s
job-cancel = Annuleren
job-done = { $p0 ->
        [one] 1 pagina, { $p1 } KiB
       *[other] { $p0 } pagina's, { $p1 } KiB
    }
job-now = rev { $p0 } geëxporteerd; het model staat nu op rev { $p1 }
job-cancelled = geannuleerd; geen bestand geschreven
job-failed = mislukt: { $p0 }
notifications-title = Notificaties
notifications-empty = Geen notificaties
recovery-title = Niet-opgeslagen wijzigingen herstellen?
recovery-text = { $p0 } heeft niet-opgeslagen wijzigingen uit een sessie die onverwacht is beëindigd.
recovery-count = Niet-opgeslagen wijzigingen: { $p0 }
recovery-last-change = Laatste wijziging: { $p0 }
recovery-last-change-utc = { $p0 } UTC
elapsed-just-now = zojuist
elapsed-minutes = { $p0 ->
        [one] 1 minuut geleden
       *[other] { $p0 } minuten geleden
    }
elapsed-hours = { $p0 ->
        [one] 1 uur geleden
       *[other] { $p0 } uur geleden
    }
elapsed-days = { $p0 ->
        [one] 1 dag geleden
       *[other] { $p0 } dagen geleden
    }
recovery-moved = Het project is sindsdien op schijf gewijzigd.
recovery-discard = Verwerpen…
recovery-failed-moved = het project is op dezelfde plaatsen op schijf gewijzigd; herstel vraagt om het samenvoegpaneel
recovery-failed-replay = het journaal kon niet opnieuw worden afgespeeld
recovery-failed = Herstellen mislukt: { $p0 }
toast-dismiss = Sluiten
toast-name = Notificatie
banner-part-imported = 1 artikel geïmporteerd uit { $p2 } ({ $p1 } overgeslagen).
banner-parts-imported = { $p0 } artikelen geïmporteerd uit { $p2 } ({ $p1 } overgeslagen).
banner-parts-none = Geen nieuwe artikelen in { $p0 }: { $p1 } al in het project, { $p2 } geweigerd.
toast-parts-read = { $p0 } gelezen: { $p1 } nieuw, { $p2 } al in het project, { $p3 } geweigerd.
announce-copied = { $p0 } records gekopieerd
banner-paste-refused = Niet geplakt: { $p0 }
notices-title = Kennisgevingen van derden
notices-generated-missing = De kennisgevingen van de Rust-crates worden voor elke release gegenereerd (THIRD-PARTY-NOTICES.txt naast de applicatie); deze build heeft er geen.
toast-valve-added = { $p0 } toegevoegd op plaats { $p1 } van { $p2 }; klik waar het symbool komt.
toast-seat-valve = { $p0 } is nu het ventiel op plaats { $p1 } van { $p2 }.
toast-seat-plate = { $p0 } is nu een voedingsplaat van { $p1 }.
toast-seat-full = Het ventieleiland heeft geen vrije ventielplaats: { $p0 } blijft een los ventiel.
toast-seat-own = { $p0 } is geen ventiel voor een ventieleiland (het heeft een eigen spoel die met draden wordt aangesloten, of is geen ventiel): het blijft een apparaat op zich.
toast-position-held = Positie { $p0 } van { $p1 } bevat { $p2 }: kies Omwisselen om de twee om te wisselen.
toast-solenoid-draw = Klik waar magneetspoel { $p0 } van { $p1 } komt op pagina { $p2 }.
toast-zoom-way-back = Ver ingezoomd: { $p0 }, of Pagina passend rechtsonder op de pagina, toont de hele pagina weer.
toast-plug-draw = Klik waar de stekker van { $p0 } komt op pagina { $p1 }.
toast-ports-plugged = Blindstoppen op de open werkpoorten van { $p0 } gezet.
toast-bind-off-card = { $p0 } ligt niet op de module { $p1 }: bedraad { $p0 } eerst met een pen van { $p1 }; het kanaal koppelt de pen van de module.
toast-bind-off-card-untagged = Het aansluitpunt ligt niet op de module { $p0 }: een kanaal koppelt een pen van zijn module of de spoel van een ventiel erop.
toast-bind-wired-pins = { $p0 } is met { $p2 } pennen van { $p1 } bedraad ({ $p3 }): zet het kanaal op de pen van de module die u bedoelt.
toast-bind-no-position = { $p0 } kan niet op het ventieleiland { $p1 }: er is geen ventielplaats vrij, of { $p0 } zit op een andere samenstelling.
toast-bind-done = { $p0 } gekoppeld aan { $p1 }.
toast-bind-wired = { $p0 } gekoppeld aan { $p1 }, waarop { $p2 } is aangesloten.
toast-bind-attached = { $p0 } aan { $p1 } bevestigd op positie { $p2 } en { $p3 } gekoppeld aan { $p4 }.
toast-cards-attached = { $p0 } aan { $p1 } bevestigd.
toast-card-waits = { $p0 } wacht op een station: sleep hem in het PLC-paneel op zijn station, of klik hem daar onder Modules zonder station met rechts aan en kies Bevestigen aan.
toast-card-waits-cpu = { $p0 } wacht op een station: maak er een met Nieuw station…, en de module gaat in zijn rek.
toast-plc-export = { $p0 } geëxporteerd: { $p1 }, { $p2 }, { $p3 }, { $p4 }.
toast-plc-import = { $p0 } geïmporteerd.
toast-medium-new = Medium { $p0 } toegevoegd.
toast-line-medium = De slang voert nu { $p0 }.
toast-fluid-settings = De standaardslang is nu { $p0 }: elke slang zonder eigen maat neemt haar over, ook de slangen die al getekend zijn.
toast-fluid-settings-no-material = De standaardslang is nu { $p0 }, zonder materiaal: elke slang zonder eigen maat neemt haar over, ook de slangen die al getekend zijn; geef haar een materiaal om hun specificatie compleet te maken.
toast-fluid-settings-no-size = Het project heeft nu geen standaardmaat voor slangen: elke slang neemt die van haar medium of haar eigen.
toast-bind-displaced = ; { $p0 } was daar gekoppeld en is nu ontkoppeld
job-kind-table = Tabelexport
job-kind-import = Tekeningimport
job-kind-read = Importbestand lezen
job-done-table = { $p0 } KiB
job-kind-package = Pakket
job-done-package = { $p0 ->
        [one] 1 bestand
       *[other] { $p0 } bestanden
    }
toast-package-draft = { $p0 ->
        [one] Het conceptpakket { $p1 } is geschreven: 1 bestand
       *[other] Het conceptpakket { $p1 } is geschreven: { $p0 } bestanden
    }.
toast-package = { $p0 ->
        [one] Het pakket { $p1 } is geschreven: 1 bestand
       *[other] Het pakket { $p1 } is geschreven: { $p0 } bestanden
    }.
toast-export-table = { $p0 } geëxporteerd: { $p1 } KiB, in { $p2 }.
toast-export-table-again = { $p0 } opnieuw geëxporteerd: { $p1 } KiB, in { $p2 }.
toast-table-imported = { $p0 ->
        [one] 1 gewijzigde cel geïmporteerd uit { $p1 }
       *[other] { $p0 } gewijzigde cellen geïmporteerd uit { $p1 }
    }.
toast-check-qualified = Controle · { $p1 }: { $p0 }.
toast-section-new = Sectie { $p0 } { $p1 } gemaakt, folio's { $p2 } tot en met { $p3 }. Een nieuwe pagina met een folio in dat bereik komt erin.
toast-page-new-section = Pagina { $p0 } is gemaakt: { $p1 }.
history-header = { $p0 ->
        [one] { $p0 } stap · cursor op { $p1 }
       *[other] { $p0 } stappen · cursor op { $p1 }
    }
history-empty = Nog niets om ongedaan te maken
history-not-undoable = niet ongedaan te maken
history-changes-title = Wijzigingen van stap { $p0 }
history-redo = ongedaan gemaakt
# The text protocol's answer to a step that changed the project: the step's title and its pages.
history-changed-on = { $p0 } op { $p1 }
# Wat Ongedaan maken of Opnieuw terugnam of opnieuw deed, en waar.
history-undone = Ongedaan gemaakt: { $p0 }
history-redone = Opnieuw gedaan: { $p0 }
note-action-go-to-page = Naar pagina
note-action-show-messages = Meldingen tonen
note-action-wheel-scrolls = Wiel schuift de pagina
note-action-route-his = Ook deze routeren…
history-redo-to = Tot hier opnieuw
history-origin-gui = met de hand
history-origin-from = uit { $p0 }
history-origin-external = gewijzigd op schijf
history-origin-import = import
history-origin-merge = samenvoeging
history-origin-fix = snelle oplossing { $p0 }
toast-pages-copied = { $p0 ->
        [one] 1 pagina gekopieerd: { $p1 }
       *[other] { $p0 } pagina's gekopieerd: { $p1 }
    }.
toast-pages-copied-bus = De kopieën van { $p0 } zitten nog op geen bus: teken hun buslijn op een buspagina en geef elk zijn adres.
history-header-kept = oudste bewaard: stap { $p0 } · { $p1 } MB van { $p2 } MB
note-macro-saved = { $p0 } { $p1 } opgeslagen in { $p2 }
announce-variable = { $p0 } is een variabele van de volgende macro
toast-part-new = Artikel { $p0 } staat in het project.
toast-part-new-library = Artikel { $p0 } staat in het project en in { $p1 }.
banner-keymap-refused = Het toetsenindelingsbestand is geweigerd: { $p0 }. De voorinstelling { $p1 } geldt; het bestand is bewaard als { $p2 }.
banner-keymap-write = De toetsenindeling kon niet worden geschreven.
banner-keymap-unread = Het toetsenindelingsbestand kon niet worden gelezen ({ $p0 }); de voorinstelling { $p1 } geldt en het bestand is gelaten zoals het is.
banner-keymap-refused-kept = Het toetsenindelingsbestand is geweigerd: { $p0 }. De voorinstelling { $p1 } geldt; het bestand is gelaten zoals het is, omdat het niet opzij kon worden gekopieerd.
banner-keymap-kept = De toetsenindeling geldt voor deze sessie, maar is niet geschreven: keymap.toml is gelaten zoals het is.
toast-revision-approved = Revisie { $p0 } goedgekeurd; opgeslagen.
toast-revision-released = Revisie { $p0 } vrijgegeven als { $p1 }.
toast-revision-withdrawn = Revisie { $p0 } ingetrokken.
toast-revision-author = De { $p1 } is nu de auteur van { $p0 }.
toast-revision-discarded = De onvolledige vrijgave van { $p0 } verworpen.
toast-release-troubles = De vrijgave liet onderhoudswaarschuwingen achter ({ $p0 }): zie Meldingen.
toast-bus-line-no-cable = Buslijn getekend: er is nog geen kabel.
note-action-choose-cable = Kabel kiezen…
toast-locate-devices = Locatie { $p1 } gegeven aan { $p0 }; Ongeplaatste apparaten toont nu wat op de plaat moet.
toast-legend-tags = Legenda toegevoegd. De apparaten tonen hun kenmerken, niet de nummers die de legenda opsomt.
toast-devices-location = { $p0 ->
        [one] Het apparaat heeft nu de locatie { $p1 }
       *[other] { $p0 } apparaten hebben nu de locatie { $p1 }
    }.
toast-auto-fill = { $p0 } geplaatst op { $p1 }.
toast-auto-fill-left = { $p0 } geplaatst op { $p1 }; { $p2 }.
toast-enclosure-new = Kast { $p0 } bij { $p1 } gemaakt.
toast-enclosure-new-shown = Kast { $p0 } bij { $p1 } gemaakt; de pagina toont de montageplaat.
toast-enclosure-drawn = { $p0 } van { $p1 } getekend op pagina { $p2 }.
toast-enclosure-new-page = Kast { $p0 } bij { $p1 } gemaakt en getekend op de nieuwe paneellayoutpagina { $p2 }.
toast-convert = Omzetten in verbinding
toast-show-block = Rapportblok tonen
note-action-select-unlisted = Selecteren
note-action-connect = Verbinden
note-action-remove-undrawn = { $p0 ->
        [one] Uit het project verwijderen
       *[other] Uit het project verwijderen
    }
note-action-keep-tag = { $p0 } houden
note-action-keep-tags = De oude kenmerken houden
toast-connect-offer = { $p0 } staat tegenover { $p1 }, verder weg of over andere draden, en is dus niet verbonden: Verbinden legt de draad.
toast-show-changes = Wijzigingen tonen
toast-external = Externe wijziging toegepast: { $p0 }.
toast-external-folios = Externe wijziging toegepast: { $p0 } op folio's { $p1 }.
toast-external-folio = Externe wijziging toegepast: { $p0 } op folio { $p1 }.
toast-external-git = {" "}(git { $p0 })
banner-external-collisions = Het project is op schijf gewijzigd en de wijziging botst met uw bewerkingen. Opslaan wacht tot u voor elk veld uw versie of die op schijf kiest.
banner-external-unreadable = Het project op schijf is gewijzigd en kan niet worden gelezen; uw bewerkingen blijven bewaard. Sla een kopie op, of wacht tot de schijf compleet is.
banner-external-choose = Kiezen…
toast-external-kept = Uw versie is behouden: { $p0 }.
toast-external-same = De wijziging op schijf is gelijk aan uw versie: er is niets over te nemen.
banner-older-format = Dit project heeft een ouder formaat; opslaan zet het om.
banner-cloud = Dit project staat in een cloudgesynchroniseerde map ({ $p0 }). Twee personen kunnen het binnen de synchronisatietijd openen; gebruik git of een gedeelde netwerkmap voor teams.
banner-cloud-other = Dit project staat in een cloudgesynchroniseerde map. Twee personen kunnen het binnen de synchronisatietijd openen; gebruik git of een gedeelde netwerkmap voor teams.
toast-ewz = { $p0 } geëxporteerd.
toast-ewz-unsaved = { $p0 } geëxporteerd met de opgeslagen toestand; de niet-opgeslagen wijzigingen zitten er niet in.
toast-git-setup = Samenvoegdriver ingesteld.
banner-reimported = { $p0 } opnieuw geïmporteerd.

# The banner while the import job runs.
banner-importing = { $p0 } wordt geïmporteerd…
toast-cable-new = Kabel { $p0 } gemaakt met { $p1 }.
toast-cable-new-cores = { $p0 ->
        [one] 1 ader
       *[other] { $p0 } aders
    }
toast-cores-connected = Draden verbonden met de aders van { $p1 }: { $p0 }.
toast-cable-wires = Kabel { $p0 } gemaakt van { $p1 }.
toast-cable-wires-changed = Kabel { $p0 } bevat nu { $p1 }.
toast-bus-cable = Kabel { $p0 } draagt { $p1 }.
toast-report-block-new = Rapportblok { $p0 } toegevoegd vanaf folio { $p1 }.
toast-report-block-empty = Rapportblok { $p0 } toegevoegd vanaf folio { $p1 }; het heeft nog niets te tonen en maakt dus geen pagina. { $p2 }.
toast-report-block-had = Rapportblok { $p0 } toegevoegd vanaf folio { $p1 }. De set had er al een op pagina { $p2 }: ga ernaartoe, of maak dit ongedaan.
toast-parts-unlisted = { $p0 ->
        [one] 1 apparaat heeft geen artikel en staat niet in de stuklijst. Selecteer het; Bewerken ▸ Artikel kiezen… geeft het er een, of Type nog onbekend… zet het met een omschrijving in de lijst tot het bekend is
       *[other] { $p0 } apparaten hebben geen artikel en staan niet in de stuklijst. Selecteer ze; Bewerken ▸ Artikel kiezen… geeft ze er een, of Type nog onbekend… zet ze met een omschrijving in de lijst tot het bekend is
    }.
toast-parts-list-opened = De stuklijst van de set op pagina { $p0 } geopend; er komt geen tweede. Rapport… op het tabblad Uitvoer voegt nog een lijst toe.
toast-list-opened = De { $p0 } geopend op pagina { $p1 }.
toast-rework-written = De wijzigingslijst geschreven naar { $p0 }.
toast-labels-written = { $p0 ->
        [one] 1 labelbestand naar { $p1 } geschreven
       *[other] { $p0 } labelbestanden naar { $p1 } geschreven
    }.
toast-label-columns = De labelbestanden krijgen de gekozen kolommen en vorm.
toast-back-to-editor = Terug naar de tekening
toast-route-done = { $p0 ->
        [one] 1 draad gerouteerd: { $p1 }
       *[other] { $p0 } draden gerouteerd: { $p1 }
    }.
toast-route-applied = { $p0 ->
        [one] 1 draad op { $p1 } gerouteerd
       *[other] { $p0 } draden op { $p1 } gerouteerd
    }.
toast-route-nothing = Niets te routeren: geen draad zou beter lopen.
toast-wire-error = De draad geeft een fout: { $p0 }.
toast-route-done-selected = { $p0 ->
        [one] 1 geselecteerde draad gerouteerd: { $p1 }
       *[other] { $p0 } geselecteerde draden gerouteerd: { $p1 }
    }.
toast-route-nothing-selected = { $p0 ->
        [one] Niets te routeren in het ene geselecteerde element: geen draad zou beter lopen
       *[other] Niets te routeren onder de { $p0 } geselecteerde elementen: geen draad zou beter lopen
    }.
toast-route-kept = { $p0 ->
        [one] Niets veranderd: 1 draad houdt de route die hij heeft
       *[other] Niets veranderd: { $p0 } draden houden de route die ze hebben
    }.
toast-lists-frame = Elke lijst van het project, ook de inhoudsopgave, wordt nu afgedrukt op { $p0 }; de kaders van de getekende pagina's en de sectiescheidingen blijven zoals ze waren.
toast-drawing-imported = { $p0 } geïmporteerd als pagina { $p1 }.
toast-underlay-placed = { $p0 } onder pagina { $p1 } gelegd.
toast-eplan-parts-added = { $p0 }: { $p1 } artikelen aan je artikelbibliotheek toegevoegd, { $p2 } bijgewerkt, { $p3 } niet geïmporteerd.
toast-eplan-report-imported = { $p0 } geïmporteerd als nieuw project met { $p1 } apparaten.
# het project veranderde terwijl een rapport werd gelezen, en Annuleren hield het open.
toast-report-read-dropped = Het rapport is niet geïmporteerd: het project veranderde terwijl het werd gelezen en blijft open; importeer het rapport opnieuw om zijn project te maken.
toast-plc-freeze = { $p0 } van { $p1 } vastgezet; reservekanalen houden hun automatische adres.
toast-plc-readdress = Opnieuw geadresseerd: { $p0 } verschoven.
toast-placed-positions-empty = { $p0 }: { $p1 } posities, alle leeg.
toast-placed-positions = { $p0 }: { $p1 } posities, waarvan { $p2 } leeg.
toast-terminal-undrawn = Het ventieleiland { $p0 } van het project staat nog op geen fluïdumpagina: Fluïdum ▸ Ventieleiland tekent het hier.
toast-terminal-draw = Klik waar { $p0 } komt op pagina { $p1 }: het wordt als zijn posities getekend, met zijn ventielen in hun vakken.
toast-terminal-valves-drawn = De ventielen van { $p0 } staan in hun vakken, de voedingspoorten 1, 3 en 5 aan de linkerkant.
toast-fluid-page-made = Fluïdumpagina { $p0 } is ervoor gemaakt.
toast-card-joined = { $p0 } is in slot { $p2 } van { $p1 } gezet.
toast-show-folder = In map tonen
toast-tutorial-here = Het oefenproject van de zelfstudie staat onder Tutorials in uw projectenmap.
toast-tutorial-second-window = Het oefenproject van de zelfstudie staat onder Tutorials in uw projectenmap; het opent in een tweede venster en uw project blijft hier open.
banner-second-window-failed = Het tweede venster is niet gestart: open het oefenproject met Bestand ▸ Recent geopend.
toast-layout-overruns = Op 1:{ $p0 } loopt de layout over het tekenvlak van het kader. Layout aan het blad aanpassen tekent hem op 1:{ $p1 }, gecentreerd.
toast-legend-own-page = De legenda past niet heel naast de layout en krijgt daarom een eigen pagina: { $p0 }.
toast-legend-moved = De legenda staat nu op een eigen pagina: { $p0 }.
toast-kept-in-place = { $p0 ->
        [one] Op zijn plaats gehouden: { $p1 }. Schikken en routeren zet de andere symbolen eromheen; zelf verplaatsen kan nog steeds
       *[other] Op hun plaats gehouden: { $p1 }. Schikken en routeren zet de andere symbolen eromheen; zelf verplaatsen kan nog steeds
    }.
toast-let-go = Niet langer op de plaats gehouden: { $p0 }.
toast-plc-joined = { $p0 } hoort nu bij { $p1 }.
toast-guide-page = De pagina van de handleiding: { $p0 }.
toast-fix-all = { $p0 } bevindingen opgelost.
toast-fix-all-fix-mark-cp-unused = { $p0 } pennen als niet gebruikt gemarkeerd.
toast-fix-all-left = { $p0 } oplossingen zijn weggelaten: wat ze veranderen, is sinds de controle veranderd.
toast-card-redrawn = { $p0 } is in één rij getekend: Schikken en routeren tekent zijn draden naar de nieuwe pinnen.
toast-card-terminals-faced = De klemmen die met { $p0 } verbonden zijn, hebben de draad naar de kaart nu op hun pin die naar de kaart wijst: Schikken en routeren tekent de draden.
toast-rack-own-pages = Elke kaart staat op een eigen pagina: { $p0 }.
toast-module-moved-new-page = { $p0 } staat op de nieuwe pagina { $p1 }.
banner-template-saved = Het sjabloon "{ $p0 }" is gemaakt en staat in de lijst van sjablonen.
banner-template-saved-used = Het sjabloon "{ $p0 }" is gemaakt. Nieuwe projecten beginnen ermee.
banner-wizard-done-listed = Het sjabloon is geschreven naar { $p0 } en staat in de lijst van sjablonen.
banner-check-level-set = Het controleniveau is nu "{ $p0 }".
toast-arrange-applied = { $p0 } geschikt: { $p1 }, { $p2 }.
toast-arrange-nothing = Niets te schikken: de pagina staat al zo goed als de regels toelaten.
toast-arrange-none-left = Er blijft niets over om toe te passen: elke wijziging van dit voorbeeld is uitgevinkt.
toast-list-columns = De lijst { $p0 } drukt haar kolommen af zoals gekozen.
toast-legend-columns = De legenda op { $p0 } drukt haar kolommen af zoals gekozen.
toast-list-rows = { $p0 ->
        [one] De lijst { $p1 } heeft 1 eigen rij
       *[other] De lijst { $p1 } heeft { $p0 } eigen rijen
    }.
toast-list-rows-none = De lijst { $p0 } heeft geen eigen rijen.
toast-drilling-written = { $p0 ->
        [one] 1 bestand geschreven naar { $p1 }
       *[other] { $p0 } bestanden geschreven naar { $p1 }
    }.
toast-cabinet-routed = { $p0 ->
        [one] 1 draad van { $p1 } gerouteerd
       *[other] { $p0 } draden van { $p1 } gerouteerd
    }.
toast-cabinet-gone = { $p0 ->
        [one] 1 loop verwijderd van een draad die { $p1 } niet meer legt
       *[other] { $p0 } lopen verwijderd van draden die { $p1 } niet meer legt
    }.
toast-cabinet-all-hold = Elke draad van { $p0 } heeft zijn loop.
toast-layout-turned = { $p0 } gedraaid.
toast-layout-moved = { $p0 } verplaatst.
toast-ways-stale = { $p0 ->
        [one] De loop van 1 draad klopt niet meer
       *[other] De lopen van { $p0 } draden kloppen niet meer
    }.
