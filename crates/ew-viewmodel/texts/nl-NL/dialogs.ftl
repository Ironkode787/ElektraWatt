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
prompt-new-project-title = Het project in een nieuwe map maken
prompt-basic-project-title = Het sjabloon naar een nieuwe map schrijven
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
dialog-part-edit-title = Artikel { $p0 } bewerken
dialog-part-choose-button = Kiezen
dialog-part-new-title = Nieuw artikel
confirm-part-remove-title = { $p0 } uit uw artikelbibliotheek verwijderen?
confirm-part-remove-text = Volgende projecten bieden het niet meer aan, en ongedaan maken brengt het niet terug; een project dat het gebruikt houdt zijn kopie.
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
# rc.3: Nieuw kabeltype… (het kleine formulier van een kabelartikel) en het kabeldeel van een kabelartikel.
dialog-cable-type-new-title = Nieuw kabeltype
prompt-open-font-title = Kies een lettertypebestand voor het standaardlettertype
new-project-folder-new = Deze map bestaat nog niet: Maken maakt hem aan.
new-project-folder-file = Een bestand met die naam staat de map in de weg: typ een andere map, of kies er een met Bladeren….
new-project-folder-no-drive = Dat station is er niet: typ een map op een station van deze computer, of kies er een met Bladeren….
edit-section-title = Sectie bewerken
dialog-bus-cable-new-title = Nieuwe buskabel
dialog-bus-plug-new-title = Nieuwe busstekker
