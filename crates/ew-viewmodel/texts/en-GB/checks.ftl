# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: GPL-3.0-or-later

# The texts of the window in en-GB: messages, checks, releases and revisions. One message per key; `{ $p0 }`,
# `{ $p1 }`, ... are the arguments, always strings. The source language and the fallback of every other.

# The Messages panel's sentence at its top while findings are pending.
messages-pending-note = Pending findings are on the page you are editing: they count once you leave that page or run a check.
messages-group-error = Errors
messages-group-warning = Warnings
messages-group-info = Infos
check-error = 1 error
check-errors = { $p0 } errors
check-warning = 1 warning
check-warnings = { $p0 } warnings
check-info = 1 info
check-infos = { $p0 } infos
check-waived-one = 1 waived
check-waived = { $p0 } waived
check-pending-one = 1 pending
check-pending = { $p0 } pending
# After an earlier check, the toast leads with what changed since, then the project's totals.
check-since = { $p0 } since the last check; project: { $p1 }
check-unchanged = no change
# The same counts as the last check, other findings (the new ones counted).
check-changed-one = 1 finding changed
check-changed = { $p0 } findings changed
severity-error = Error
severity-warning = Warning
severity-info = Info
severity-errors = Errors
severity-warnings = Warnings
severity-infos = Infos
messages-header = { $p0 } · E { $p1 } · W { $p2 } · I { $p3 } · waived { $p4 } · pending { $p5 }
messages-header-waived = waived { $p0 }
messages-header-pending = pending { $p0 }
messages-detail-rule = { $p0 } { $p1 }
messages-detail-kind = Kind: { $p0 } · severity: { $p1 }
messages-detail-fix = Quick fix: { $p0 }
messages-fix-button = Fix…
messages-rule-page = Rule page
messages-waive-button = Waive…
messages-unwaive-button = Unwaive
messages-detail-waived = Waived
waive-title = Waive finding
revision-approve-title = Approve revision { $p0 }
revision-release-title = Release revision { $p0 }?
revision-withdraw-title = Withdraw a revision
revision-author-title = Set the author of { $p0 }
revision-incomplete-title = Settle the incomplete release of { $p0 }?
revision-approve-text = The checker signs off the saved content of { $p0 }, written by the { $p1 }. The release gate runs on the saved project first; the sign-off is saved.
revision-no-author = { $p0 } has no author yet: set it first (Set revision author…), then save.
revision-gate-error = The release gate cannot run: { $p0 }.

# FIX41b rulings 1 and 4: the rule by its title, the noun in the count's number.
revision-approve-blocked = The release gate has { $p0 } errors besides the four-eyes rule; correct or waive them first:
revision-approve-blocked-one = The release gate has 1 error besides the four-eyes rule; correct or waive it first:
revision-release-blocked = The release is blocked by { $p0 } findings of the release gate:
revision-release-blocked-one = The release is blocked by 1 finding of the release gate:
revision-finding = { $p0 }: { $p1 }
revision-more = and { $p0 } more
revision-waivers = { $p0 ->
        [one] Approving also acknowledges 1 safety waiver in force
       *[other] Approving also acknowledges { $p0 } safety waivers in force
    }.
revision-release-text = The release runs six steps and cannot be undone:
revision-step-gate = 1. Run the release gate on the saved project
revision-step-freeze = 2. Freeze the revision's numbering and save the project
revision-step-commit = 3. Commit the project folder in git
revision-step-no-commit = 3. No git commit: the project folder is not in a git repository
revision-step-archive = 4. Write and verify the archive { $p0 } in the folder { $p1 }
revision-step-tag = 5. Create the git tag { $p0 }
revision-step-no-tag = 5. No git tag
revision-step-stamp = 6. Stamp { $p0 } released and save
revision-withdraw-text = The withdrawal is a record after the revision it withdraws; the released revision and its archive stay.
revision-author-text = The party with this role is the author of { $p0 }; the checker who approves it must have another role.
revision-incomplete-text = A release of { $p0 } stopped before its stamp and left the archive { $p1 }.
revision-incomplete-complete = Complete release stamps the revision released with it, committing and tagging it when the project folder is in a git repository.
revision-incomplete-refused = It cannot be completed: { $p0 }.
revision-incomplete-unproven = It is not provably this folder's own leftover, so it is not offered for discarding; remove the file by hand if you know it is one.
release-source = Release

