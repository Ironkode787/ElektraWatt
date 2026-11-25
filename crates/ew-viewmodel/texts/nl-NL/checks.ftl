# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: GPL-3.0-or-later

# The texts of the window in nl-NL: messages, checks, releases and revisions. Every key of the en-GB file of the same
# name, with the same arguments; a message missing here falls back to en-GB. Product and file names, rule IDs, command
# names, key caps, identifiers and units are not translated.

messages-pending-note = Wachtende bevindingen staan op de pagina die u bewerkt: ze tellen mee zodra u die pagina verlaat of een controle uitvoert.
check-error = 1 fout
check-errors = { $p0 } fouten
check-warning = 1 waarschuwing
check-warnings = { $p0 } waarschuwingen
check-info = 1 info
check-infos = { $p0 } info's
check-waived = { $p0 ->
        [one] 1 vrijgesteld
       *[other] { $p0 } vrijgesteld
    }
check-pending = { $p0 ->
        [one] 1 wachtend
       *[other] { $p0 } wachtend
    }
# After an earlier check, the toast leads with what changed since, then the project's totals.
check-since = { $p0 } sinds de vorige controle; project: { $p1 }
check-unchanged = geen wijziging
check-changed = { $p0 ->
        [one] 1 bevinding gewijzigd
       *[other] { $p0 } bevindingen gewijzigd
    }
severity-error = Fout
severity-warning = Waarschuwing
severity-info = Info
severity-errors = Fouten
severity-warnings = Waarschuwingen
severity-infos = Info's
messages-header = { $p0 } · F { $p1 } · W { $p2 } · I { $p3 } · { $p4 } vrijgesteld · { $p5 } wachtend
messages-header-waived = { $p0 } vrijgesteld
messages-header-pending = { $p0 } wachtend
messages-detail-rule = { $p0 } { $p1 }
messages-detail-kind = Soort: { $p0 } · ernst: { $p1 }
messages-detail-fix = Snelle oplossing: { $p0 }
messages-fix-button = Oplossen…
messages-rule-page = Regelpagina
messages-waive-button = Vrijstellen…
messages-unwaive-button = Vrijstelling opheffen
messages-detail-waived = Vrijgesteld
waive-title = Bevinding vrijstellen
revision-approve-title = Revisie { $p0 } goedkeuren
revision-release-title = Revisie { $p0 } vrijgeven?
revision-withdraw-title = Een revisie intrekken
revision-author-title = De auteur van { $p0 } instellen
revision-incomplete-title = De onvolledige vrijgave van { $p0 } afhandelen?
revision-approve-text = De controleur keurt de opgeslagen inhoud van { $p0 } goed, geschreven door de { $p1 }. De vrijgavepoort draait eerst op het opgeslagen project; de goedkeuring wordt opgeslagen.
revision-no-author = { $p0 } heeft nog geen auteur: stel die eerst in (Auteur van revisie instellen…) en sla dan op.
revision-gate-error = De vrijgavepoort kan niet draaien: { $p0 }.
revision-approve-blocked = { $p0 ->
        [one] De vrijgavepoort heeft 1 fout naast de vier-ogenregel; corrigeer die of stel die eerst vrij:
       *[other] De vrijgavepoort heeft { $p0 } fouten naast de vier-ogenregel; corrigeer ze of stel ze eerst vrij:
    }
revision-release-blocked = { $p0 ->
        [one] De vrijgave wordt geblokkeerd door 1 bevinding van de vrijgavepoort:
       *[other] De vrijgave wordt geblokkeerd door { $p0 } bevindingen van de vrijgavepoort:
    }
revision-finding = { $p0 }: { $p1 }
revision-more = en nog { $p0 }
revision-waivers = { $p0 ->
        [one] Goedkeuren erkent ook 1 geldende veiligheidsvrijstelling
       *[other] Goedkeuren erkent ook { $p0 } geldende veiligheidsvrijstellingen
    }.
revision-release-text = De vrijgave doorloopt zes stappen en kan niet ongedaan worden gemaakt:
revision-step-gate = 1. De vrijgavepoort op het opgeslagen project uitvoeren
revision-step-freeze = 2. De nummering van de revisie vastzetten en het project opslaan
revision-step-commit = 3. De projectmap committen in git
revision-step-no-commit = 3. Geen git-commit: de projectmap staat niet in een git-repository
revision-step-archive = 4. Het archief { $p0 } in de map { $p1 } schrijven en verifiëren
revision-step-tag = 5. De git-tag { $p0 } maken
revision-step-no-tag = 5. Geen git-tag
revision-step-stamp = 6. { $p0 } als vrijgegeven stempelen en opslaan
revision-withdraw-text = De intrekking is een record na de revisie die ze intrekt; de vrijgegeven revisie en haar archief blijven.
revision-author-text = De partij met deze rol is de auteur van { $p0 }; de controleur die haar goedkeurt, moet een andere rol hebben.
revision-incomplete-text = Een vrijgave van { $p0 } stopte vóór haar stempel en liet het archief { $p1 } achter.
revision-incomplete-complete = Vrijgave voltooien stempelt de revisie daarmee als vrijgegeven, en commit en tagt haar wanneer de projectmap in een git-repository staat.
revision-incomplete-refused = Ze kan niet worden voltooid: { $p0 }.
revision-incomplete-unproven = Het is niet aantoonbaar een restant van deze map zelf, dus het wordt niet aangeboden om te verwerpen; verwijder het bestand met de hand als u weet dat het er een is.
release-source = Vrijgave
release-trouble-not-removed = Het vrijgaverecord { $p0 } kon niet worden verwijderd{ $p1 }.
release-trouble-not-read = De vrijgaverecords in de map { $p0 } konden niet worden gelezen{ $p1 }.
release-trouble-not-updated = Het vrijgaverecord van { $p0 } kon niet worden bijgewerkt met de tijd van het archief{ $p1 }, dus verwerpen na een crash behoudt het archief.
release-cause-not-found = : het is niet gevonden
release-cause-denied = : toegang is geweigerd
release-cause-not-folder = : het is geen map
revision-complete-title = De vrijgave van { $p0 } voltooien?
revision-complete-text = Voltooien stempelt { $p0 } als vrijgegeven met het archief { $p1 } en slaat het project op. Het kan niet ongedaan worden gemaakt.
revision-complete-git = De projectmap staat in een git-repository: voltooien commit haar en maakt de tag { $p0 }.
revision-discard-title = Het archief van { $p0 } verwerpen?
revision-discard-text = Verwerpen verwijdert het archief { $p0 }, en de git-tag die deze vrijgave eventueel heeft gemaakt; { $p1 } blijft open. Het kan niet ongedaan worden gemaakt.
revision-waiver = { $p0 } op { $p1 }
revision-waiver-gone = een vrijstelling waarvan de controle de bevinding niet meer meldt
approve-read-only = het project is alleen-lezen geopend
approve-four-eyes = de { $p0 } is de partij van de auteur; de vier-ogenregel vraagt om een controleur van een andere partij
approve-blocked = { $p0 ->
        [one] de vrijgavepoort heeft 1 fout naast de vier-ogenregel
       *[other] de vrijgavepoort heeft { $p0 } fouten naast de vier-ogenregel
    }
