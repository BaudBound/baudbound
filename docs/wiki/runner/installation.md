---
title: Installation and Updates
description: Install and update BaudBound on supported Windows and Linux systems.
tags: [runner, installation]
---
# Installation and Updates

## Supported systems

BaudBound releases currently target 64-bit Windows and 64-bit Linux. Windows is distributed as an installer. Linux is distributed as an AppImage built on Ubuntu 22.04.

Download release files only from the [BaudBound GitHub Releases page](https://github.com/NATroutter/BaudBound/releases). Open the latest published release and choose the file for the operating system. You do not need Rust, Node.js, or the source repository to run a published release.

## Platform installation {.tabset}

### Windows

1. Download the `.exe` setup file from the latest published release on the [BaudBound GitHub Releases page](https://github.com/NATroutter/BaudBound/releases).
2. Open the downloaded file and complete the installer.
3. Start **BaudBound** from the Start menu.
4. Confirm that the Dashboard opens without an error banner.

The desktop interface uses Microsoft Edge WebView2. It is already installed on current Windows systems. If the application reports that WebView2 is missing, install the current WebView2 Runtime from Microsoft and launch BaudBound again.

The installer provides the desktop application. It does not guarantee that `baudbound` is added to every terminal's `PATH`. Use the Start menu for normal desktop use. CLI users can run the installed executable directly or add its installation directory to `PATH` themselves.

### Linux

An AppImage is a portable executable. The steps below keep it in your home directory and create a `baudbound` command for your user account. They do not install a system package and do not require `sudo`.

1. Download the `.AppImage` from the latest published release on the [BaudBound GitHub Releases page](https://github.com/NATroutter/BaudBound/releases).
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

If the AppImage reports a FUSE error, use the matching distribution tab in [Linux FUSE packages](#linux-fuse-packages). If `baudbound --version` works, no additional FUSE package is needed.

BaudBound does not currently publish `.deb` or `.rpm` packages. Compatibility depends on the host architecture, graphics session, and system libraries, so verify the AppImage on the intended machine before relying on it.

AppImages are portable executables and are not installed through the system package database. To run BaudBound continuously without the desktop application, follow [Linux Background Service](linux-background-service.md).
{.is-info}

## Linux FUSE packages {.tabset}

Use this section only when the AppImage reports a FUSE error.

### Debian and Ubuntu

Update package metadata, then check which FUSE 2 runtime is available:

```text
sudo apt update
apt-cache policy libfuse2t64 libfuse2
```

Install the package whose `Candidate` line shows a version instead of `(none)`. Ubuntu 24.04 and newer normally use:

```text
sudo apt install -y libfuse2t64
```

Debian and older Ubuntu releases normally use:

```text
sudo apt install -y libfuse2
```

Install only one. If neither package has a candidate, confirm that the standard distribution repositories are enabled.

### Fedora

```text
sudo dnf install -y fuse-libs
```

### Arch Linux

Perform a full system upgrade while installing FUSE 2 because Arch does not support partial upgrades:

```text
sudo pacman -Syu --needed fuse2
```

### Gentoo

```text
sudo emerge --ask sys-fs/fuse:0
```

### Void Linux

The AppImage supports Void's 64-bit glibc edition, not its musl edition:

```text
sudo xbps-install -S fuse
```

These package names follow the AppImage project's [FUSE troubleshooting guidance](https://docs.appimage.org/user-guide/troubleshooting/fuse.html) and the distributions' package documentation.

## First launch

The first launch creates the runner home and a default `config.toml` automatically. You do not need to run `baudbound config init` for a normal desktop installation.

Open **Doctor** in the desktop navigation. A successful check confirms that the configuration and runner storage are accessible and shows which native actions the current machine supports.

These commands print the installed version, run diagnostics, and show the active configuration path:

```text
baudbound --version
baudbound doctor
baudbound config path
```

## Updates

### Automatic update

This is the recommended update method for desktop users. No terminal commands are required.

1. Start BaudBound normally.
2. When an update is available, review the version and release notes in the update dialog.
3. Choose **Download update** and wait for the progress bar to finish.
4. Choose **Restart and install**.
5. After BaudBound opens again, confirm the new version in the application.

On Linux, automatic updates require the real AppImage to remain writable by the current user. The per-user installation above satisfies that requirement. Do not change its owner to `root`. A headless service must be stopped before replacing the AppImage manually.

### Manual Linux fallback

Use these steps only when the update dialog cannot complete the update.

1. Stop BaudBound according to how it is currently running:

- **Desktop application:** Open **Service** and stop the desktop background runner. Then open the tray menu and choose **Quit**. Closing only the window is not enough because BaudBound normally remains in the tray.
- **One-time CLI command:** Wait for the command to finish before updating.
- **`baudbound serve` in a terminal:** Press `Ctrl+C` in that terminal and wait for the process to exit.
- **systemd service:** Run `sudo systemctl stop baudbound` and confirm `sudo systemctl status baudbound` reports it as inactive.
- **OpenRC service:** Run `sudo rc-service baudbound stop` and confirm `sudo rc-service baudbound status` reports it as stopped.
- **runit service:** Run `sudo sv down baudbound` and confirm `sudo sv status baudbound` reports it as down.

2. Open a terminal and confirm that no BaudBound process remains:

```text
pgrep -af 'BaudBound|baudbound'
```

No output means BaudBound has stopped. If a process is listed, return to the application or service manager and stop it normally before continuing.

3. Put the new AppImage at `~/Downloads/BaudBound.AppImage` using one of these download options.

**Option A: Web browser**

Open the latest published release on the [BaudBound GitHub Releases page](https://github.com/NATroutter/BaudBound/releases). Download its `.AppImage` file, move it to `~/Downloads` if necessary, and rename the downloaded file to `BaudBound.AppImage`.

**Option B: Terminal with curl**

This option requires `curl` and `jq`. Confirm that both commands are installed:

```text
command -v curl
command -v jq
```

Both commands must print a path. If either prints nothing, install that command with the distribution's package manager or use the web browser option.

Fetch the AppImage URL from GitHub's latest published release and store it in `APPIMAGE_DOWNLOAD_URL`:

```text
APPIMAGE_DOWNLOAD_URL="$(curl --fail --silent --show-error "https://api.github.com/repos/NATroutter/BaudBound/releases/latest" | jq -er '.assets | map(select(.name | endswith(".AppImage"))) | if length == 1 then .[0].browser_download_url else error("expected exactly one AppImage asset") end')"
printf '%s\n' "$APPIMAGE_DOWNLOAD_URL"
```

The printed URL must begin with `https://github.com/NATroutter/BaudBound/releases/download/` and end with `.AppImage`. Download that asset:

```text
mkdir -p "$HOME/Downloads"
curl --fail --location --output "$HOME/Downloads/BaudBound.AppImage" "$APPIMAGE_DOWNLOAD_URL"
```

If the API is unavailable, rate-limited, or does not contain exactly one AppImage, the URL command fails instead of choosing an arbitrary asset. Use the web browser option in that case. A failed download leaves the current installation unchanged.

After completing either option, continue with step 4. Both options produce the same `~/Downloads/BaudBound.AppImage` file used by the remaining update steps.

4. Confirm that the downloaded file exists:

```text
ls -lh "$HOME/Downloads/BaudBound.AppImage"
```

5. Copy it over the currently installed AppImage and restore executable permissions:

```text
cp "$HOME/Downloads/BaudBound.AppImage" "$HOME/.local/opt/baudbound/BaudBound.AppImage"
chmod 0755 "$HOME/.local/opt/baudbound/BaudBound.AppImage"
```

6. Verify the installed version:

```text
baudbound --version
```

The command must print the intended new version. The `baudbound` symlink does not change because it still points to the stable installed path.

7. Start BaudBound the same way it was running before the update:

- **Desktop application:** Run `baudbound`.
- **Foreground service:** Run `baudbound serve`.
- **systemd service:** Run `sudo systemctl start baudbound` and check `sudo systemctl status baudbound`.
- **OpenRC service:** Run `sudo rc-service baudbound start` and check `sudo rc-service baudbound status`.
- **runit service:** Run `sudo sv up baudbound` and check `sudo sv status baudbound`.

Manual replacement does not use Tauri's automatic signature-verification flow, so download only from the official GitHub Releases page. See [Linux Background Service](linux-background-service.md) for service setup and control commands.

Continue with [Script Management](script-management.md).
