# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: GPL-3.0-or-later

# The texts of the window in nl-NL: dialogs, file prompts, confirmations and the dialogs that make a new thing. Every
# key of the en-GB file of the same name, with the same arguments; a message missing here falls back to en-GB. Product
# and file names, rule IDs, command names, key caps, identifiers and units are not translated.

prompt-open-image-title = Een afbeelding invoegen
prompt-export-dxf-title = DXF exporteren: kies een map voor de set
confirm-export-dxf-replace-title = De DXF-set in { $p0 } vervangen?
confirm-export-dxf-replace-text = { $p0 ->
        [one] Het ene bestand van de eerdere DXF-export verdwijnt, met elke wijziging die er sindsdien in is gemaakt; ongedaan maken brengt het niet terug
       *[other] De { $p0 } bestanden van de eerdere DXF-export verdwijnen, met elke wijziging die er sindsdien in is gemaakt; ongedaan maken brengt ze niet terug
    }.
dialog-replace = Vervangen
dialog-close = Sluiten
dialog-save = Opslaan
dialog-discard = Verwerpen
dialog-cancel = Annuleren
dialog-go = Ga
confirm-title = Wijzigingen in { $p0 } opslaan voor het sluiten?
dialog-dont-save = Niet opslaan
confirm-text = Het project heeft niet-opgeslagen wijzigingen.
prompt-open-title = Een projectmap openen
prompt-save-as-title = Kies een nieuwe, lege map
prompt-export-title = PDF exporteren
prompt-pdf-name = project.pdf
dialog-apply = Toepassen
open-failed = Het systeem kon het bestand niet openen.
dialog-save-workspace = Indeling opslaan
save-workspace-title = Indeling opslaan als
save-workspace-hint = Naam van de werkruimte
confirm-journal-title = Niet-opgeslagen wijzigingen verwerpen?
confirm-journal-text = Een eerdere sessie van dit project eindigde met niet-opgeslagen wijzigingen. Verwerpen verwijdert dat werk.
prompt-open-qet-title = Een QElectroTech-project openen
prompt-open-parts-title = Artikeltabel importeren
dialog-draw = Tekenen
prompt-plc-export-title = De PLC-configuratie exporteren
prompt-plc-import-title = Een PLC-uitwisselingsbestand importeren
process-tag-scheme-title = Schema voor proceskenmerken
prompt-export-table-title = Tabel exporteren
prompt-table-name = table.xlsx
prompt-open-table-title = Tabel importeren
dialog-waive = Vrijstellen
new-page-title = Nieuwe pagina
new-structure-title = Nieuw structuurknooppunt
new-external-title = Nieuwe externe verwijzing
new-section-title = Nieuwe sectie
dialog-create = Maken
dialog-export = Exporteren
new-strip-title = Nieuwe klemmenstrook
dialog-renumber = Hernummeren
dialog-page-copy = Kopiëren
confirm-purge-title = Ongebruikte afbeeldingen opruimen?
confirm-purge-text = Deze bestanden worden verwijderd en komen niet terug met ongedaan maken.
confirm-purge-more = en nog { $p0 }
dialog-purge = Opruimen
save-macro-title = Opslaan als macro
dialog-save-macro = In bibliotheek opslaan
dialog-part-choose-title = Een artikel kiezen voor { $p0 }
dialog-part-choose-button = Kiezen
confirm-part-remove-title = { $p0 } uit uw artikelbibliotheek verwijderen?
confirm-part-remove-text = Volgende projecten bieden het niet meer aan, en ongedaan maken brengt het niet terug; een project dat het gebruikt houdt zijn kopie.
confirm-part-leave-title = De wijzigingen aan { $p0 } opslaan?
confirm-part-leave-text = Het artikel heeft wijzigingen die niet zijn opgeslagen. Weggaan zonder opslaan laat ze vervallen.
confirm-part-card-text = Wat Opslaan in dit project wijzigt, wacht op de kaart naast het artikel. Opslaan past het toe voordat het andere artikel opengaat; Verwerpen laat het project zoals het is.
dialog-stay = Blijven
dialog-remove = Verwijderen
prompt-open-keymap-title = Toetsenindeling importeren
prompt-save-keymap-title = Toetsenindeling exporteren
prompt-keymap-name = keymap.toml
dialog-approve = Goedkeuren
dialog-release = Vrijgeven
dialog-withdraw = Intrekken
dialog-set-author = Auteur instellen
dialog-complete-release = Vrijgave voltooien
dialog-discard-archive = Archief verwerpen
dialog-not-now = Niet nu
prompt-export-ewz-title = .ewz exporteren
prompt-ewz-name = project.ewz
prompt-open-ewz-title = .ewz openen
prompt-import-title = Een QElectroTech-project, een tekening, een PDF, EPLAN-gegevens of een rapport importeren, of een projectarchief (.ewz) openen
prompt-import-filter = Projecten, archieven, tekeningen, PDF's, EPLAN-bestanden en rapporten
dialog-connect = Verbinden
prompt-open-symbol-title = Een symbool openen of importeren
prompt-save-symbol-title = Symboolconcept exporteren
prompt-symbol-name = symbool.json
prompt-library-folder-title = Een map voor de symboolbibliotheek kiezen
prompt-symbol-filter = Symboolbestanden
prompt-image-filter = Afbeeldingen
prompt-converter-title = Kies de ODA File Converter
dialog-put-station = Hier zetten
prompt-folder-title = Kies de map waarin het project komt
new-project-title = Nieuw project
new-project-name = Naam
new-project-folder = Map
new-project-browse = Bladeren…
new-project-made-in = Het project komt in { $p0 }
new-project-start-from = Beginnen met
new-project-empty = Leeg project
new-project-empty-what = Eén lege pagina, met het kader en titelblok en de instellingen van het programma.
new-project-other = Een ander project…
new-project-other-what = Kies een project om mee te beginnen: zijn instellingen, en zijn pagina's alleen als u het vakje aanvinkt.
new-project-for-new = voor nieuwe projecten
new-project-copy-pages = { $p0 ->
        [one] Ook zijn pagina en de apparaten erop kopiëren
       *[other] Ook zijn { $p0 } pagina's en de apparaten erop kopiëren
    }