# The cause in words from the error's kind ({1}: a cause below, or nothing).
release-trouble-not-removed = The release record { $p0 } could not be removed{ $p1 }.
release-trouble-not-read = The release records in the folder { $p0 } could not be read{ $p1 }.
release-trouble-not-updated = The release record of { $p0 } could not be updated with the archive's time{ $p1 }, so a discard after a crash keeps the archive.
release-cause-not-found = : it was not found
release-cause-denied = : permission was denied
release-cause-not-folder = : it is not a folder

# Completing or discarding an incomplete release asks first.
revision-complete-title = Complete the release of { $p0 }?
revision-complete-text = Completing stamps { $p0 } released with the archive { $p1 } and saves the project. It cannot be undone.
revision-complete-git = The project folder is in a git repository: completing commits it and creates the tag { $p0 }.
revision-discard-title = Discard the archive of { $p0 }?
revision-discard-text = Discarding deletes the archive { $p0 }, and the git tag this release created, if any; { $p1 } stays open. It cannot be undone.
revision-waiver = { $p0 } on { $p1 }
revision-waiver-gone = a waiver whose finding the check no longer reports
approve-read-only = the project is open read-only
approve-four-eyes = the { $p0 } is the author's party; the four-eyes rule needs a checker of another party
approve-blocked = the release gate has { $p0 } errors besides the four-eyes rule
approve-blocked-one = the release gate has 1 error besides the four-eyes rule
approve-unacknowledged = the sign-off does not acknowledge the safety waivers in force: { $p0 }
release-error-person = a release needs a person at the window, never a script, a replay or a plugin
release-error-read-only = the project is open read-only
release-error-gate = the release gate cannot run: { $p0 }
release-error-git-consent = the project folder is in a git repository; tick “Commit and tag in git” to agree to the commit and the tag { $p0 }
revision-release-cannot = The release cannot start: { $p0 }.
release-error-blocked = the release gate has { $p0 } findings that block it
release-error-blocked-one = the release gate has 1 finding that blocks it
release-error-archive-root = the archive folder { $p0 } cannot be used: { $p1 }
release-error-incomplete = the archive { $p0 } exists already: an earlier release stopped before its stamp; settle it first (Settle the incomplete release…)
release-error-tag-exists = the git tag { $p0 } exists already, and a release never moves a tag
release-error-git = a git step failed before the archive was written; nothing was released
release-error-save = the project could not be saved, so the release stopped
release-error-generation = the saved project could not be read back for the archive, so the release stopped
release-error-output = the PDF could not be written, so the release stopped
release-error-archive = the archive could not be built, or it does not read
release-error-write = the archive could not be written, so the release stopped
release-error-tag = the git tag could not be created; the archive of this run was removed
release-error-stamp = the revision could not be stamped released; the archive and the git tag of this run were removed
release-error-unsettled = the release is saved, but the window could not take its stamp; reopen the project
release-error-undecided = the stamp could not be saved completely; reopen the project: it completes the release, or offers to settle it
release-error-io = { $p0 } could not be used{ $p1 }
release-error-no-incomplete = there is no incomplete release
release-error-not-completable = it cannot be completed: { $p0 }
release-error-foreign = { $p0 }; it is kept
release-error-unproven = { $p0 }; it is kept: complete the release with it, or remove the file by hand if you know it is a leftover
release-error-unproven-kept = { $p0 }; it is kept: remove the file by hand if you know it is a leftover
# Ruling 3: the finding detail's kind, by rule kind (`ew_checks::RuleKind`).
finding-kind-safety = Safety
finding-kind-conflict = Conflict
finding-kind-completeness = Completeness
finding-kind-hygiene = Hygiene
finding-kind-integrity = Integrity
messages-scheme-migration = Migration report
messages-fix-all = Fix all ({ $p0 })…
messages-fix-all-like = Fix all like this ({ $p0 })…
check-level-title = Check level
check-level-purpose = Before drawings go out, the program checks them. The check level says how strict that check is.
check-level-pre-delivery = What must be right before drawings go out, such as an interruption point without its partner on another page. A few tidiness checks are left out, such as a structure code with a separator in it.
check-level-full = Every check, the tidiness checks too, such as a structure code with a separator in it (=P.01).
messages-offer-cable-type = Choose a cable type…
messages-offer-length = Type the length…
messages-offer-plc-section = Open the part's PLC section…
