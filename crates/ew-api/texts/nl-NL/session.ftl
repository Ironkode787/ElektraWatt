# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: GPL-3.0-or-later

# The texts of `ew-api` in nl-NL: what the session refuses or names: the edits of a table, the part editor, the find
# pane's fields, the symbol importers, the libraries' states. Every key of the en-GB file of the same name, with the
# same arguments; a message missing here falls back to en-GB. Product and file names, rule IDs, command names, key
# caps, identifiers and units are not translated.

error-part-length = "{ $p0 }" is geen lengte: geef millimeters, zoals 45 of 22,5.
error-part-hole-incomplete = Gat { $p0 } heeft zijn X, zijn Y, zijn soort en zijn maat nodig.
error-part-cell-needed = Rij { $p0 } heeft hier een waarde nodig: alleen een waarde die leeg mag blijven kan worden weggelaten.
error-parts-written-partly = { $p0 } opgeslagen; { $p1 } niet opgeslagen, omdat hun bibliotheek niet geschreven kon worden: { $p2 }
error-part-gland-incomplete = Een wartel heeft zijn schroefdraad nodig en de kleinste en grootste kabel die hij afdicht.
error-part-gland-range = De grootste kabel die een wartel afdicht kan niet kleiner zijn dan de kleinste.
error-part-hole-off = Gat { $p0 } op { $p1 }, { $p2 } mm ligt buiten de voorkant, die { $p3 } × { $p4 } mm is: het midden wordt gemeten vanaf de linkeronderhoek van de voorkant.
error-part-front-round = Een ronde voorkant heeft dezelfde breedte en hoogte nodig: geef beide bij Afmetingen en montage, of kies Rechthoek.
error-part-outline-locked = Een artikel uit een bibliotheek houdt zijn vooraanzicht: Contour importeren… wijzigt een eigen artikel van het project of een nieuw artikel.
error-part-outline-unreadable = { $p0 } is niet te lezen: { $p1 }
error-part-accessory = Geen artikel heeft nummer { $p0 }
error-stock-kind = { $p0 } is geen draad, adereindhuls, merker of kabelwartel: de voorraad houdt alleen die
error-stock-gone = Het artikel staat niet meer in zijn bibliotheek: het kan niet in de voorraad
error-part-accessory-kind = { $p0 } is geen toebehoren: een draad, adereindhuls of merker wordt van de draden geteld. Voeg het in plaats daarvan toe aan de werkplaatsvoorraad, op zijn blad in Artikelen.
reason-no-terminal-def = Het project heeft geen klemdefinitie om in te voegen
reason-no-accessory-part = Het project heeft geen artikel voor dit toebehoren; sluit er eerst een in
refusal-strip-nothing = niets te bewerken
refusal-strip-no-accessory = de klemmenstrook heeft geen toebehoren { $p0 }
refusal-strip-accessory-part = een toebehoren heeft een artikel nodig
refusal-strip-accessory-kind = `{ $p0 }` is niet een van { $p1 }
refusal-strip-gesture-alone = invoegen of verplaatsen is een handeling op zich
refusal-strip-no-part = het project heeft geen artikel `{ $p0 }` van soort { $p1 }
refusal-strip-no-part-either = het project heeft geen artikel `{ $p0 }` van soort { $p1 } of { $p2 }
refusal-strip-not-strip = het apparaat is geen klemmenstrook
refusal-strip-not-terminal = de rij is geen klem
refusal-strip-terminal-strip = de klemmenstrook van de klem is geen klemmenstrook
refusal-strip-insert-count = voeg ten minste één klem in
refusal-strip-designation-one = een aanduiding benoemt één klem
table-accessory-row = rij met toebehoren
table-pos-move = rijen verplaatsen met Alt+Up en Alt+Down
table-no-bridge = de klem heeft geen brug
table-bridge-how = markeer de rijen van twee of meer klemmen van de strook en druk op F2 in deze kolom, of kies Klemmen bruggen in het menu van een klem op de pagina
reason-swap-two-cores = Markeer twee aders van één kabel
reason-swap-cores-unwired = Geen van beide aders heeft een verbinding
# A damaged copy written from its library, with the version written.
refusal-words-copy-at-version = { $p0 } in versie { $p1 }
refusal-words-part-kind = { $p0 }
refusal-words-cp-other-page = een aansluitpunt van een andere pagina
# The property grid's own parse errors: a typed value that is not one of its field's type.
refusal-input-scale = `{ $p0 }` is geen schaal; typ 1:n met n een geheel getal van 1 tot en met 4294967295
refusal-input-decimal = `{ $p0 }` is geen decimaal getal
refusal-input-awg = `{ $p0 }` is geen AWG-maat
refusal-input-mm = `{ $p0 }` is geen lengte: typ millimeters (ten hoogste 6 decimalen), of de lengte met haar eenheid, zoals 2 m of 35 cm
refusal-input-rect = `{ $p0 }` is geen rechthoek: typ de plaats en de grootte in millimeters, zoals 20, 15 mm · 60 × 40 mm
refusal-input-colour-alias = `{ $p0 }` is geen kleurcode; bedoelt u { $p1 } { $p2 }? Toegestaan: { $p3 }
refusal-input-colour = `{ $p0 }` is geen kleurcode; toegestaan: { $p1 }
refusal-input-colour-none = geef één of twee kleurcodes
refusal-input-colour-count = een kleur heeft één of twee codes
refusal-input-text-control = een tekst heeft geen regeleinden of stuurtekens
refusal-input-yes-no = `{ $p0 }` is geen ja of nee
refusal-input-whole = `{ $p0 }` is geen geheel getal
refusal-input-bus-address = `{ $p0 }` is geen busadres: typ een stationsnummer, een PROFINET-naam met IP-adres, pos en alias (EtherCAT), port en nummer (IO-Link), of TBD
refusal-input-range = { $p0 } ligt buiten { $p1 }..{ $p2 }
refusal-input-no-unit = `{ $p0 }` heeft geen eenheid; eenheden: { $p1 }
refusal-input-ac-dc = `{ $p0 }`: typ alleen het getal en de eenheid, zoals 24 V; DC of AC is het type van een potentiaal, in te stellen bij haar definitie
refusal-input-no-node = geen structuurknooppunt `{ $p0 }` van dit aspect
refusal-input-form = geen formulier van het project heet "{ $p0 }"; zijn formulieren: { $p1 }
library-form-frame = Kader
library-form-title-block = Titelblok
library-form-cover = Voorblad
library-form-divider = Sectiescheiding
library-form-report = Rapportpagina
library-form-label = Label
library-form-other = Formulier
library-form-named = { $p0 } { $p1 }
library-form-own-landscape = { $p0 } { $p1 } liggend, op { $p2 }
library-form-own-portrait = { $p0 } { $p1 } staand, op { $p2 }
library-form-on-no-page = geen pagina
library-form-numbered = { $p0 } ({ $p1 })
refusal-input-shown = dit veld wordt getoond, niet bewerkt
refusal-input-json = geen strikte JSON: { $p0 }
refusal-input-no-part-copy = het project heeft geen artikelkopie `{ $p0 }`
refusal-input-quantity = `{ $p0 }` is geen hoeveelheid groter dan 0
refusal-input-not-a-field = `{ $p0 }` is geen veld van { $p1 }
refusal-input-plc-address = geef een adres (bijvoorbeeld Q0.1), auto, excluded of tbd
refusal-input-plc-target = `{ $p0 }` noemt geen apparaat met een functie als doel
refusal-ref-not-uuid = `{ $p0 }`: geen UUID
refusal-ref-no-record = `{ $p0 }`: het project heeft zo'n record niet
refusal-ref-section-number = `{ $p0 }`: een sectienummer is een geheel getal
refusal-ref-no-section = Het project heeft nog geen sectie { $p1 }: maak die met Nieuwe sectie… (de mapknop boven de paginalijst, of Pagina ▸ Nieuwe sectie…) en kies hem dan hier.
refusal-ref-no-folio = `{ $p0 }`: geen getekende pagina heeft dit folio
refusal-ref-folio-many = `{ $p0 }`: { $p1 } treffers (meer dan één getekende pagina heeft dit folio); gebruik `id:<uuid>`
refusal-ref-no-tag = `{ $p0 }`: geen apparaat drukt dit kenmerk af
refusal-ref-no-tag-forms = `{ $p0 }`: geen apparaat drukt dit kenmerk af; een verwijzing is een kenmerk (`-101Q1`, een klem als `-X1031:5`), `tag:<tag>`, `section:<number>`, `folio:<label>` of `id:<uuid>`
refusal-ref-ambiguous = `{ $p0 }` is dubbelzinnig: { $p1 } apparaten drukken dit kenmerk af ({ $p2 }); noem er één bij zijn volledige aanduiding of zijn `id:<uuid>`
refusal-ref-candidate = { $p0 } als `id:{ $p1 }`
refusal-ref-not-text = `ref` is een tekst `<kind>:<value>`
refusal-input-plc-slot = `{ $p0 }` is geen slot; geef een getal (3), <rack>.<slot> (1.3) of auto
refusal-input-metres = `{ $p0 }` is geen lengte in meters (bijvoorbeeld 12,5 m)
# A clipboard fragment that does not read: its kind of failure and each line's reason.
refusal-fragment-not = het klembord bevat geen ElektraWatt-fragment
refusal-fragment-newer = gekopieerd uit een nieuwere ElektraWatt, die een nieuwer formaat schrijft ({ $p0 })
refusal-fragment-too-large = het fragment is te groot: { $p0 }
refusal-fragment-line = fragmentregel { $p0 }: { $p1 }
refusal-fragment-bytes = { $p0 } bytes, ten hoogste { $p1 }
refusal-fragment-line-bytes = regel { $p0 } heeft { $p1 } bytes, ten hoogste { $p2 }
refusal-fragment-line-depth = regel { $p0 } is dieper genest dan { $p1 }
refusal-fragment-records = meer dan { $p0 } elementen
refusal-fragment-object = een regel van een fragment is een JSON-object
refusal-fragment-header-line = regel 2 is de kop `$fragment`
refusal-fragment-older = gekopieerd uit een ouder ElektraWatt-formaat; open dat project eerst met deze versie en sla het op
refusal-fragment-kind = een element van de soort { $p0 } wordt niet geplakt (alleen apparaten, functies, symbolen, verbindingen, aftakkingen, potentiaaldefinities, pagina-elementen en structuurkaders)
refusal-fragment-id-twice = de identiteit van het element staat er twee keer
refusal-fragment-unknown-line = een onbekende `$`-regel
refusal-fragment-no-kind = een regel zonder `$k` is geen element
refusal-fragment-no-header = de kop `$fragment` ontbreekt
refusal-fragment-header-object = de kop `$fragment` is een object
refusal-fragment-header-member = de kop heeft geen `{ $p0 }`
refusal-fragment-format-text = `format` is een tekst
refusal-fragment-format-version = `format` { $p0 } is geen versie
refusal-fragment-project = `project` is een UUID
refusal-fragment-grid = `grid` is [num, den] van positieve gehele getallen
refusal-fragment-page-kind = `page_kind` is een tekst
refusal-fragment-anchor = `anchor` is [x, y] in nm
refusal-fragment-ghost = `ghost` is een lijst van [x0, y0, x1, y1]
refusal-fragment-free = `free` is een lijst van nummers van rechthoeken van `ghost`
refusal-fragment-node-object = `$node` is een object
refusal-fragment-node-members = `$node` heeft de leden id, aspect, path en aliases
refusal-fragment-node-needs = `$node` vraagt een identiteit, een aspect en een pad
refusal-fragment-node-aliases = `aliases` is een lijst van teksten
refusal-page-copy-page-twice = de pagina staat er twee keer
refusal-page-copy-grids = de gekopieerde pagina's hebben verschillende verbindingsrasters; kopieer de pagina's van elk raster apart
refusal-page-copy-code = { $p0 } is geen structuurcode
refusal-page-copy-ip-join = de onderbrekingspunten { $p0 } van de kopie zouden samengaan met de punten waarvan ze gekopieerd zijn; geef ze andere namen bij Hernoemen (bijvoorbeeld { $p1 } = { $p1 }.2)
refusal-macro-value-set-none = geen waardenset `{ $p0 }`: de macro heeft er geen
refusal-macro-value-set-unknown = geen waardenset `{ $p0 }`: de macro heeft { $p1 }
# revision.approve, revision.release and project.assets.purge: the refusals of ew-api's own.
refusal-revision-approve-direct = de goedkeuring wordt alleen vastgelegd door Revisie goedkeuren… op het opgeslagen project, dat eerst de vrijgavepoort laat draaien
refusal-revision-release-direct = de vrijgave wordt alleen geschreven door Revisie vrijgeven…, na de vrijgavepoort, het as-built-archief en de git-tag
refusal-purge-read-only = het project is alleen-lezen geopend; opruimen vraagt een project dat open is om te bewerken
refusal-purge-quarantined = het project heeft regels in quarantaine in { $p0 }; herstel ze vóór het opruimen
refusal-purge-unsaved = het project heeft niet-opgeslagen wijzigingen; sla op vóór het opruimen
refusal-purge-recovery = een onverwacht beëindigde sessie liet niet-opgeslagen wijzigingen in haar journaal; herstel of verwerp ze vóór het opruimen
refusal-purge-incomplete = de laatste opslag is onvolledig; die wordt eerst voltooid
refusal-purge-external = het project is op schijf gewijzigd; voeg de wijziging eerst samen
refusal-purge-stale = { $p0 } van de vermelde afbeeldingen kunnen niet meer worden opgeruimd; bekijk het voorbeeld opnieuw; er is niets verwijderd
# The form for one image.
refusal-purge-stale-one = 1 van de vermelde afbeeldingen kan niet meer worden opgeruimd; bekijk het voorbeeld opnieuw; er is niets verwijderd
refusal-purge-unreadable = een bestand van het project kon niet worden gelezen; er is niets verwijderd
refusal-purge-save = de opslag die de afbeeldingen verwijdert is mislukt: { $p0 }
refusal-purge-human-only = opruimen is alleen voor mensen: bevestig het in ElektraWatt of met ew project assets purge op de opdrachtregel; er is niets verwijderd
# Update from library of the damaged library copies: the refusals of ew-api's own.
refusal-update-copies-no-library = geen bibliotheek op deze computer heeft een leesbare versie van { $p0 }; er is niets geschreven
refusal-update-copies-misfit = de versies van de beschadigde kopieën in de bibliotheken passen niet bij het project, dat dan niet zou openen; er is niets geschreven
refusal-update-copies-locked = een andere sessie heeft het project open; er is niets geschreven
refusal-update-copies-read-only = het project kan alleen als alleen-lezen worden geopend, om een andere reden dan zijn beschadigde bibliotheekkopieën; er is niets geschreven
refusal-update-copies-findings = het project heeft andere integriteitsbevindingen, die het alleen-lezen houden; er is niets geschreven
refusal-update-copies-save = het bijwerken kon niet worden weggeschreven: { $p0 }
saveerr-read-only = het project is alleen-lezen
saveerr-lock-lost = de vergrendeling is overgenomen
saveerr-external = het project is op schijf gewijzigd
saveerr-failed = een bestand kon niet worden geschreven; op schijf is niets gewijzigd
saveerr-incomplete = het opslaan is onvolledig en wordt bij het volgende openen voltooid
saveerr-pending = een eerdere opslag is nog onvolledig
saveerr-generation-limit = dit project is zo vaak opgeslagen als zijn formaat toestaat; sla het op in een nieuwe map (Opslaan als…), waarmee de telling opnieuw begint
saveerr-other = het project kon niet worden geschreven
props-position-number = Typ een ventielplaats, zoals 2.
props-position-no-terminal = Het ventiel zit op geen ventieleiland: plaats het in het kader van het eiland, of gebruik Ventiel toevoegen in het menu van het eiland.
props-supply-no-mark = Het punt heeft geen voedingsmarkering: klik er met rechts op en kies eerst Voeding komt hier binnen…
refusal-layout-length = Typ een lengte in millimeters, zoals 158.5.
refusal-variant-no-part = Het apparaat heeft geen artikel met varianten.
refusal-variant-unknown = { $p0 } heeft geen variant { $p1 }; kies er een uit de lijst.
refusal-end-no-holder = Deze draad heeft geen eigen verbinding voor zijn einde: elke verbinding ervan deelt hij met een andere draad.
refusal-end-treatment-unknown = { $p0 } is geen afwerking; kies adereindhuls, twin-adereindhuls, ringkabelschoen of kaal.
refusal-end-part-unknown = Het project bevat geen artikel van een adereindhuls of ringkabelschoen { $p0 }; kies er een uit de lijst.
refusal-gland-part-unknown = Het project bevat geen wartelartikel { $p0 }: kies een wartel met Artikel kiezen….
refusal-layout-length-comma = Typ een lengte in millimeters, zoals 158,5.
refusal-layout-lengths-two = Typ twee lengtes in millimeters, zoals 40, 740.
refusal-layout-hole-row = Die rij noemt geen gat van het patroon.
refusal-layout-lengths-two-comma = Typ twee lengtes in millimeters, zoals 40; 740.
refusal-layout-lengths-three = Typ drie lengtes in millimeters, zoals 10, 20, 0.
refusal-layout-lengths-three-comma = Typ drie lengtes in millimeters, zoals 10; 20; 0.
refusal-layout-length-above-zero = Typ een lengte groter dan 0 mm.
refusal-layout-tube-no-port = De slang eindigt niet op een poort waaraan het medium kan worden gedefinieerd.
refusal-layout-device-no-fluid-port = Het apparaat heeft geen fluïdumpoort waaraan het medium kan worden gedefinieerd.
refusal-layout-medium-choose = Kies een van de media van het project.
refusal-layout-placement-gone = De plaatsing staat niet meer in het project.
refusal-layout-carrier-gone = De drager staat niet meer in het project.
refusal-layout-page-gone = De pagina staat niet meer in het project.
refusal-layout-choose-flip = Kies Normal of Flipped.
refusal-layout-choose-angle = Kies 0°, 90°, 180° of 270°.
refusal-layout-choose-yes-no = Kies ja of nee.
refusal-layout-choose-hinge = Kies Links of Rechts.
refusal-layout-choose-corner = Kies de hoek waarvandaan de gaten gemeten worden, of die van het project.
surface-alike = { $p0 } ({ $p1 })
surface-datum-project = Die van het project: { $p0 }
refusal-layout-choose-labels = Kies Kenmerken, Positienummers, Beide of Kenmerken of nummers.
refusal-layout-choose-label-mode = Kies Afgekort of Volledig.
refusal-layout-row-not-here = Deze rij kan hier niet worden gewijzigd.
refusal-layout-placement-no-part = { $p0 } gebruikt geen artikel { $p1 }; kies een van zijn artikelen.
refusal-layout-no-rail-part = Het project heeft geen railartikel { $p0 }.
refusal-layout-no-duct-part = Het project heeft geen kabelgootartikel { $p0 }.
refusal-layout-surface-gone = Dat montagevlak bestaat niet meer; kies er een uit de lijst.
refusal-layout-no-surface = Het project heeft geen montagevlak { $p0 }; kies er een uit de lijst.
refusal-layout-surface-many = Meer dan één montagevlak heet { $p0 }; kies er een uit de lijst.
refusal-layout-surface-first = Kies eerst het montagevlak.
find-field-function-text = Functietekst
find-field-tag = Kenmerk
find-field-property = Eigenschap
find-field-title = Titel
find-field-description = Beschrijving
find-field-process-tag = Proceskenmerk
find-field-part-number = Artikelnummer
# Why a search cannot run, in words, never the parser's own text.
find-error-regex = De reguliere expressie is niet bruikbaar: controleer haar haakjes, herhalingen en escapetekens, of maak haar eenvoudiger
find-error-too-long = De reguliere expressie is langer dan { $p0 } tekens
find-error-no-page = Geen pagina heeft het folio { $p0 }
table-no-record = geen record van deze soort
table-define-alone = een potentiaal zonder definitie wordt alleen op een eigen rij gedefinieerd
table-define-name-first = Geef de potentiaal eerst een naam
table-reason-derived = afgeleid
table-reason-generated = gegenereerd
table-reason-no-command = niet te wijzigen in deze tabel
table-reason-end-outside = buiten het project gemaakt
table-reason-end-shared = deelt elke verbinding met een andere draad: geen eigen verbinding voor zijn einde
table-reason-property-grid = bewerk in het eigenschappenraster
table-reason-window = bewerk in het venster
table-same-record = hetzelfde record als rij { $p0 }
table-same-bridge = dezelfde brug als rij { $p0 }
table-gesture-edit = bewerken
table-gesture-fill-down = omlaag vullen
table-gesture-pattern-fill = patroon vullen
table-gesture-paste = plakken
table-gesture-clear = wissen
table-row-pages = pagina
table-rows-pages = pagina's
table-row-devices = apparaat
table-rows-devices = apparaten
table-row-connections = verbinding
table-rows-connections = verbindingen
table-row-strips = klem
table-rows-strips = klemmen
table-row-cables = ader
table-rows-cables = aders
table-row-plc = kanaal
table-rows-plc = kanalen
table-row-bus = poort
table-rows-bus = poorten
table-row-tubes = slang
table-rows-tubes = slangen
table-row-media = medium
table-rows-media = media
table-row-potentials = potentiaal
table-rows-potentials = potentialen
table-row-ips = onderbrekingspunt
table-rows-ips = onderbrekingspunten
table-row-structure = structuurknooppunt
table-rows-structure = structuurknooppunten
table-row-parts = artikel
table-rows-parts = artikelen
table-row-blocks = rapportblok
table-rows-blocks = rapportblokken
table-row-unplaced = ongeplaatst apparaat
table-rows-unplaced = ongeplaatste apparaten
table-row-macros = macro
table-rows-macros = macro's
table-row-libraries = bibliotheekitem
table-rows-libraries = bibliotheekitems
table-row-messages = bevinding
table-rows-messages = bevindingen
hint-address-profibus = PROFIBUS DP: het stationsadres, 0 tot en met 126, zoals 3; TBD zolang het open is
hint-address-profinet = PROFINET: de apparaatnaam, een spatie en het IP-adres, zoals plc1 192.168.0.1; TBD zolang het open is
hint-address-ethercat = EtherCAT: pos en de positie na de koppelaar, of alias en de alias, zoals pos 2; TBD zolang het open is
hint-address-io-link = IO-Link: port en de poort van de master, zoals port 3; TBD zolang het open is
hint-address-other = Het adres zoals zijn bus het schrijft; TBD zolang het open is, leeg voor geen
badge-source-missing = Alleen in dit project
badge-deprecated = Verouderd
badge-local-fork = Lokale afsplitsing
reason-part-edit-target = Selecteer een apparaat met een artikel, of een artikel in de navigator Bibliotheken
reason-part-no-manufacturer = Een artikel heeft een fabrikant nodig
reason-part-no-number = Een artikel heeft een artikelnummer nodig
reason-part-unit = Kies de eenheid: piece, m of set
reason-part-unchanged = Niets op te slaan: het artikel is ongewijzigd
error-part-tech-value = { $p0 }: { $p1 }
error-part-tech-twice = { $p0 } is twee keer opgegeven
error-part-manufacturer = Geen fabrikant heet “{ $p0 }”; kies er een uit de lijst
error-part-kind = “{ $p0 }” is geen artikelsoort
error-part-category = Een categorie heeft drie niveaus: categorie/groep/definitie
error-part-value = { $p0 }
error-part-field = Het blad van een artikel heeft geen veld { $p0 }
# een nieuw artikel en zijn businterfaces, gemaakt in de artikeleditor (Bewerken ▸ Nieuw artikel…).
error-part-bus-interface = Een interface heeft een naam nodig, zoals X1 of DP
error-part-bus-system = “{ $p0 }” is geen bussysteem; kies er een uit de lijst
error-part-bus-role = “{ $p0 }” is geen poortrol; kies er een uit de lijst
error-part-bus-ports = “{ $p0 }” is geen aantal poorten: typ een geheel getal van 1 tot { $p1 }, zoals 8
error-part-bus-termination = “{ $p0 }” is geen stand van een afsluitschakelaar: kies aan, uit of geen
error-part-bus-twice = Interface { $p0 } staat er twee keer; geef elke rij een eigen interface of poortlabel
error-part-new-id = Er kon geen nieuw artikel worden gemaakt: het systeem gaf geen willekeurig getal; probeer het opnieuw
error-part-exists = { $p1 } { $p0 } staat al in Mijn artikelen of in dit project: open het in de lijst, of geef het nieuwe artikel een eigen artikelnummer
error-part-library = { $p0 } kon niet worden geschreven: { $p1 }
error-part-no-my-parts = Zonder open project wordt een nieuw artikel in Mijn artikelen bewaard, en Mijn artikelen kan hier niet worden gemaakt: open een project om het artikel daarin te bewaren
error-part-keep-no-my-parts = Mijn artikelen kan hier niet worden gemaakt, want dit venster heeft er geen eigen map voor: het artikel blijft in dit project
error-part-names-unknown = Het noemt een artikel dat geen bibliotheek van u heeft en dit project ook niet: kies dat artikel opnieuw in zijn veld
error-part-take-back-failed = { $p0 } kon niet terugnemen wat deze opslag schreef, dus deze staan er nu in: { $p1 }. Open ze daar, of verwijder ze daar.
reason-part-symbol-unsaved = Sla het symbool eerst op in een bibliotheek; het artikel noemt het dan
reason-part-not-own = Dit artikel staat niet in uw artikelbibliotheek: alleen een eigen artikel wordt eruit verwijderd
# de PLC-sectie van een nieuw artikel.
error-part-plc-role = “{ $p0 }” is geen PLC-rol; kies station of module
error-part-plc-kind = “{ $p0 }” is geen kanaalsoort; kies er een uit de lijst
error-part-plc-kind-missing = Kies de soort van de kanalen
error-part-plc-count = “{ $p0 }” is geen aantal kanalen: typ een geheel getal van 1 tot { $p1 }, zoals 8
error-part-plc-count-missing = Typ hoeveel kanalen het artikel heeft, zoals 8
error-part-plc-supply = Een voeding heeft een L+-pen en een M-pen; geef beide, of geen van beide
error-part-plc-supply-pairs = { $p0 } L+-pennen, { $p1 } M-pennen: geef één M-pen, of één bij elke L+-pen
error-part-plc-pin = “{ $p0 }” geeft geen pen voor kanaal { $p1 }: schrijf de pen van kanaal n met accolades, zoals {"{"}n+1{"}"} of X2:{"{"}n+1{"}"}
error-part-plc-pin-twice = { $p0 } zou de pen van twee kanalen zijn; elk kanaal heeft een eigen pen nodig
error-part-plc-slots = “{ $p0 }” is geen lijst van slots: typ getallen en reeksen, zoals 1-8, 10
error-part-plc-max = “{ $p0 }” is geen aantal modules: typ een geheel getal, zoals 32
error-part-plc-kind2-way = De tweede soort is een andere dan de eerste: ingangen of uitgangen naast zijn kanalen, elk DI, DO, AI, AO of teller
# An image over the pixel limit that an earlier build took in is kept at open, not drawn, and named.
banner-image-not-drawn = De afbeelding op folio { $p0 } is { $p1 } × { $p2 } pixels, meer dan een tekening kan tonen, en wordt daarom bewaard maar niet getekend; verklein haar en voeg haar opnieuw toe.
banner-image-not-drawn-folios = De afbeelding op folio's { $p0 } is { $p1 } × { $p2 } pixels, meer dan een tekening kan tonen, en wordt daarom bewaard maar niet getekend; verklein haar en voeg haar opnieuw toe.
banner-logo-not-drawn = Het bedrijfslogo is { $p0 } × { $p1 } pixels, meer dan een tekening kan tonen, en wordt daarom bewaard maar niet getekend; verklein het en voeg het opnieuw toe.
saveerr-external-collisions = de wijziging op schijf botst met uw bewerkingen; kies eerst uw versie of die op schijf
saveerr-external-unreadable = het project op schijf is gewijzigd en kan niet worden gelezen; uw bewerkingen blijven bewaard
reason-cores-no-wires = Selecteer de draden die de aders van de kabel worden
reason-cores-no-cable = Het project heeft nog geen kabel; maak er eerst een
error-cores-too-few = { $p0 } heeft { $p1 } vrije aders voor { $p2 } draden: selecteer minder draden, of kies een andere kabel.
error-cable-type = "{ $p0 }" is geen kabeltype. Schrijf het als 3G1.5 of 4x0.75, of geef het artikelnummer van een kabel.
error-cable-core-unknown = De kabel heeft geen ader { $p0 }: zijn aders zijn { $p1 }.
error-cable-core-twice = Ader { $p0 } is aan twee draden gegeven: geef elke draad een eigen ader.
symbol-input-limit = Het symboolbestand overschrijdt de toegestane grootte of het aantal elementen.
symbol-input-version = Dit formaat voor symboolontwerp wordt niet ondersteund.
symbol-invalid-key = Gebruik een sleutel met letters, cijfers, punten, koppeltekens of onderstrepingstekens: { $p0 }.
symbol-key-required = Geef het symbool een sleutel.
symbol-unknown-member = Het symbool bevat een veld dat deze versie niet kent: { $p0 }. Verwijder of hernoem het voordat u opslaat.
symbol-export-exists = Er bestaat al een bestand met deze naam. Kies een andere naam.
symbol-invalid-version = Voer een versie in als hoofdversie.subversie.patch: { $p0 }.
symbol-name-required = Geef het symbool een naam: { $p0 }.
symbol-provenance-required = Voeg een licentie en bron toe voor { $p0 }.
symbol-grid-unsupported = Kies het standaardraster van 4 mm of het QET-raster van 10 px voor { $p0 }.
symbol-cp-off-grid = Aansluitpunt { $p0 } ligt buiten het verbindingsraster. Verplaats het naar het raster voordat u opslaat.
symbol-cp-mapping = Aansluitpunt { $p0 } vereist een uniek, passend functieaansluitpunt.
symbol-cp-direction = Kies een uitgaande richting voor aansluitpunt { $p0 }.
symbol-slot-pin = De aanduidingstekst verwijst naar een ontbrekend aansluitpunt: { $p0 }.
symbol-anchor-slot = Het anker verwijst naar een ontbrekende tekstpositie in { $p0 }.
symbol-graphic-invalid = Grafisch element { $p0 } is leeg of gebruikt een niet-ondersteund tekentype.
symbol-geometry-limit = De symboolgeometrie overschrijdt de toegestane coördinaten of complexiteit.
symbol-slot-arrangement = Tekstindeling { $p0 } verwijst naar een ontbrekende tekstpositie.
symbol-function-changed = De toegewezen functiedefinitie is gewijzigd. Wijs deze opnieuw toe voordat u opslaat.
symbol-function-required = Wijs een functiedefinitie toe aan dit symbool.
symbol-input-object = Het symbooldocument is ongeldig.
symbol-import-review = Controleer de geïmporteerde geometrie, verbindingen en bronvoorwaarden voordat u opslaat.
symbol-graphics-required = Teken ten minste één grafisch element voordat u opslaat.
symbol-link-duplicate = Koppelingspunt { $p0 } moet een unieke naam hebben.
symbol-extra-variants = Opgeslagen alternatieve tekeningen moeten afzonderlijk worden gecontroleerd; maak eerst een kopie van de gewenste tekening.
symbol-qet-review = De QElectroTech-lezer heeft beperkingen bij de conversie gemeld. Vergelijk het concept vóór gebruik met de bron.
eplan-input-limit = De EPLAN-invoer overschrijdt de ondersteunde grootte of complexiteit.
eplan-archive-path = Het EPLAN-archief bevat onveilige of dubbele bestandsnamen.
eplan-archive-invalid = Het EPLAN-archief is beschadigd, versleuteld of niet ondersteund.
eplan-archive-method = Deze compressiemethode wordt niet ondersteund.
eplan-binary-unsupported = Deze binaire EPLAN-bibliotheek of macro wordt niet ondersteund. Exporteer een PXF XML-macro of artikel-XML vanuit EPLAN.
eplan-xml-invalid = De EPLAN-XML is ongeldig of bevat niet-toegestane declaraties.
eplan-geometry-invalid = De EPLAN-tekening bevat ongeldige coördinaten.
eplan-pxf-version = Deze EPLAN PXF-versie wordt niet ondersteund; de lezer ondersteunt revisie 1.23.
eplan-no-symbols = Er is geen ondersteunde symbooltekening of verbindingssjabloon gevonden.
eplan-format-unsupported = Dit EPLAN-documenttype wordt niet ondersteund.
symbol-import-stopped = De importfunctie is gestopt door een interne fout; er is niets geïmporteerd.
symbol-function-connections = De functie bevat een ongeldige interne verbinding. Controleer de aansluitingsindexen en het verbindingstype.
symbol-function-semantics = Niet-ondersteunde elektrische functiegegevens: { $p0 }. Kies een ondersteunde functie of corrigeer de verbindingsgegevens voor het opslaan.