new-project-pages-note = Zonder dit vakje begint het project met één lege pagina.
new-project-default-name = Nieuw project
new-project-need-name = Typ een naam voor het project.
new-project-bad-name = Een naam mag geen \ / : * ? " < > | bevatten en niet op een punt of spatie eindigen.
new-project-need-folder = Kies de map waarin het project komt: Bladeren….
new-project-folder-whole = Typ het hele pad van de map, of kies hem met Bladeren….
new-project-need-other = Kies het project om mee te beginnen: klik nog eens op Een ander project….
new-project-start-unreadable = Dat sjabloon kan nu niet worden gelezen: kies een ander begin.
new-project-taken = Er is al een map met die naam: kies een andere naam.
new-project-other-unreadable = Die map bevat geen project dat het programma kan lezen: { $p0 }
save-template-title = Opslaan als sjabloon
save-template-ok = Opslaan als sjabloon
save-template-text = Het sjabloon houdt de instellingen, kaders en titelblokken, tekststijlen, secties en structuur van dit project. De pagina's en hun apparaten gaan er alleen in als u het vakje aanvinkt.
save-template-default-name = Sjabloon { $p0 }
save-template-failed = Het sjabloon kon niet worden geschreven: { $p0 }
new-project-other-change = Een ander kiezen…
# Nieuw kabeltype… (het kleine formulier van een kabelartikel) en het kabeldeel van een kabelartikel.
prompt-open-font-title = Kies een lettertypebestand voor het standaardlettertype
prompt-open-outline-title = Kies een DXF-tekening van de voorkant van het artikel
new-project-folder-new = Deze map bestaat nog niet: Maken maakt hem aan.
new-project-folder-file = Een bestand met die naam staat de map in de weg: typ een andere map, of kies er een met Bladeren….
new-project-folder-no-drive = Dat station is er niet: typ een map op een station van deze computer, of kies er een met Bladeren….
edit-section-title = Sectie bewerken
form-field-new-title = Nieuw eigen veld
dialog-create-field = Veld maken
form-field-name = Naam
form-field-for = Eén waarde voor
form-field-value = Waarde nu

