# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: GPL-3.0-or-later

# The words of the domain's kinds in nl-NL (architecture §18).

## A port's role, as the tube lists print it

port-role-supply = voeding
port-role-working = werkpoort
port-role-exhaust = afvoer
port-role-pilot = stuurpoort
port-role-pilot-exhaust = stuurafvoer
port-role-external-pilot = externe stuurlucht
port-role-consumer = verbruiker
port-role-sensing = meetpoort
port-role-other = poort

## A line kind, as the tube lists print it

line-kind-working = werkleiding
line-kind-pilot = stuurleiding
line-kind-exhaust = afvoerleiding
line-kind-drain = lekleiding
line-kind-flexible = slang
line-kind-other = leiding

## A tube by its ends, and a junction as a tube's end

tube-ends = { $p0 } → { $p1 }
tube-end-junction = T-stuk
junction-in = de aftakking in { $p0 }

## A medium the derivation names

medium-vacuum = vacuüm (uit { $p0 })
medium-mixed = gemengd
title-scale = Schaal
medium-exhaust = afvoer

## A PLC channel, as the fluid findings name it

channel-kind-di = DI
channel-kind-do = DO
channel-kind-ai = AI
channel-kind-ao = AO
channel-kind-counter = teller
channel-kind-io-link = IO-Link
channel-kind-supply = voeding
channel-kind-multi-function = multifunctie
channel-kind-other = overig
channel-of-module = kanaal { $p0 } van { $p1 }
channel-no-address = geen adres

## The category of an open item, as the list of open items prints it

open-item-tbd-value = Waarde n.t.b.
open-item-device-without-part = Apparaat zonder artikel
open-item-unplaced-device = Niet geplaatst apparaat
open-item-unconnected-cp = Niet aangesloten aansluitpunt
open-item-missing-length = Lengte ontbreekt

## A text style's name, as the Text styles page lists it and the symbol editor offers it

text-style-tag-main = Apparaatkenmerk
text-style-data-small = Kleine tekst
text-style-cp-small = Aansluitcodering
text-style-pin = Pintekst
text-style-xref-device = Kruisverwijzing
text-style-marker = Adermarkering
text-style-terminal = Klemnummer
text-style-text-normal = Normale tekst
text-style-field-label = Veldlabel
text-style-box-title = Titel kader
text-style-cover-table = Tabel voorblad
text-style-cover-title = Titel voorblad
text-style-cover-subtitle = Ondertitel voorblad
text-style-section-id = Sectienummer
text-style-section-subtitle = Ondertitel sectie
text-style-imported = Geïmporteerde tekst van { $p0 } pt
text-style-imported-bold = Geïmporteerde vette tekst van { $p0 } pt
text-style-own = Een tekststijl van dit project

## A potential type and a circuit class, as the lists print them and the checks name them

potential-type-code = { $p0 }
potential-type-signal = signaal
potential-type-undefined = onbepaald
potential-type-other = overig
circuit-class-mains = net
circuit-class-control = sturing
circuit-class-analogue = analoog
circuit-class-bus = bus
circuit-class-pe = PE
circuit-class-other = overig

# Why a library's PLC profile is no address profile at hand (plc::unreadable_profile).
lib-profile-unreadable = het adresprofiel { $p0 } kan niet worden gelezen: { $p1 }

# Where a wire's property comes from: as the SQL view's provenance column words it (prop-source-*), and as the
# Properties grid's via chip words it (prop-via-*); a project default reads alike in both.
prop-source-override = overschreven op de draad
prop-source-core = ader { $p0 } van { $p1 }
prop-source-cable-core = kabelader
prop-source-potential = potentiaaldefinitie
prop-source-colour-profile = kleurprofiel { $p0 }
prop-source-rating = beveiligd door { $p0 }, { $p1 } A · { $p2 }
prop-source-size-profile = maatprofiel { $p0 }
prop-source-project = projectstandaard
prop-source-built-in = ingebouwde standaard van afleidingsprofiel { $p0 }
prop-via-core = kabel { $p0 }, ader { $p1 }
prop-via-potential = potentiaaldefinitie { $p0 }
prop-via-colour-profile = potentiaal { $p0 } · kleurprofiel '{ $p1 }'
prop-via-rating = potentiaal { $p0 } · beveiligd door { $p1 }, { $p2 } A · { $p3 }
prop-via-size-profile = potentiaal { $p0 } · maatprofiel '{ $p1 }' · klasse { $p2 }
prop-via-built-in = ingebouwde standaard ({ $p0 })
prop-via-override = overschreven

## Where a length comes from, as the wire list, the tube list and the cable overview print it