eplan-slk-invalid = De database van de symboolbibliotheek is beschadigd of heeft een niet-ondersteunde recordlayout.
eplan-slk-version = Deze versie van de EPLAN-symboolbibliotheekdatabase wordt niet ondersteund.
eplan-slk-encoding = Dit oudere SLK-bestand heeft een niet-ondersteunde tekstcodering. UTF-16- en UTF-8-bestanden worden ondersteund.
eplan-slk-companion = Bewaar het SLK-bestand naast de volledige .sdb-map en open het opnieuw. Een vereiste tekentabel ontbreekt.
eplan-slk-record-unsupported = De database van de symboolbibliotheek bevat een niet-ondersteunde recordcodering.
dxf-input-limit = Het DXF-bestand overschrijdt de ondersteunde grootte of complexiteit.
dxf-input-invalid = Het bestand is geen DXF-tekening die deze lezer begrijpt.
dxf-no-unit = De tekening noemt geen eenheid. Kies de eenheid waarin ze getekend is.
dxf-no-blocks = De tekening heeft geen blok met een naam om een symbool van te maken.
dxf-no-page = Het project heeft geen getekende pagina { $p0 }.
dxf-outline-empty = { $p0 } bevat geen lijn om een contour van te maken.

# A date typed in the project's date format.
refusal-input-date = `{ $p0 }` is geen datum: typ hem als { $p1 }