## Output ▸ Workshop ▸ Rework list… (doc 13 §3.5)

rework-title = Wijzigingslijst
rework-compare-with = Vergelijken met
rework-archive-file = Archiefbestand…
rework-against = het project zoals het nu is tegen het archief van { $p0 }
rework-release = { $p0 }, vrijgegeven { $p1 }
rework-export = Exporteren…
rework-work = Het werk: { $p0 }.
rework-nothing = Niets te veranderen: het project is zoals het vrijgegeven is.
rework-not-listed = Niet vermeld: draden die alleen op een pagina verschoven of een andere route kregen, en een nieuw draadnummer.
rework-count-remove-device = { $p0 ->
        [one] 1 apparaat verwijderen
       *[other] { $p0 } apparaten verwijderen
    }
rework-count-remove-terminal = { $p0 ->
        [one] 1 klem verwijderen
       *[other] { $p0 } klemmen verwijderen
    }
rework-count-remove-cable = { $p0 ->
        [one] 1 kabel verwijderen
       *[other] { $p0 } kabels verwijderen
    }
rework-count-remark = { $p0 ->
        [one] 1 merker vervangen
       *[other] { $p0 } merkers vervangen
    }
rework-count-relabel = { $p0 ->
        [one] 1 label vervangen
       *[other] { $p0 } labels vervangen
    }
rework-count-mount-device = { $p0 ->
        [one] 1 apparaat monteren
       *[other] { $p0 } apparaten monteren
    }
rework-count-add-terminal = { $p0 ->
        [one] 1 klem toevoegen
       *[other] { $p0 } klemmen toevoegen
    }
rework-count-add-cable = { $p0 ->
        [one] 1 kabel toevoegen
       *[other] { $p0 } kabels toevoegen
    }
rework-count-pull = { $p0 ->
        [one] 1 draad uittrekken
       *[other] { $p0 } draden uittrekken
    }
rework-count-replace = { $p0 ->
        [one] 1 draad vervangen
       *[other] { $p0 } draden vervangen
    }
rework-count-move-end = { $p0 ->
        [one] 1 einde verleggen
       *[other] { $p0 } einden verleggen
    }
rework-count-remake-end = { $p0 ->
        [one] 1 einde opnieuw afwerken
       *[other] { $p0 } einden opnieuw afwerken
    }
rework-count-check-length = { $p0 ->
        [one] 1 lengte nakijken
       *[other] { $p0 } lengtes nakijken
    }
rework-count-add = { $p0 ->
        [one] 1 draad toevoegen
       *[other] { $p0 } draden toevoegen
    }
rework-no-release = Dit project heeft nog geen vrijgegeven revisie: de wijzigingslijst vergelijkt met een vrijgave. Geef het vrij op het tabblad Controle, of kies een archiefbestand.
rework-no-archive = Het archief van { $p0 }, { $p1 }, staat niet in de map naast het project: kies het archiefbestand.
rework-other-archive = { $p0 } is niet het archief van { $p1 }: het is een ander bestand met die naam. Kies het archiefbestand.
rework-not-opened = Het archief opent niet: { $p0 }
rework-labels = Labels voor deze wijzigingen…

## Output ▸ Workshop ▸ Labels… (doc 13 §5.4)

labels-title = Labels
labels-which = Labels
labels-all = Alle labels
labels-since = Gewijzigd sinds { $p0 }
labels-archive = Gewijzigd sinds het archief { $p0 }
labels-profile = Labelbestand
labels-change = Wijzigen…
labels-file = Bestand
labels-set = Labelset
labels-marker-part = Merkerartikel
labels-count = Labels
labels-misfits = { $p0 ->
        [one] 1 label past niet op zijn merkerartikel
       *[other] { $p0 } labels passen niet op hun merkerartikel
    }
labels-export = Exporteren…
labels-show = Tonen
labels-hide = Verbergen
labels-misfit = { $p0 } op { $p1 }: { $p2 }
labels-profile-title = Labelbestand
labels-generic = Algemene kolommen