length-source-typed = getypt
length-source-cable = kabel
length-source-from-part = uit artikel
length-source-missing = ontbreekt

## The rework list: what the fitter does, and what changed

rework-remove-device = Apparaat verwijderen
rework-remove-terminal = Klem verwijderen
rework-remove-cable = Kabel verwijderen
rework-pull = Uittrekken
rework-replace = Vervangen
rework-move-end = Einde verleggen
rework-remake-end = Einde opnieuw afwerken
rework-check-length = Lengte nakijken
rework-remark = Merker vervangen
rework-relabel = Label vervangen
rework-mount-device = Apparaat monteren
rework-add-terminal = Klem toevoegen
rework-add-cable = Kabel toevoegen
rework-add = Toevoegen
rework-change-size = doorsnede { $p0 } → { $p1 }
rework-change-colour = kleur { $p0 } → { $p1 }
rework-change-wire-part = draadartikel { $p0 } → { $p1 }
rework-change-end = einde { $p0 } → { $p1 }
rework-change-remake = op { $p0 }: { $p1 } → { $p2 }
rework-change-remark = merker op { $p0 }: { $p1 } → { $p2 }
rework-change-relabel = { $p0 } → { $p1 }
rework-change-length = lengte { $p0 } → { $p1 }
rework-changes-between = { "; " }

## An end's treatment, as the wire list's end cell prints it (doc 13 §4.4)

treatment-ferrule = huls
treatment-ferrule-size = huls { $p0 }
treatment-twin = twinhuls { $p0 } · { $p1 }
treatment-twin-alone = twinhuls { $p0 }
treatment-ring-lug = ringkabelschoen
treatment-ring-lug-stud = ringkabelschoen { $p0 }
treatment-bare = kaal
treatment-connector = connector
treatment-unknown = ?

## The label sets (doc 13 §5.3)

label-set-wire-markers = Draadmerkers
label-set-terminal-markers = Klemmerkers
label-set-strip-markers = Klemmenstrookmerkers
label-set-cable-markers = Kabelmerkers
label-set-device-labels = Apparaatlabels
label-set-tube-markers = Slangmerkers
label-set-plates = Frontplaatjes

## A strip accessory's kind (07 §2.5): the strip editor's rows and the assembly list's Item

accessory-end-plate = Eindplaat
accessory-end-clamp = Eindsteun
accessory-partition = Scheidingsplaat
accessory-marker = Markeringsdrager
accessory-cover = Afdekking
accessory-other = Toebehoren
accessory-end-plate-plural = Eindplaten
accessory-end-clamp-plural = Eindsteunen
accessory-partition-plural = Scheidingsplaten
accessory-marker-plural = Markeringsdragers
accessory-cover-plural = Afdekkingen
accessory-other-plural = Toebehoren
## De vlakken van een kast (doc 12 §2.8)

surface-plate = Montageplaat
surface-door = Deur
surface-door-left = Linkerdeur
surface-door-right = Rechterdeur
surface-door-n = Deur { $p0 }
surface-left = Linkerzijwand
surface-right = Rechterzijwand
surface-top = Bovenkant
surface-bottom = Onderkant
surface-other = Vlak
surface-of-plate = { $p0 } montageplaat
surface-of-door = { $p0 } deur
surface-of-door-left = { $p0 } linkerdeur
surface-of-door-right = { $p0 } rechterdeur
surface-of-door-n = { $p0 } deur { $p1 }
surface-of-left = { $p0 } linkerzijwand
surface-of-right = { $p0 } rechterzijwand
surface-of-top = { $p0 } bovenkant
surface-of-bottom = { $p0 } onderkant
surface-of-other = { $p0 } vlak
surface-seen-front = van voren gezien
surface-seen-outside = van buiten gezien
surface-seen-left = van links gezien
surface-seen-right = van rechts gezien
surface-seen-above = van boven gezien, de deurkant onder
surface-seen-below = van onderen gezien, de deurkant boven
surface-caption = { $p0 }, { $p1 }
corner-bottom-left = Linksonder
corner-bottom-right = Rechtsonder
corner-top-left = Linksboven
corner-top-right = Rechtsboven
hinge-left = Links
hinge-right = Rechts
hinge-word = Scharnier

## Hoe een artikel gemonteerd wordt, en het label van een drager (doc 12 §2.5)

mount-screw = geschroefd
mount-busbar-adapter = stroomrailadapter
mount-door-cutout = in een uitsparing
mount-on-parent = op zijn ouder gemonteerd
mount-none = niet gemonteerd
carrier-rail = Rail { $p0 }
carrier-duct = Kabelgoot { $p0 }
carrier-rail-in-sentence = rail { $p0 }
carrier-duct-in-sentence = kabelgoot { $p0 }