# The Libraries pane's states and its Update button in words.
library-state-current = Actueel
library-state-update = Update beschikbaar ({ $p0 })
error-part-pin-unknown = Het artikel heeft geen pen { $p0 }
error-part-pin-name = Geef de pen een naam zoals op het apparaat staat, bijvoorbeeld 13 of L+
error-part-pin-role = { $p0 } is geen potentiaal die een pen kan voeren
rack-channel-no-device = Op dit kanaal zit nog geen draad: zijn functietekst wordt die van het apparaat dat erop aangesloten wordt. Tot dan drukt de kaart de symboolnaam van het kanaal in zijn cel af: geef het er een.
error-cable-section-cores = “{ $p0 }” is geen aantal aders: typ een geheel getal van 1 tot { $p1 }, zoals 3
error-cable-section-pe = Een kabel met één ader heeft geen beschermingsleiding ernaast: geef hem twee aders of meer, of zet de beschermingsleiding uit
error-cable-section-csa = “{ $p0 }” is geen doorsnede: typ de vierkante millimeters, zoals 1,5
error-cable-section-core = Ader { $p0 } heeft een eigen naam nodig (“{ $p1 }”), zoals 1 of BN
error-cable-section-colour = Ader { $p0 }: “{ $p1 }” is geen kleurcode; typ er een zoals BK, BN of GNYE, of laat het leeg
part-misfit-cores = { $p0 } aders, te weinig voor zijn { $p1 } draden
part-misfit-pe = geen beschermingsleiding voor zijn PE-draad

