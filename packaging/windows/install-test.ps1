# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: MIT OR Apache-2.0

# The install test of the per-user setup (WP-S6.06 D11, CI-R-10's CI half; IN-8, IN-9, IN-10, IN-13), run by the
# `windows` job of package.yml on the hosted windows-2025 runner, from the checkout. Never run `install` or
# `again-and-uninstall` on a workstation: they install into the user's %LOCALAPPDATA%, write the user's registry and
# Start menu, and add firewall rules (as an administrator, as the runner is).
#
#   install-test.ps1 install -Setup <setup.exe> -Tree <staged tree> -Version <v>
#       IN-9: outbound firewall rules block the setup, elektrawatt.exe, ew.exe and the elektrawatt.exe of D13's three
#       copies by program, the firewall on. D9: `/S /D=<folder>` refuses a folder holding docs\keep.txt.
#       IN-8: the setup's manifest asks for asInvoker; `/S` installs into %LOCALAPPDATA%\Programs\ElektraWatt; the
#       key names under HKLM\SOFTWARE and the file names under both Program Files folders are the same before and
#       after; the installed folder is the staged tree (every file's SHA-256) and the uninstaller; the Start-menu
#       shortcut opens elektrawatt.exe; the uninstall entry holds D9's values, DisplayVersion <v>.
#   install-test.ps1 deny -Dir <dir> | allow -Dir <dir>
#       IN-10: denies the running user writing and deleting in <dir> (inherited by everything in it) and proves a
#       write fails; `allow` removes that denial.
#   install-test.ps1 again-and-uninstall -Setup <setup.exe> -Tree <staged tree> -Version <v>
#       IN-13: a settings file and a keymap file in the user's settings folder (%APPDATA%\ElektraWatt, where the
#       shell reads them), then the setup once more with `/S` over the installation (D9's upgrade: the old
#       uninstaller first), IN-8's checks of the installation (the folder, the shortcut and the entry; not the
#       machine listing, which IN-13 does not ask for), then the uninstall as Windows runs it (QuietUninstallString):
#       the folder, the shortcut and the entry are gone, the two files are the same bytes, and the firewall rules
#       are still in force (IN-9: for the whole test).
param(
    [Parameter(Mandatory)][ValidateSet('install', 'deny', 'allow', 'again-and-uninstall')][string]$Mode,
    [string]$Setup,
    [string]$Tree,
    [string]$Version,
    [string]$Dir
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$Root = Join-Path $env:LOCALAPPDATA 'Programs\ElektraWatt'
$Shortcut = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\ElektraWatt.lnk'
$EntryKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\ElektraWatt'
$Settings = Join-Path $env:APPDATA 'ElektraWatt'
$RuleGroup = 'ElektraWatt install test'
$Stage = Join-Path $PSScriptRoot '..\stage.py'
# D13's copies of the installed tree, each without one resource: `stage.py resources --work $RUNNER_TEMP\resources`
# in package.yml starts their elektrawatt.exe (stage.py's RESOURCES names the words).
$Copies = @(foreach ($word in @('sample', 'content', 'notices')) {
    Join-Path $env:RUNNER_TEMP "resources\without-$word\tree\elektrawatt.exe"
})

function Fail([string]$Message) {
    Write-Host "::error::install-test: $Message"
    exit 1
}

# IN-9: the rules (the setup, the two programs and D13's three copies), and the firewall on for every profile
# (outbound stays allowed for everything else).
function Add-FirewallBlock {
    $off = @(Get-NetFirewallProfile -PolicyStore ActiveStore | Where-Object { $_.Enabled -ne 'True' })
    if ($off.Count -gt 0) {
        $names = $off.Name -join ', '
        Write-Host "install-test: the firewall was off for $names; turned on for the test"
        Set-NetFirewallProfile -Profile $off.Name -Enabled True
    }
    $copy = 0
    foreach ($program in @($Setup, (Join-Path $Root 'elektrawatt.exe'), (Join-Path $Root 'ew.exe')) + $Copies) {
        $name = "$RuleGroup ($(Split-Path -Leaf $program))"
        if ($Copies -contains $program) { $copy++; $name = "$RuleGroup (D13 copy $copy, elektrawatt.exe)" }
        New-NetFirewallRule -DisplayName $name -Group $RuleGroup -Direction Outbound -Action Block -Program $program |
            Out-Null
    }
    Assert-FirewallBlock
}

function Assert-FirewallBlock {
    $off = @(Get-NetFirewallProfile -PolicyStore ActiveStore | Where-Object { $_.Enabled -ne 'True' })
    if ($off.Count -gt 0) {
        Fail "the firewall is off for $($off.Name -join ', '), so the block rules are not in force (IN-9)"
    }
    $rules = @(Get-NetFirewallRule -Group $RuleGroup -PolicyStore ActiveStore |
        Where-Object { $_.Enabled -eq 'True' -and $_.Action -eq 'Block' -and $_.Direction -eq 'Outbound' })
    $programs = @($rules | Get-NetFirewallApplicationFilter | ForEach-Object { $_.Program })
    if ($programs.Count -ne 3 + $Copies.Count) {
        $want = "the setup, the two programs and D13's $($Copies.Count) copies"
        Fail "the outbound block rules in force name $($programs.Count) programs, not $want"
    }
    Write-Host "install-test: outbound blocked for $($programs -join ', ')"
}

# IN-8: the key names under HKLM\SOFTWARE and the file names under the Program Files folders, each walked with an
# explicit stack. Not `Get-ChildItem -Recurse -Name`: for `-Name` the engine recurses itself, one set of frames per
# level, and asks the provider only whether a child is a container, so it can enter junctions and symbolic links; on
# the hosted runner it ended in "call depth overflow" after 24 minutes (package.yml run 4). A reparse point is listed
# but not entered. A key that refuses to open is listed without its subkeys, and the skipped keys are counted.
function Get-MachineListing {
    $names = [Collections.Generic.List[string]]::new()
    $skippedKeys = 0
    $hive = [Microsoft.Win32.RegistryKey]::OpenBaseKey('LocalMachine', 'Registry64')
    $keys = [Collections.Generic.Stack[string]]::new()
    $keys.Push('SOFTWARE')
    while ($keys.Count -gt 0) {
        $path = $keys.Pop()
        $key = $null
        try { $key = $hive.OpenSubKey($path) } catch { $key = $null }
        if ($null -eq $key) { $skippedKeys++; continue }
        try {
            foreach ($child in $key.GetSubKeyNames()) {
                $names.Add("HKLM\$path\$child")
                $keys.Push("$path\$child")
            }
        } catch { $skippedKeys++ } finally { $key.Dispose() }
    }
    $hive.Dispose()
    # Hidden and system entries count too, as `-Force` did; a folder that refuses to open reads as empty.
    $options = [IO.EnumerationOptions]::new()
    $options.AttributesToSkip = [IO.FileAttributes]0
    $options.IgnoreInaccessible = $true
    $skippedFolders = 0
    $folders = [Collections.Generic.Stack[IO.DirectoryInfo]]::new()
    foreach ($folder in @($env:ProgramFiles, ${env:ProgramFiles(x86)})) {
        $folders.Push([IO.DirectoryInfo]::new($folder))
    }
    while ($folders.Count -gt 0) {
        $folder = $folders.Pop()
        try {
            foreach ($entry in $folder.EnumerateFileSystemInfos('*', $options)) {
                $names.Add($entry.FullName)
                $link = $entry.Attributes.HasFlag([IO.FileAttributes]::ReparsePoint)
                if ($entry -is [IO.DirectoryInfo] -and -not $link) {
                    $folders.Push($entry)
                }
            }
        } catch { $skippedFolders++ }
    }
    Write-Host "install-test: the listing skipped $skippedKeys keys and $skippedFolders folders that refused to open"
    , $names.ToArray()
}

# IN-8: the subtrees that Windows' own diagnostics write while the test runs (ID-1283). The listing's changes in them
# are printed as ignored, not failed. A subtree joins only when a run shows the operating system writing it.
$IgnoredSubtrees = @(
    # Windows Error Reporting records why a process ended, under TermReason\<process ID> (package.yml runs 5 and 7).
    'HKLM\SOFTWARE\Microsoft\Windows\Windows Error Reporting\TermReason'
    # RADAR, the heap-leak diagnosis, records the applications it diagnosed: run 5's was the test's own pwsh.exe.
    'HKLM\SOFTWARE\Microsoft\RADAR\HeapLeakDetection\DiagnosedApplications'
    # WMI's tracing registers and drops its providers: runs 5 and 7 dropped wfascim, the CIM provider of the firewall
    # cmdlets.
    'HKLM\SOFTWARE\Microsoft\Wbem\Tracing\Providers'
)

# True, after printing it as ignored, for a change in one of the ignored subtrees (its prefix at a path-segment
# boundary, ignoring case) whose name holds none of the setup's names: ElektraWatt, the setup's file, the install
# folder.
function Test-Ignored([string]$Change, [string]$Name) {
    $ignoreCase = [StringComparison]::OrdinalIgnoreCase
    $subtree = @($IgnoredSubtrees.Where({ $Name.Equals($_, $ignoreCase) -or $Name.StartsWith("$_\", $ignoreCase) }))
    if ($subtree.Count -eq 0) { return $false }
    $marks = @('ElektraWatt', (Split-Path -Leaf $Setup), $Root).Where({ $_ })
    if (@($marks.Where({ $Name.IndexOf($_, $ignoreCase) -ge 0 })).Count -gt 0) { return $false }
    Write-Host "install-test: ignored $Change $Name (under $($subtree[0]), which Windows writes)"
    $true
}

function Assert-SameListing([string[]]$Before, [string[]]$After) {
    $old = [Collections.Generic.HashSet[string]]::new($Before, [StringComparer]::OrdinalIgnoreCase)
    $new = [Collections.Generic.HashSet[string]]::new($After, [StringComparer]::OrdinalIgnoreCase)
    $added = @($After.Where({ -not $old.Contains($_) -and -not (Test-Ignored 'added' $_) }))
    $removed = @($Before.Where({ -not $new.Contains($_) -and -not (Test-Ignored 'removed' $_) }))
    foreach ($line in $added) { Write-Host "install-test: added $line" }
    foreach ($line in $removed) { Write-Host "install-test: removed $line" }
    if ($added.Count + $removed.Count -gt 0) {
        $counts = "$($added.Count) names added, $($removed.Count) removed"
        Fail "the setup changed HKLM\SOFTWARE or a Program Files folder: $counts"
    }
    Write-Host "install-test: HKLM\SOFTWARE and the Program Files folders unchanged ($($Before.Count) names)"
}

function Get-Listing([string]$Folder) {
    $lines = @(python -I $Stage listing $Folder)
    if ($LASTEXITCODE -ne 0) { Fail "stage.py listing $Folder failed" }
    $lines
}

# D9: the setup refuses a folder that holds files and no uninstall.exe, whose own `docs` or `content` its uninstall
# would delete; `/S` exits non-zero there, and the folder keeps its file. Run before any installation.
function Assert-RefusesForeignFolder {
    $folder = Join-Path $env:RUNNER_TEMP 'foreign-folder'
    $keep = Join-Path $folder 'docs\keep.txt'
    $text = "the user's own file"
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $keep) | Out-Null
    [IO.File]::WriteAllText($keep, $text)
    $run = Start-Process -FilePath $Setup -ArgumentList '/S', "/D=$folder" -Wait -PassThru
    if ($run.ExitCode -eq 0) { Fail "the setup /S installed into $folder, which holds a file and no uninstall.exe" }
    $left = @(Get-ChildItem -LiteralPath $folder -Recurse -Force | ForEach-Object { $_.FullName })
    $want = @((Split-Path -Parent $keep), $keep)
    if (($left -join ', ') -ne ($want -join ', ') -or [IO.File]::ReadAllText($keep) -cne $text) {
        Fail "the refused setup changed $folder; it holds $($left -join ', ')"
    }
    foreach ($written in @($EntryKey, $Shortcut)) {
        if (Test-Path -LiteralPath $written) { Fail "the refused setup wrote $written" }
    }
    Write-Host "install-test: the setup /S refused $folder (exit $($run.ExitCode)); docs\keep.txt is the same"
}

function Install-Setup {
    $run = Start-Process -FilePath $Setup -ArgumentList '/S' -Wait -PassThru
    if ($run.ExitCode -ne 0) { Fail "the setup /S exited with $($run.ExitCode)" }
}

# IN-8: the folder, the shortcut and the entry of an installation.
function Assert-Installed {
    $installed = @(Get-Listing $Root)
    $uninstaller = @($installed | Where-Object { $_ -clike '*  uninstall.exe' })
    if ($uninstaller.Count -ne 1) { Fail "the installed folder $Root holds no uninstall.exe" }
    $rest = @($installed | Where-Object { $_ -cnotlike '*  uninstall.exe' })
    $diff = @(Compare-Object -CaseSensitive (Get-Listing $Tree) $rest)
    if ($diff.Count -gt 0) {
        $diff | ForEach-Object { Write-Host "install-test: $($_.SideIndicator) $($_.InputObject)" }
        Fail "the installed folder differs from the staged tree (<= staged only, => installed only)"
    }
    Write-Host "install-test: $Root is the staged tree ($($rest.Count) files) and uninstall.exe"
    if (-not (Test-Path -LiteralPath $Shortcut)) { Fail "no Start-menu shortcut $Shortcut" }
    $target = (New-Object -ComObject WScript.Shell).CreateShortcut($Shortcut).TargetPath
    if ($target -ne (Join-Path $Root 'elektrawatt.exe')) { Fail "the Start-menu shortcut opens $target" }
    if (-not (Test-Path -LiteralPath $EntryKey)) { Fail "no uninstall entry $EntryKey" }
    $entry = Get-ItemProperty -LiteralPath $EntryKey
    $uninstall = Join-Path $Root 'uninstall.exe'
    $want = [ordered]@{
        DisplayName          = 'ElektraWatt'
        DisplayVersion       = $Version
        Publisher            = 'The ElektraWatt contributors'
        InstallLocation      = $Root
        UninstallString      = "`"$uninstall`""
        QuietUninstallString = "`"$uninstall`" /S"
        NoModify             = '1'
        NoRepair             = '1'
    }
    foreach ($name in $want.Keys) {
        $value = $entry.PSObject.Properties[$name]
        if (-not $value) { Fail "the uninstall entry has no $name" }
        if ("$($value.Value)" -ne $want[$name]) {
            Fail "the uninstall entry's $name is '$($value.Value)', not '$($want[$name])'"
        }
    }
    $size = $entry.PSObject.Properties['EstimatedSize']
    if (-not $size -or [int64]$size.Value -le 0) { Fail 'the uninstall entry has no EstimatedSize' }
    Write-Host "install-test: the shortcut opens elektrawatt.exe; the uninstall entry is D9's, version $Version"
}

function Get-SettingsHashes {
    @(Get-ChildItem -LiteralPath $Settings -Recurse -File | Sort-Object FullName |
        ForEach-Object { "$((Get-FileHash -Algorithm SHA256 -LiteralPath $_.FullName).Hash)  $($_.FullName)" })
}

function Assert-SameSettings([string[]]$Before, [string]$When) {
    $diff = @(Compare-Object -CaseSensitive $Before (Get-SettingsHashes))
    if ($diff.Count -gt 0) {
        $diff | ForEach-Object { Write-Host "install-test: $($_.SideIndicator) $($_.InputObject)" }
        Fail "the user's settings folder $Settings changed $When"
    }
    Write-Host "install-test: the settings and keymap files are the same bytes $When"
}

switch ($Mode) {
    'install' {
        $text = [Text.Encoding]::ASCII.GetString([IO.File]::ReadAllBytes($Setup))
        $levels = @([regex]::Matches($text, 'requestedExecutionLevel\s+level="([A-Za-z]+)"') |
            ForEach-Object { $_.Groups[1].Value } | Sort-Object -Unique)
        if (($levels -join ', ') -cne 'asInvoker') {
            Fail "the setup's manifest asks for '$($levels -join ', ')', not asInvoker alone"
        }
        Write-Host 'install-test: the setup asks for asInvoker'
        Add-FirewallBlock
        Assert-RefusesForeignFolder
        $before = Get-MachineListing
        Install-Setup
        Assert-SameListing $before (Get-MachineListing)
        Assert-Installed
    }
    'deny' {
        $who = "$env:USERDOMAIN\$env:USERNAME"
        # Specific rights only, the delete right as `DE`: with `D` in the list icacls writes its simple rights `(W,D)`,
        # and a program in the folder can no longer be started (tried on a staged tree).
        icacls $Dir /deny "${who}:(OI)(CI)(WD,AD,WEA,WA,DE,DC)" | Out-Host
        if ($LASTEXITCODE -ne 0) { Fail "icacls could not deny $who writing in $Dir" }
        $probe = Join-Path $Dir 'write-probe.txt'
        $written = $true
        try { [IO.File]::WriteAllText($probe, 'x') } catch { $written = $false }
        if ($written) { Fail "$who can still write in $Dir after the denial" }
        Write-Host "install-test: $who may not write or delete in $Dir"
    }
    'allow' {
        $who = "$env:USERDOMAIN\$env:USERNAME"
        icacls $Dir /remove:d $who | Out-Host
        if ($LASTEXITCODE -ne 0) { Fail "icacls could not remove the denial for $who in $Dir" }
    }
    'again-and-uninstall' {
        New-Item -ItemType Directory -Force -Path $Settings | Out-Null
        $lf = "`n"
        [IO.File]::WriteAllText((Join-Path $Settings 'settings.toml'),
            "version = 1${lf}first_run_done = true${lf}language = `"nl`"${lf}")
        [IO.File]::WriteAllText((Join-Path $Settings 'keymap.toml'),
            "[meta]${lf}name = `"IN-13`"${lf}version = 1${lf}${lf}[[bind]]${lf}keys = `"Ctrl+Shift+F12`"${lf}" +
            "action = `"help.about`"${lf}context = `"global`"${lf}")
        $before = Get-SettingsHashes
        Install-Setup
        Assert-Installed
        Assert-SameSettings $before 'after the setup ran over the installation'
        $quiet = (Get-ItemProperty -LiteralPath $EntryKey).QuietUninstallString
        if ($quiet -notmatch '^"([^"]+)"\s+(.+)$') {
            Fail "the QuietUninstallString '$quiet' is not a quoted program and its arguments"
        }
        $run = Start-Process -FilePath $Matches[1] -ArgumentList $Matches[2] -Wait -PassThru
        if ($run.ExitCode -ne 0) { Fail "the uninstaller exited with $($run.ExitCode)" }
        # NSIS's uninstaller copies itself to %TEMP% and returns; the copy removes the installation.
        $deadline = (Get-Date).AddMinutes(2)
        # `@()` around the call: a script block that outputs nothing gives $null, which has no Count under
        # Set-StrictMode (package.yml run 6), and one path left would be a bare string.
        $left = { @($Root, $EntryKey, $Shortcut).Where({ Test-Path -LiteralPath $_ }) }
        while (@(& $left).Count -gt 0) {
            if ((Get-Date) -gt $deadline) {
                Fail "two minutes after the uninstall, still there: $((& $left) -join ', ')"
            }
            Start-Sleep -Seconds 1
        }
        Write-Host 'install-test: the uninstall removed the folder, the shortcut and the entry'
        Assert-SameSettings $before 'after the uninstall'
        Assert-FirewallBlock
    }
}
