# Install ElektraWatt

This guide installs ElektraWatt on Windows with its setup, or the command-line program `ew` alone from its archive on
Windows or Linux, and then upgrades or removes it. Each download carries the version in its name, shown here as
`<version>`.

## What you need

- For the setup: Windows 10 or 11 on an x86-64 PC. No administrator rights and no network connection are needed.
- For a CLI archive: Windows or Linux (glibc 2.35 or newer, as on Ubuntu 22.04 and Debian 12) on x86-64.

## Install with the setup on Windows

1. Run `ElektraWatt-<version>-x64-user-setup.exe`.
2. The setup is not signed yet, so Windows may stop it with **Windows protected your PC**. Choose **More info**, check
   that the file is the one you downloaded, then choose **Run anyway**.
3. Keep the folder the setup proposes, `%LOCALAPPDATA%\Programs\ElektraWatt`, or choose another one of your own, then
   choose **Install**.
4. On the last page, leave **Run ElektraWatt** ticked to start it, and choose **Finish**.

The setup installs for you alone: the program, the command-line program `ew.exe`, the starter content, the sample
project and this guide, all in the one folder, with a shortcut **ElektraWatt** in the Start menu. It does not change
`PATH`.

To install without questions, for example from a script, run the setup with `/S`. Add `/D=<folder>` as the last
argument, without quotes even when the folder has spaces, to install into another folder:

```text
ElektraWatt-<version>-x64-user-setup.exe /S /D=C:\Tools\ElektraWatt
```

## Use the CLI archive

The archives hold `ew` with the starter content and the sample project, for a machine without the window or for a
CI job: `ew-<version>-x86_64-pc-windows-msvc.zip` and `ew-<version>-x86_64-unknown-linux-gnu.tar.gz`.

1. Unpack the archive. It holds one folder, `ew-<version>-<target>`.
2. `ew` finds the starter content through `--content` or the environment variable `EW_CONTENT_DIR`, not beside itself.
   Set `EW_CONTENT_DIR` to the folder's `content` folder:
   - Linux: `export EW_CONTENT_DIR=<folder>/content`
   - Windows (PowerShell): `$env:EW_CONTENT_DIR = "<folder>\content"`
3. Check it: `ew --version` prints the version, and `ew export pdf <folder>/samples/J-E-2416.ew --out sample.pdf`
   writes the sample project's drawings as a PDF.

The `ew.exe` that the setup installs finds the starter content the same way: set `EW_CONTENT_DIR` to the `content`
folder of the installation.

## Upgrade

Run the setup of the newer version as above. It finds the installed version, removes it first with its own
uninstaller, then installs, so no file of the older version remains. Your settings and keymaps stay as they are.

To upgrade `ew` from an archive, unpack the newer archive and point `EW_CONTENT_DIR` at its `content` folder.

## Uninstall

1. Open **Settings**, then **Apps** and **Installed apps** (on Windows 10: **Apps & features**).
2. Find **ElektraWatt** and choose **Uninstall**.

To uninstall without questions, run `uninstall.exe /S` from the installation folder. The uninstall removes the
installation folder, the Start-menu shortcut and the entry in the list of apps. It leaves your projects, your settings
and keymaps (in `%APPDATA%\ElektraWatt`), and your libraries and the program's cache (in `%LOCALAPPDATA%\ElektraWatt`):
delete those folders yourself when you no longer want them.

A CLI archive installs nothing: delete its folder.