# The navigators' group rows (output::navigators): the bus navigator's nets and ports, the PLC navigator's modules without a station and racks, the sections, the fluid groups.
nav-bus-logical = Logische netten
nav-bus-physical = Fysieke netten
nav-bus-no-net = Poorten zonder net
nav-bus-undrawn = Poorten nog niet getekend
# De cel Staat in van de bustabel: onder welk net de rij van een poort staat.
nav-bus-listed-logical = logisch net
nav-bus-listed-physical = fysiek net
nav-bus-listed-none = geen net
nav-bus-segment = Segment { $p0 }
nav-plc-no-cpu = Modules zonder station
nav-bridges = Bruggen
nav-none = (geen)
nav-unplaced-glands = Wartels
nav-unplaced-gland-mount = wartel { $p0 }
nav-front-matter = Voorwerk
nav-section = Sectie { $p0 } - { $p1 }
nav-rack = Rek { $p0 }
nav-fluid-terminals = Ventieleilanden
nav-fluid-devices = Fluïdumapparaten
nav-fluid-media = Media
# An interruption point's direction (output::navigators::ips; the context menu's choice too).
choice-source = Bron
choice-target = Doel
choice-undirected = Ongericht
# The Messages pane's severity groups (output::navigators::messages).
messages-group-error = Fouten
messages-group-warning = Waarschuwingen
messages-group-info = Info
# A channel's state in the PLC navigator (output::navigators::plc; the field's choice too).
choice-used = Gebruikt
choice-spare = Reserve
choice-reserved = Gereserveerd
choice-deactivated = Gedeactiveerd
# The words of a choice that a navigator's column also heads by (output::navigators).
choice-bridge-part = Brugartikel
choice-bus = Bus
choice-channel = Kanaal
choice-designation = Aanduiding
choice-device = Apparaat
choice-devices = Apparaten
choice-folio = Folio
choice-function-text = Functietekst
choice-library = Bibliotheek
choice-module = Module
choice-name = Naam
choice-pages = Pagina's
choice-potential = Potentiaal
choice-section = Sectie
choice-shield = Afscherming
choice-station = Station
choice-type = Type

