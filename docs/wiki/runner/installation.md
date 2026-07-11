---
title: Installation and Updates
description: Install and update BaudBound on supported Windows and Linux systems.
tags: [runner, installation]
---
# Installation and Updates

## Supported systems

BaudBound releases currently target 64-bit Windows and 64-bit Linux. Windows is distributed as an installer. Linux is distributed as an AppImage built on Ubuntu 22.04.

Download release files only from the [BaudBound GitHub Releases page](https://github.com/NATroutter/BaudBound/releases). Open the newest non-draft release and choose the file for the operating system. You do not need Rust, Node.js, or the source repository to run a published release.

## Platform installation {.tabset}

### Windows

1. Download the `.exe` setup file from the newest GitHub release.
2. Open the downloaded file and complete the installer.
3. Start **BaudBound** from the Start menu.
4. Confirm that the Dashboard opens without an error banner.

The desktop interface uses Microsoft Edge WebView2. It is already installed on current Windows systems. If the application reports that WebView2 is missing, install the current WebView2 Runtime from Microsoft and launch BaudBound again.

The installer provides the desktop application. It does not guarantee that `baudbound` is added to every terminal's `PATH`. Use the Start menu for normal desktop use. CLI users can run the installed executable directly or add its installation directory to `PATH` themselves.

### Linux

An AppImage is a portable executable: downloading it does not install a system package or create a global `baudbound` command.

1. Download the `.AppImage` from the newest GitHub release.
2. Open a terminal in the directory containing the download. Most browsers save it in `~/Downloads`.
3. Make the file executable and start it:

```text
cd ~/Downloads
chmod +x BaudBound_*.AppImage
./BaudBound_*.AppImage
```

The final command opens the desktop application. Keep using `./BaudBound_*.AppImage` while the file remains in that directory. Rename or move the file if a stable path is required.

If the AppImage reports a FUSE error, install the FUSE 2 compatibility package for the distribution. The [Linux Background Service](../self-hosting/linux-background-service.md) guide lists the correct package for Debian, Ubuntu, Fedora, and Arch.

BaudBound does not currently publish `.deb` or `.rpm` packages. Compatibility depends on the host architecture, graphics session, and system libraries, so verify the AppImage on the intended machine before relying on it.

AppImages are portable executables and are not installed through the system package database. For a persistent headless installation with distro-specific prerequisites, follow [Linux Background Service](../self-hosting/linux-background-service.md).
{.is-info}

## First launch

The first launch creates the runner home and a default `config.toml` automatically. You do not need to run `config init` for a normal desktop installation.

Open **Doctor** in the desktop navigation. A successful check confirms that the configuration and runner storage are accessible and shows which native actions the current machine supports.

When the `baudbound` executable is available from the terminal, these commands print the installed version, run diagnostics, and show the active configuration path:

```text
baudbound --version
baudbound doctor
baudbound config path
```

Linux AppImage users who did not create a global command must replace `baudbound` with the AppImage path, for example `./BaudBound_2.0.0_amd64.AppImage doctor`.

## Updates

Desktop builds check for a signed update at startup. When a newer version is available, the update dialog shows its release information and download progress. After verification, choose Restart to install and launch it.

Headless operators and Linux users whose installation cannot be replaced automatically should download the new artifact from the project's GitHub releases. Never bypass signature verification.

Continue with [Script Management](script-management.md).
