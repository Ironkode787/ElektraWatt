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