# A library's index, tier and access in the Libraries navigator (output::navigators::libraries; the status bar's Read-only too).
lib-index-fresh = Index actueel
lib-index-rebuilt = Index opnieuw opgebouwd
lib-notes-between = { "; " }
lib-index-failed = Index mislukt: { $p0 }
lib-tier-system = Systeem
lib-tier-community = Gemeenschap
lib-tier-company = Bedrijf
lib-tier-user = Gebruiker
lib-access-writable = Beschrijfbaar
status-bar-read-only = Alleen-lezen

# The names of function kinds by their FnDef category (output::navigators::function_kind), and the spelt-out last entry of a changeover's category.
kind-words-decoration = Decoratie
kind-words-overview = Overzicht
kind-words-black-box = Black box
kind-words-interruption-point = Onderbrekingspunt
kind-words-external-reference = Externe verwijzing
kind-words-earth-connection = Aardverbinding
kind-words-functional-earth-connection = Functionele aardverbinding
kind-words-profibus-connector = PROFIBUS-connector
kind-words-profibus-port = PROFIBUS-poort
kind-words-plug-pin = Stekkerpen
kind-words-terminal-strip-segment = Klemmenstrooksegment
kind-words-feed-through-terminal = Doorgangsklem
kind-words-pe-terminal = PE-klem
kind-words-1p-mcb = 1P-installatieautomaat
kind-words-1p-n-mcb = 1P+N-installatieautomaat
kind-words-1p-manual-switch = 1P-handschakelaar
kind-words-2p-manual-switch = 2P-handschakelaar
kind-words-no-position-switch = NO-positieschakelaar
kind-words-no-selector-switch = NO-keuzeschakelaar
kind-words-enclosure-heater = Kastverwarming
kind-words-double-acting-cylinder = Dubbelwerkende cilinder
kind-words-two-way-process-valve = Tweewegprocesklep
kind-words-5-2-valve = 5/2-ventiel
kind-segment-changeover = wissel
# A function's kind by the generic rule (output::navigators::function_kind): its two category segments joined
# by kind-generic, each by its kind-segment-<segment> text (a Dutch segment carries its own joint).
kind-generic = { $p0 }{ $p1 }
kind-segment-no = NO-
kind-segment-nc = NC-
kind-segment-1p = 1P-
kind-segment-3p = 3P-
kind-segment-cabinet = kast
kind-segment-fixed = vaste{ " " }
kind-segment-contact = contact
kind-segment-coil = spoel
kind-segment-fuse = zekering
kind-segment-lamp = lamp
kind-segment-motor = motor
kind-segment-pushbutton = drukknop
kind-segment-resistor = weerstand
kind-segment-thermostat = thermostaat
kind-segment-transformer = transformator
kind-segment-2p = 2P-
kind-segment-a1a2 = A1A2-
kind-segment-acdc = AC/DC-
kind-segment-adjustable = instelbaar{ " " }
kind-segment-and = AND-
kind-segment-autodrain = automatisch aftappend{ " " }
kind-segment-automatic = automatische{ " " }
kind-segment-closedcentre = midden gesloten{ " " }
kind-segment-cushioned = gedempte{ " " }
kind-segment-cylinder = { "" }
kind-segment-doubleacting = dubbelwerkende{ " " }
kind-segment-exhaustedcentre = midden ontlucht{ " " }
kind-segment-fluid = pneumatisch{ " " }
kind-segment-frl = FRL-
kind-segment-frldetailed = uitgebreide FRL-
kind-segment-generic = algemene{ " " }
kind-segment-lever = hendel
kind-segment-leverdetent = Hendel/vergrendeling-
kind-segment-leverspring = Hendel/veer-
kind-segment-magnetic = magnetische{ " " }
kind-segment-manifold = spruitstuk
kind-segment-manualdrain = handmatig aftappend{ " " }
kind-segment-open = open{ " " }
kind-segment-or = OR-
kind-segment-pnp = PNP-
kind-segment-parallel = parallel
kind-segment-pilotspring = Stuurlucht/veer-
kind-segment-pressure = { "" }
kind-segment-pressurisedcentre = midden belucht{ " " }
kind-segment-profinet = PROFINET-
kind-segment-pushbuttonspring = Drukknop/veer-
kind-segment-quick = snel
kind-segment-relieving = ontluchtende{ " " }
kind-segment-rollerspring = Rol/veer-
kind-segment-sensing = { "" }
kind-segment-solenoidsolenoid = Spoel/spoel-
kind-segment-solenoidspring = Spoel/veer-
kind-segment-solenoidspringno = Spoel/veer-NO-
kind-segment-standard = { "" }
kind-segment-supply = lucht
kind-segment-bulkhead = schotdoorvoer
kind-segment-busport = buspoort
kind-segment-check = terugslagklep
kind-segment-coupling = koppeling
kind-segment-distributor = verdeler
kind-segment-drain = condensaataftap
kind-segment-ejector = ejector
kind-segment-emergencystop = noodstop
kind-segment-exhaust = ontluchting
kind-segment-filter = filter
kind-segment-gauge = manometer
kind-segment-gripper = grijper
kind-segment-ip = onderbrekingspunt
kind-segment-lubricator = olienevelaar
kind-segment-manualswitch = handschakelaar
kind-segment-mcb = installatieautomaat
kind-segment-oneway = eenrichtingssmoorventiel
kind-segment-onoffvalve = afsluiter
kind-segment-plug = blindstop
kind-segment-powersupply = voeding
kind-segment-pressureswitch = drukschakelaar
kind-segment-proximitysensor = naderingsschakelaar
kind-segment-quickexhaust = snelontluchtingsventiel
kind-segment-rcd = aardlekschakelaar
kind-segment-regulator = drukregelaar
kind-segment-reservoir = drukvat
kind-segment-rodless = stangloze cilinder
kind-segment-semirotary = zwenkaandrijving
kind-segment-serviceunit = verzorgingseenheid
kind-segment-shuttle = wisselventiel
kind-segment-silencer = geluiddemper
kind-segment-singleacting = enkelwerkende cilinder
kind-segment-softstart = softstartventiel
kind-segment-source = bron
kind-segment-suctioncup = zuignap
kind-segment-supplyplate = aansluitplaat
kind-segment-switch = schakelaar
kind-segment-throttle = smoorventiel
kind-segment-twopressure = tweedrukventiel
kind-segment-valve22 = 2/2-ventiel
kind-segment-valve2x32 = 2×3/2-ventiel
kind-segment-valve32 = 3/2-ventiel
kind-segment-valve52 = 5/2-ventiel
kind-segment-valve53 = 5/3-ventiel

