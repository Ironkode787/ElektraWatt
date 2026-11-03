# Install ElektraWatt

This guide installs ElektraWatt on Windows with its setup or on Linux as an AppImage, or the command-line program `ew`
alone from its archive on Windows or Linux, and then upgrades or removes it. Each download carries the version in its
name, shown here as `<version>`.

## What you need

- For the setup: Windows 10 or 11 on an x86-64 PC. No administrator rights and no network connection are needed.
- For the AppImage: Linux with glibc 2.35 or newer (as on Ubuntu 22.04 and Debian 12) on x86-64. No administrator
  rights, no network connection and no other package are needed.
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

## Run the AppImage on Linux

The AppImage is one file, `ElektraWatt-<version>-x86_64.AppImage`, that holds the program, the command-line program
`ew`, the starter content, the sample project and this guide. It installs nothing: you run the file itself.

1. Make it executable, then start it:

   ```text
   chmod +x ElektraWatt-<version>-x86_64.AppImage
   ./ElektraWatt-<version>-x86_64.AppImage
   ```

2. The AppImage opens itself through FUSE, which most desktops have. Where FUSE is missing (in a container, or on a
   system without the `fuse` kernel module), it stops with a message about FUSE. Run it with
   `--appimage-extract-and-run` as the first argument instead: it unpacks itself into a temporary folder for that run.

   ```text
   ./ElektraWatt-<version>-x86_64.AppImage --appimage-extract-and-run
   ```

3. To run `ew`, give `ew` as the first argument, then its own arguments (after `--appimage-extract-and-run` where FUSE
   is missing). Like the `ew` of a CLI archive, it reads the starter content only through `--content` or
   `EW_CONTENT_DIR` (see below):

   ```text
   ./ElektraWatt-<version>-x86_64.AppImage ew --version
   ./ElektraWatt-<version>-x86_64.AppImage ew export pdf my-project.ew --out drawings.pdf
   ```

Keep the file where you like, for example in `~/Applications`. The AppImage is not signed yet: check that the file is
the one you downloaded against the checksums published with its release.

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

To upgrade the AppImage, download the newer file, make it executable and delete the older one. Your settings and
keymaps stay as they are.

To upgrade `ew` from an archive, unpack the newer archive and point `EW_CONTENT_DIR` at its `content` folder.

## Uninstall

1. Open **Settings**, then **Apps** and **Installed apps** (on Windows 10: **Apps & features**).
2. Find **ElektraWatt** and choose **Uninstall**.

To uninstall without questions, run `uninstall.exe /S` from the installation folder. The uninstall removes the
installation folder, the Start-menu shortcut and the entry in the list of apps. It leaves your projects, your settings
and keymaps (in `%APPDATA%\ElektraWatt`), and your libraries and the program's cache (in `%LOCALAPPDATA%\ElektraWatt`):
delete those folders yourself when you no longer want them.

The AppImage installs nothing: delete its file. It leaves your projects, your settings and keymaps (in
`~/.config/elektrawatt`), and your libraries and the program's cache (in `~/.local/share/elektrawatt` and
`~/.cache/elektrawatt`): delete those folders yourself when you no longer want them.

A CLI archive installs nothing: delete its folder.