approve-unacknowledged = de goedkeuring erkent de geldende veiligheidsvrijstellingen niet: { $p0 }
release-error-person = een vrijgave vraagt om een persoon aan het venster, nooit een script, een herhaling of een plug-in
release-error-read-only = het project is alleen-lezen geopend
release-error-gate = de vrijgavepoort kan niet draaien: { $p0 }
release-error-git-consent = de projectmap staat in een git-repository; vink “Committen en taggen in git” aan om in te stemmen met de commit en de tag { $p0 }
revision-release-cannot = De vrijgave kan niet beginnen: { $p0 }.
release-error-blocked = { $p0 ->
        [one] de vrijgavepoort heeft 1 bevinding die haar blokkeert
       *[other] de vrijgavepoort heeft { $p0 } bevindingen die haar blokkeren
    }
release-error-archive-root = de archiefmap { $p0 } kan niet worden gebruikt: { $p1 }
release-error-incomplete = het archief { $p0 } bestaat al: een eerdere vrijgave stopte vóór haar stempel; handel die eerst af (De onvolledige vrijgave afhandelen…)
release-error-tag-exists = de git-tag { $p0 } bestaat al, en een vrijgave verplaatst nooit een tag
release-error-git = een git-stap is mislukt voordat het archief werd geschreven; er is niets vrijgegeven
release-error-save = het project kon niet worden opgeslagen, dus de vrijgave is gestopt
release-error-generation = het opgeslagen project kon niet worden teruggelezen voor het archief, dus de vrijgave is gestopt
release-error-output = de PDF kon niet worden geschreven, dus de vrijgave is gestopt
release-error-archive = het archief kon niet worden opgebouwd, of het leest niet
release-error-write = het archief kon niet worden geschreven, dus de vrijgave is gestopt
release-error-tag = de git-tag kon niet worden gemaakt; het archief van deze uitvoering is verwijderd
release-error-stamp = de revisie kon niet als vrijgegeven worden gestempeld; het archief en de git-tag van deze uitvoering zijn verwijderd
release-error-unsettled = de vrijgave is opgeslagen, maar het venster kon haar stempel niet overnemen; open het project opnieuw
release-error-undecided = de stempel kon niet volledig worden opgeslagen; open het project opnieuw: het voltooit de vrijgave of biedt aan haar af te handelen
release-error-io = { $p0 } kon niet worden gebruikt{ $p1 }
release-error-no-incomplete = er is geen onvolledige vrijgave
release-error-not-completable = ze kan niet worden voltooid: { $p0 }
release-error-foreign = { $p0 }; het blijft bewaard
release-error-unproven = { $p0 }; het blijft bewaard: voltooi de vrijgave ermee, of verwijder het bestand met de hand als u weet dat het een restant is
release-error-unproven-kept = { $p0 }; het blijft bewaard: verwijder het bestand met de hand als u weet dat het een restant is
finding-kind-safety = Veiligheid
finding-kind-conflict = Conflict
finding-kind-completeness = Volledigheid
finding-kind-hygiene = Netheid
finding-kind-integrity = Integriteit
messages-scheme-migration = Migratierapport
messages-fix-all = Alles oplossen ({ $p0 })…
messages-fix-all-like = Alle zoals deze oplossen ({ $p0 })…
check-level-title = Controleniveau
check-level-purpose = Voordat tekeningen de deur uitgaan, controleert het programma ze. Het controleniveau zegt hoe streng die controle is.
check-level-pre-delivery = Wat goed moet zijn voordat tekeningen de deur uitgaan, zoals een onderbrekingspunt zonder zijn partner op een andere pagina. Een paar netheidscontroles vallen weg, zoals een structuurcode met een scheidingsteken erin.
check-level-full = Elke controle, ook de netheidscontroles, zoals een structuurcode met een scheidingsteken erin (=P.01).
messages-offer-cable-type = Kies een kabeltype…
messages-offer-length = Typ de lengte…
messages-offer-plc-section = Open het PLC-deel van het artikel…