# The stamp a print scaled to its paper carries, in the project's language (output::print).
print-stamp-scaled = geschaald op { $p0 } %

# The bus navigator's column labels (output::navigators::bus).
column-interface = Interface
column-address = Adres
column-termination = Afsluiting
column-listed-in = Staat in
column-role = Rol
column-logical-net = Logisch net
column-physical-net = Fysiek net
column-rating = Kenwaarden
column-title = Titel
column-frame = Kader
column-generated = Gegenereerd
column-changed-in = Gewijzigd in
column-kind = Soort
column-part = Artikel
column-colour = Aderkleur
column-strip = Klemmenstrook
column-pos = Pos
strips-with = met { $p0 }
strips-left-off = weggelaten op deze klemmenstrook
strips-accessory = Toebehoren
table-derived-accessory = het komt met het artikel van de klemmen: Verwijderen laat het op deze klemmenstrook weg
column-state = Status
column-pair = Paar
column-target = Doel
column-symbol = Symbool
column-process-tag = Proceskenmerk
column-placed-on = Geplaatst op
column-slot = Slot
column-rack = Rek
column-data-type = Gegevenstype
column-signal-range = Signaalbereik
column-safety = Veiligheid
column-comment = Opmerking
column-page-cell = Paginavak
column-direction = Richting
column-partner = Partner
column-external = Extern
column-usage = Gebruik
column-part-number = Artikelnummer
column-first-folio = Eerste folio
column-size = B × H (mm)
column-mount = Montage
column-version = Versie
column-variables = Variabelen
column-tier = Niveau
column-licence = Licentie
column-access = Toegang
column-used-by = Gebruikt door
column-severity = Ernst
column-rule = Regel
column-message = Melding
column-object = Object
column-fix = Oplossing
column-connections-part = Draadartikel
misfit-of = Past niet bij { $p0 } van { $p1 }; { $p2 }: { $p3 }
misfit-function = Een artikel van een andere soort: zijn functie is niet die van het apparaat
misfit-function-kinds = Een artikel van een andere soort: { $p0 }, geen { $p1 }
misfit-pin = Het heeft geen aansluiting { $p0 }; { $p1 }:{ $p0 } is bedraad
misfit-clamp = Zijn klem { $p0 } neemt { DECIMAL($p1) } tot { DECIMAL($p2) } mm²; draad { $p3 } is { DECIMAL($p4) } mm²
misfit-clamp-unnumbered = Zijn klem { $p0 } neemt { DECIMAL($p1) } tot { DECIMAL($p2) } mm²; een draad erop is { DECIMAL($p3) } mm²

## Hoe de wartel van een kabel en zijn gat afgeleid zijn (doc 12 §3.9): de woorden van een herkomstlabel
via-gland-proposed = voorgesteld
via-gland-proposed-for = voorgesteld voor { $p0 } mm
via-gland-no-diameter = de kabel geeft geen buitendiameter
via-gland-none-seals = geen wartel van de voorraad dicht { $p0 } mm af
via-gland-hole-on-page = op pagina { $p0 }
via-gland-hole-placed = geplaatst
via-gland-hole-not-placed = niet geplaatst

## Wat een gat van een vlak maakte, zijn rij Gemaakt (doc 12-3d §10.6), en de koppeling van de rij
hole-made-by-hand = Met de hand
hole-made-for-cable = Voor de wartel van { $p0 }
hole-made-for-gone-cable = Voor de wartel van een kabel; de kabel is er niet meer
hole-cable-select = Selecteer de kabel

