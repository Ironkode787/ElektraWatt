; SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
; SPDX-License-Identifier: MIT OR Apache-2.0

; The per-user setup of ElektraWatt (WP-S6.06 D2, D9; doc 18 §8, `ElektraWatt-<v>-x64-user-setup.exe`), compiled by
; makensis (NSIS 3, pinned in packaging/components.toml) from the tree that packaging/stage.py staged:
;
;   makensis -DVERSION=<v> -DTREE=<staged tree> -DOUTFILE=<dir>\ElektraWatt-<v>-x64-user-setup.exe elektrawatt.nsi
;
; - No administrator rights (`RequestExecutionLevel user`: the manifest asks for asInvoker); the default folder is
;   %LOCALAPPDATA%\Programs\ElektraWatt; `/S` installs silently, `/D=<folder>` (last on the command line) chooses it.
; - Modern UI pages: the folder, the progress, the finish page with NSIS's "Run ElektraWatt" ("ElektraWatt starten").
;   English and Dutch, chosen by the user's Windows language; every text is NSIS's own translation but the shortcut's
;   name, "ElektraWatt". The setup and the shortcut show Windows' default icon (the executable has none yet, FP-4).
; - It installs D3's tree, the uninstaller `uninstall.exe`, a Start-menu shortcut for the current user, and the
;   uninstall entry HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\ElektraWatt; nothing under HKLM.
; - Upgrade: a setup that finds the entry proposes its folder, runs the old uninstaller silently in its folder and
;   waits for it, then installs, so no file of an older version remains.
; - A folder that holds files is refused unless it holds both uninstall.exe and elektrawatt.exe, or is the folder
;   that the entry names (the page keeps Next off; `/S` exits with 2).
; - The uninstall removes D3's entries by name, the uninstaller, the shortcut and the entry, then the folder when it is
;   empty: never another file of that folder, nor the user's settings, data or cache folders.
; - zlib compression, the default: NSIS's LZMA module is under CPL-1.0 (its COPYING), the zlib one under the zlib
;   licence of NSIS itself, so the setup's code stays under that one licence.
; package.yml's `windows` job proves it on the hosted runner (packaging/windows/install-test.ps1, IN-8 to IN-13).

!ifndef VERSION
  !error "makensis -DVERSION=<v>: the version that names the setup and its uninstall entry"
!endif
!ifndef TREE
  !error "makensis -DTREE=<dir>: the installed tree that packaging/stage.py staged"
!endif
!ifndef OUTFILE
  !error "makensis -DOUTFILE=<file>: the setup to write"
!endif

!define UNINSTALL_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\ElektraWatt"

Unicode true
SetCompressor zlib
RequestExecutionLevel user
ManifestDPIAware true
Name "ElektraWatt"
OutFile "${OUTFILE}"
InstallDir "$LOCALAPPDATA\Programs\ElektraWatt"
; An upgrade proposes, and `/S` uses, the folder of the installation that the entry names.
InstallDirRegKey HKCU "${UNINSTALL_KEY}" "InstallLocation"

!include "MUI2.nsh"
!include "LogicLib.nsh"
!include "FileFunc.nsh"

!define MUI_FINISHPAGE_RUN "$INSTDIR\elektrawatt.exe"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"
!insertmacro MUI_LANGUAGE "Dutch"

; A folder that holds files is refused unless it is an installation of ElektraWatt: it holds both uninstall.exe and
; elektrawatt.exe, or it is the folder that the uninstall entry names. The uninstall removes D3's folders `docs`,
; `content`, `samples` and `LICENSES` whole, and the setup writes its own uninstall.exe, so in another program's or
; the user's folder it would delete their files. Sets $R0 to "refused" or "accepted".
!macro CheckFolder
  StrCpy $R0 "accepted"
  ${DirState} "$INSTDIR" $R1
  ${If} $R1 == 1
    StrCpy $R0 "refused"
    ${If} ${FileExists} "$INSTDIR\uninstall.exe"
    ${AndIf} ${FileExists} "$INSTDIR\elektrawatt.exe"
      StrCpy $R0 "accepted"
    ${EndIf}
    ReadRegStr $R2 HKCU "${UNINSTALL_KEY}" "InstallLocation"
    ${If} $R2 != ""
    ${AndIf} $R2 == $INSTDIR
      StrCpy $R0 "accepted"
    ${EndIf}
  ${EndIf}
!macroend

; The folder page keeps Next off for a refused folder.
Function .onVerifyInstDir
  !insertmacro CheckFolder
  ${If} $R0 == "refused"
    Abort
  ${EndIf}
FunctionEnd

Section "ElektraWatt"
  SetShellVarContext current
  ; The same refusal for `/S /D=<folder>`, which shows no folder page: the setup ends with exit code 2, writing nothing.
  !insertmacro CheckFolder
  ${If} $R0 == "refused"
    SetErrorLevel 2
    Abort
  ${EndIf}
  ; The upgrade: the installation that the entry names goes first, through its own uninstaller run in place (`_?=`,
  ; so ExecWait waits for it), which leaves only itself; then that file and its folder.
  ReadRegStr $0 HKCU "${UNINSTALL_KEY}" "InstallLocation"
  ${If} $0 != ""
  ${AndIf} ${FileExists} "$0\uninstall.exe"
    ClearErrors
    ExecWait '"$0\uninstall.exe" /S _?=$0' $1
    ${If} ${Errors}
    ${OrIf} $1 != 0
      Abort
    ${EndIf}
    Delete "$0\uninstall.exe"
    RMDir "$0"
  ${EndIf}

  SetOutPath "$INSTDIR"
  File /r "${TREE}\*"
  WriteUninstaller "$INSTDIR\uninstall.exe"
  CreateShortcut "$SMPROGRAMS\ElektraWatt.lnk" "$INSTDIR\elektrawatt.exe"

  ${GetSize} "$INSTDIR" "/S=0K" $2 $3 $4
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayName" "ElektraWatt"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "Publisher" "The ElektraWatt contributors"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegStr HKCU "${UNINSTALL_KEY}" "QuietUninstallString" '"$INSTDIR\uninstall.exe" /S'
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoRepair" 1
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "EstimatedSize" $2
SectionEnd

Section "Uninstall"
  SetShellVarContext current
  Delete "$SMPROGRAMS\ElektraWatt.lnk"
  DeleteRegKey HKCU "${UNINSTALL_KEY}"
  ; D3's entries, one by one (packaging/stage.py --test compares them with the staged tree).
  Delete "$INSTDIR\elektrawatt.exe"
  Delete "$INSTDIR\elektrawatt.pdb"
  Delete "$INSTDIR\ew.exe"
  Delete "$INSTDIR\ew.pdb"
  Delete "$INSTDIR\NOTICE"
  Delete "$INSTDIR\PRIVACY.md"
  Delete "$INSTDIR\README.txt"
  Delete "$INSTDIR\SECURITY.md"
  Delete "$INSTDIR\THIRD-PARTY-NOTICES.txt"
  RMDir /r "$INSTDIR\LICENSES"
  RMDir /r "$INSTDIR\content"
  RMDir /r "$INSTDIR\docs"
  RMDir /r "$INSTDIR\samples"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"
SectionEnd
