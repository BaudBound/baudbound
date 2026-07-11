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

An AppImage is a portable executable. The steps below keep it in your home directory and create a `baudbound` command for your user account. They do not install a system package and do not require `sudo`.

1. Download the `.AppImage` from the newest GitHub release.
2. Open a terminal in the directory containing the download. Most browsers save it in `~/Downloads`.
3. Confirm that only the new release matches the filename pattern:

```text
cd ~/Downloads
ls -1 BaudBound_*.AppImage
```

If more than one file is printed, move the older AppImages elsewhere or use the exact new filename in the following `cp` command.

4. Create the application and command directories, copy the AppImage, make it executable, and create the `baudbound` command:

```text
mkdir -p "$HOME/.local/opt/baudbound"
mkdir -p "$HOME/.local/bin"
cp BaudBound_*.AppImage "$HOME/.local/opt/baudbound/BaudBound.AppImage"
chmod 0755 "$HOME/.local/opt/baudbound/BaudBound.AppImage"
ln -sfn "$HOME/.local/opt/baudbound/BaudBound.AppImage" "$HOME/.local/bin/baudbound"
```

The symbolic link gives the stable command `baudbound` while keeping the real AppImage at a stable, user-owned path. When an AppImage is launched through a symbolic link, its `APPIMAGE` environment variable resolves to the real file behind the link. Tauri's updater uses that resolved path, so it can replace `~/.local/opt/baudbound/BaudBound.AppImage` while the `baudbound` link remains unchanged. See the AppImage documentation for the [`APPIMAGE` and `ARGV0` behavior](https://docs.appimage.org/packaging-guide/environment-variables.html).

5. Check whether the shell can find it:

```text
command -v baudbound
```

The command should print a path ending in `.local/bin/baudbound`. If it prints nothing, add the following line to `~/.profile`, then sign out and back in:

```text
export PATH="$HOME/.local/bin:$PATH"
```

6. Verify the version and open the application:

```text
baudbound --version
baudbound
```

The final command should open the Dashboard.

If the AppImage reports a FUSE error, install the FUSE 2 compatibility package for the distribution. The [Linux Background Service](../self-hosting/linux-background-service.md) guide lists the correct package for Debian, Ubuntu, Fedora, and Arch.

BaudBound does not currently publish `.deb` or `.rpm` packages. Compatibility depends on the host architecture, graphics session, and system libraries, so verify the AppImage on the intended machine before relying on it.

AppImages are portable executables and are not installed through the system package database. For a persistent headless installation with distro-specific prerequisites, follow [Linux Background Service](../self-hosting/linux-background-service.md).
{.is-info}

## First launch

The first launch creates the runner home and a default `config.toml` automatically. You do not need to run `config init` for a normal desktop installation.

Open **Doctor** in the desktop navigation. A successful check confirms that the configuration and runner storage are accessible and shows which native actions the current machine supports.

These commands print the installed version, run diagnostics, and show the active configuration path:

```text
baudbound --version
baudbound doctor
baudbound config path
```

## Updates

Desktop builds check for a signed update at startup. When a newer version is available, the update dialog shows its release information and download progress. After verification, choose Restart to install and launch it.

On Linux, automatic updates require the real AppImage to remain writable by the current user. The per-user installation above satisfies that requirement. Do not change its owner to `root` or point the desktop `baudbound` link at the root-owned headless service copy under `/opt`.

For a manual Linux update, exit BaudBound completely and open a terminal in the directory containing the new AppImage. Copy it to a temporary destination, make it executable, and then replace the stable file:

```text
cp BaudBound_*.AppImage "$HOME/.local/opt/baudbound/BaudBound.AppImage.new"
chmod 0755 "$HOME/.local/opt/baudbound/BaudBound.AppImage.new"
mv "$HOME/.local/opt/baudbound/BaudBound.AppImage.new" "$HOME/.local/opt/baudbound/BaudBound.AppImage"
baudbound --version
```

The `baudbound` symlink does not change. Headless operators should replace `/opt/baudbound/BaudBound.AppImage` using the procedure in [Linux Background Service](../self-hosting/linux-background-service.md). Never bypass signature verification.

Continue with [Script Management](script-management.md).