## De loop van een draad zoals een voorbeeld hem noemt (de kaart van Routeren, de geschiedenis)
preview-route-way = { $p0 }: een loop door { $p1 }, { $p2 }
preview-route-way-plain = { $p0 }: een loop van { $p1 }
preview-route-way-was = { $p0 }: een loop door { $p1 }, { $p2 } (was { $p3 })
preview-route-way-plain-was = { $p0 }: een loop van { $p1 } (was { $p2 })
preview-route-unfinished = { $p0 }: zijn loop vervalt, de draad is niet af
preview-route-replaced = { $p0 }: zijn oude loop maakt plaats voor de nieuwe

## How a value beside its derived row is had (doc 16 §2.2): the provenance chip's words
via-joined = { $p0 } · { $p1 }
via-connectivity = verbindingen
via-potential-definition-at = potentiaaldefinitie bij { $p0 }
via-override = overschrijving
via-override-cdp = overschrijving (CDP)
via-override-on = overschrijving op { $p0 }
via-override-on-another = overschrijving op een andere verbinding
via-no-supply-upstream = geen voeding gevonden vóór { $p0 }, dus de nominale stroom ervan wordt niet gebruikt
via-no-supply = geen voeding gevonden vóór de draad, dus bepaalt geen beveiligingsapparaat de doorsnede
via-no-protection = geen beveiligingsapparaat vóór de draad bepaalt de doorsnede
via-frozen = vastgezet
via-frozen-sign-hidden = vastgezet (teken verborgen)
via-wire-numbering = draadnummering
via-numbering = nummering
via-numbering-scheme = nummeringsschema '{ $p0 }'
via-child-of = kind van { $p0 }
via-terminal-of-strip = klem van klemmenstrook { $p0 }
via-earth-connection = een aardsymbool: een verbinding met aarde, geen apparaat, dus geen kenmerk
via-main-representation = hoofdweergave
via-strip-groups = klemmenstrookgroepen
via-function-definition = functiedefinitie
via-shared-stretch = meerdere draden delen dit stuk: zet de eigen waarden van een draad in de tabel Verbindingen
via-set-by-hand = met de hand gezet
via-workshop-marking = de merkertekst van het project
via-workshop-stock = werkplaatsvoorraad
via-made-outside = buiten het project gemaakt
via-rule-unverified = regel (niet bevestigd)
via-rule-assumed-class = regel (niet bevestigd): fijndradige draad
via-rule-assumed-clamp = regel (niet bevestigd): klemsoort onbekend
via-rule-assumed-both = regel (niet bevestigd): fijndradige draad, klemsoort onbekend
via-rule-verified = regel
via-rule-verified-assumed-class = regel: fijndradige draad
via-rule-verified-assumed-clamp = regel: klemsoort onbekend
via-rule-verified-assumed-both = regel: fijndradige draad, klemsoort onbekend
via-project-default = projectstandaard
via-project-description = projectomschrijving
via-project-description-is = projectomschrijving is '{ $p0 }'
via-cross-reference-format = kruisverwijzingsformaat
via-page = pagina { $p0 }
via-structure-box-on-page = structuurkader op pagina { $p0 }
via-no-terminal = op geen ventieleiland
via-valve-positions = zijn ventielplaatsen
via-with = met { $p0 }
via-plc-station = zijn PLC-station
via-plc-output = PLC-uitgang { $p0 }
via-no-plc-output = nog geen PLC-uitgang
via-plug = zijn stekker
via-valve-part = het artikel van het ventiel
via-medium-definition-at = medium bepaald op { $p0 }
via-through-valves = via de ventielen doorgegeven
via-no-medium = geen medium bepaald
via-no-medium-yet = nog geen medium: kies er hier een
via-port-no-medium = geen medium: kies er een in de rij Medium van de bron, de persluchtaansluiting
via-project-default-tube = de standaardslang van het project
via-medium = medium { $p0 }
via-tube-part = slangartikel { $p0 }
via-no-tube-spec = geen standaardslang, mediumslang of slangartikel
via-primary-part = primair artikel
via-part = artikel
bus-symbol-derived-note = Getekend uit de netwerkpoorten van { $p0 }; het volgt het artikel wanneer zijn poorten veranderen.
bus-symbol-own-note = { $p0 }: het programma tekent het nooit opnieuw; een aansluiting die het mist, staat op het blad van het artikel en in de controle.
bus-symbol-program = standaard
bus-symbol-own = eigen
bus-symbol-make-own = Mijn eigen ervan maken…
bus-symbol-open = Het bussymbool openen
via-layout-label = layoutlabel
via-rail-length = raillengte min plaatsingen en eindsteunen
via-path = pad
via-stored-ways = opgeslagen lopen
via-hole-table = gatentabel
via-hole = H{ $p0 } · { $p1 }
via-hole-tap = { $p0 } · draad M{ $p1 }
via-hole-moved = verplaatst · { $p0 }
via-hole-left-out = weggelaten

## Bewaren in Mijn artikelen (doc 07 §5.8)
reason-part-keep = Alleen een artikel dat alleen in dit project staat, wordt in Mijn artikelen bewaard.
reason-part-edit-not-yours = Dit artikel is niet het uwe om te wijzigen: maak er een eigen artikel van (Mijn eigen maken in Artikelen)
reason-part-edit-mine = Dit artikel staat in Mijn artikelen: wijzig het in Artikelen, waar Opslaan er de volgende versie van maakt

## De eerste vraag van Nieuw artikel…, Wat is het? (doc 07 §5.5): de groepen, en per begin de kaart met de regel van
## wat erna gevraagd wordt
part-starts-devices = Apparaten
part-starts-wiring = Klemmen, kabels en bedrading
part-starts-cabinet = Kastopbouw
part-starts-fluid = Pneumatiek
part-start-device = Apparaat met symbolen
part-start-device-line = Een magneetschakelaar, een automaat, een relais, een schakelaar, een lamp: getekend als zijn delen.
part-start-box = Apparaat getekend als blok
part-start-box-line = Een voeding, een aandrijving, een busapparaat: een blok met zijn aansluitingen en poorten.
part-start-module = PLC-kaart of station
part-start-module-line = Kanalen met adressen, op een rek of een bus.
part-start-door = Drukknop of lamp in een deur
part-start-door-line = Getekend met een symbool, gemonteerd in een uitsparing.
part-start-terminal = Klem
part-start-terminal-line = Zijn niveaus en klemmen, zijn breedte.
part-start-cable = Kabel
part-start-cable-line = Zijn aders en zijn buitendiameter.
part-start-wire = Draad
part-start-wire-line = Een doorsnede en een kleur, voor de voorraad.
part-start-ferrule = Adereindhuls of kabelschoen
part-start-ferrule-line = De doorsneden die het neemt.
part-start-marker = Merker
part-start-marker-line = Wat het merkt en hoeveel tekst erop past.
part-start-gland = Kabelwartel
part-start-gland-line = Zijn draad en de kabels die het klemt.
part-start-enclosure = Kast
part-start-enclosure-line = Zijn maten, zijn montageplaat, zijn deuren.
part-start-plate = Montageplaat
part-start-plate-line = Zijn maten.
part-start-rail = Rail
part-start-rail-line = Zijn profiel.
part-start-duct = Kabelgoot
part-start-duct-line = Zijn buiten- en binnenmaat.
part-start-accessory = Eindsteun of toebehoren
part-start-accessory-line = Een eindplaat, een deksel, een brug: geteld met waar het bij hoort.
part-start-valve = Ventiel
part-start-valve-line = Zijn poorten en zijn spoel.
part-start-tube = Slang
part-start-tube-line = Zijn diameters en zijn druk.
part-start-fluid = Ander pneumatisch apparaat
part-start-fluid-line = Een cilinder, een luchtbehandelingseenheid: getekend met een symbool.
# The label file's export profile in a line (doc 13-package §7.3): its form, then its headings.
label-file-summary = { $p0 } · { $p1 }
label-file-xlsx = Excel
label-file-csv-comma = CSV met komma's
label-file-csv-semicolon = CSV met puntkomma's
label-file-csv-other = CSV met { $p0 }
label-file-change = Wijzigen…

## The package for the workshop (doc 13-package §8). A `$count` is `one` or `many`, `$n` its number; `$count2` and `$m`
## a second count.
package-member-contents = Inhoud
package-member-pages = Tekeningen
package-member-labels = Labelbestanden
package-member-drilling = Boortekeningen
package-form-text = Tekst
package-form-pdf = PDF
package-form-drilling = PDF, DXF
package-folder-lists = Lijsten
package-folder-labels = Labels
drilling-folder = Boortekeningen
package-refused = { $count ->
        [one] 1 fout houdt een vrijgavepakket tegen
       *[many] { $n } fouten houden een vrijgavepakket tegen
    }
package-file-name = { $p0 } { $p1 } pakket.zip
package-file-name-draft = { $p0 } { $p1 } conceptpakket.zip
package-holds-pages = elke pagina in paginavolgorde, zoals bij PDF exporteren
package-no-holes = Nog geen vlak van dit project heeft gaten
package-no-rows = Nog niets te vermelden
package-no-labels = Het project heeft nog geen labels
package-lacks-marker = { $p0 }: de voorraad heeft geen merkerartikel voor { $p1 }; het labelprogramma vraagt waarop het afdrukt.
package-lacks-part = { $p0 }: { $p1 }
package-holds-labels = { $count ->
        [one] 1 bestand
       *[many] { $n } bestanden
    }, { $count2 ->
        [one] 1 label
       *[many] { $m } labels
    }
package-holds-drilling = { $count ->
        [one] 1 vlak · 1 boorsjabloon op A4 en 1 DXF
       *[many] { $n } vlakken · { $n } boorsjablonen op A4 en { $n } DXF
    }
package-holds-wire-list = { $count ->
        [one] 1 draad
       *[many] { $n } draden
    }, { $count2 ->
        [one] 1 brug
       *[many] { $m } bruggen
    }
package-holds-assembly-list = { $count ->
        [one] 1 klemmenstrook
       *[many] { $n } klemmenstroken
    }
package-holds-hole-table = { $count ->
        [one] 1 vlak
       *[many] { $n } vlakken
    }, { $count2 ->
        [one] 1 gat
       *[many] { $m } gaten
    }
package-holds-cut-list = rails, kabelgoten, kabels, draden en slangen
package-holds-consumables = draad, adereindhulzen en merkers om te bestellen
package-holds-rows = { $count ->
        [one] 1 regel
       *[many] { $n } regels
    }
package-marks-wires = draadeinden
package-marks-terminals = klemmen
package-marks-strips = klemmenstroken
package-marks-cables = kabels
package-marks-devices = apparaten
package-marks-tubes = slangen
package-marks-plates = de frontplaatjes van de apparaten in de deur
package-draft-mark = Concept { $p0 }: niet vrijgegeven
package-draft-mark-none = Concept: geen revisie
package-drawings-file = { $p0 } tekeningen.pdf
package-about-pages = Elke pagina van de tekeningen in paginavolgorde, de lijstpagina's ertussen.
package-about-labels = { $count ->
        [one] 1 label.
       *[many] { $n } labels.
    }
package-about-labels-no-part = { $count ->
        [one] 1 label
       *[many] { $n } labels
    }; geen merkerartikel: kies de merker in het labelprogramma.
package-about-dxf = { $count ->
        [one] 1 gat
       *[many] { $n } gaten
    }, voor een machine; 0,0 in de hoek van het vlak van waaruit de gaten gemeten zijn.
package-about-template = { $count ->
        [one] 1 gat
       *[many] { $n } gaten
    } op { $count2 ->
        [one] 1 vel
       *[many] { $m } vellen
    } A4, af te drukken op 100 %.
package-contents-file = Inhoud.txt
package-contents-released-on = Pakket voor de werkplaats, revisie { $p0 }, vrijgegeven op { $p1 }
package-contents-released-undated = Pakket voor de werkplaats, revisie { $p0 }, vrijgegeven; de revisie vermeldt geen dag van vrijgave
package-contents-made-on = Pakket voor de werkplaats, revisie { $p0 }, gemaakt op { $p1 }
package-contents-made-on-none = Pakket voor de werkplaats, gemaakt op { $p0 }
package-contents-undated = Pakket voor de werkplaats, revisie { $p0 }; het project heeft geen datum
package-contents-undated-none = Pakket voor de werkplaats; het project heeft geen datum
package-contents-draft = CONCEPT: revisie { $p0 } is open, niet vrijgegeven. Op elke pagina van de tekeningen staat "{ $p1 }".
package-contents-draft-none = CONCEPT: het project heeft geen revisie. Op elke pagina van de tekeningen staat "{ $p0 }".
package-contents-release = Het pakket van de vrijgave: de pagina's dragen geen merkteken.
package-contents-ready = Voordat het naar de werkplaats ging, is het project gecontroleerd: niets stond in de weg.
package-contents-stood = Voordat het naar de werkplaats ging, is het project gecontroleerd: { $count ->
        [one] 1 punt stond open
       *[many] { $n } punten stonden open
    }. Een fout houdt een vrijgave tegen; een waarschuwing of een melding houdt niets tegen.
package-no-length-site = { $count ->
        [one] 1 draad heeft geen lengte: één einde ervan wordt ter plaatse gemaakt
       *[many] { $n } draden hebben geen lengte: van elk wordt één einde ter plaatse gemaakt
    }
package-no-length-not-laid-in = { $count ->
        [one] 1 draad van { $cabinet } heeft nog geen lengte: Routeren geeft hem er een, of typ hem
       *[many] { $n } draden van { $cabinet } hebben nog geen lengte: Routeren geeft ze er een, of typ ze
    }
package-no-length-not-laid = { $count ->
        [one] 1 draad heeft nog geen lengte: typ hem
       *[many] { $n } draden hebben nog geen lengte: typ ze
    }
package-no-length-no-layout = { $count ->
        [one] 1 draad heeft geen lengte: dit project heeft geen paneellayout; typ hem, of maak de paneellayout en routeer
       *[many] { $n } draden hebben geen lengte: dit project heeft geen paneellayout; typ ze, of maak de paneellayout en routeer
    }
package-no-length-stale = { $count ->
        [one] 1 draad heeft een lengte langs een weg die niet meer klopt, afgedrukt na "≈": Routeren geeft hem een nieuwe
       *[many] { $n } draden hebben een lengte langs een weg die niet meer klopt, afgedrukt na "≈": Routeren geeft ze nieuwe
    }
package-contents-no-length = { $p0 } ({ $p1 }).
package-no-length-line = { $p0 }.
package-contents-every-length = Elke draad heeft zijn lengte in de draadlijst; een kabelader heeft die van zijn kabel.
package-contents-other-lengths = Elke andere draad heeft zijn lengte in de draadlijst; een kabelader heeft die van zijn kabel.
package-contents-lacks = Wat de bestanden missen van de werkplaatsvoorraad en van de gegevens van zijn artikelen:
package-contents-what = Wat erin zit
package-about-labels-folder = Eén bestand per soort label en merker, voor het labelprogramma: { $count ->
        [one] 1 label.
       *[many] { $n } labels.
    }
package-about-drilling-folder = Per vlak met gaten: een boorsjabloon om op 100 % af te drukken en erop te leggen, en een DXF voor een machine.
package-about-lists-folder = De werkplaatslijsten, elk als de Excel-export van de lijst.
package-about-manifest = Dezelfde lijst voor een programma, met een SHA-256 van elk bestand.
package-contents-made-by = Gemaakt door ElektraWatt { $p0 }.
# De kaart van Routeren en `ew route`: de opgeslagen lopen die Routeren verwijdert (doc 12-3d §8.5); `$count` is
# `one` of `many`, `$n` het aantal.
route-ways-removed = { $count ->
        [one] 1 loop van een draad die { $cabinet } niet meer legt, wordt verwijderd: { $wires }
       *[many] { $n } lopen van draden die { $cabinet } niet meer legt, worden verwijderd: { $wires }
    }
