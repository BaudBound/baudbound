---
title: Installation and Updates
description: Install and update BaudBound on supported Windows and Linux systems.
tags: [runner, installation]
---
# Installation and Updates

## Supported systems

BaudBound releases currently target 64-bit Windows and 64-bit x86 Linux. Windows uses an installer. Debian and Ubuntu use a `.deb` package. Fedora uses an `.rpm` package. A portable AppImage is also published for manual use on other compatible Linux systems.

Download release files only from the [BaudBound GitHub Releases page](https://github.com/NATroutter/BaudBound/releases). Open the latest published release and choose the file for the operating system. You do not need Rust, Node.js, or the source repository to run a published release.

## Linux encrypted secret storage

BaudBound encrypts script secret values before storing them. On Linux, the desktop application requires a service that implements the standard Secret Service interface.

Most GNOME desktop installations use GNOME Keyring. Other desktop environments may provide a compatible service through KWallet or another credential manager.

The BaudBound installer does not install a credential manager automatically. The correct provider depends on the Linux desktop environment, and installing multiple providers can create conflicts.

BaudBound connects to Secret Service in the background. The desktop window and scripts that do not use secrets remain available while the connection is being established. Scripts that require secrets cannot use those values until the connection succeeds.

The Security page shows whether the credential vault is connecting, available, or unavailable. When a connection fails, the page displays the error and provides a Retry button. BaudBound never replaces the credential vault with an unencrypted key file.

### Install a Secret Service provider {.tabset}

#### Debian and Ubuntu

Install GNOME Keyring, its login integration, and the testing tool:

```bash
sudo apt install gnome-keyring libpam-gnome-keyring libsecret-tools
```

Restart the machine or sign out of the graphical desktop and sign back in. This allows the login session to start and unlock the credential vault.

#### Fedora

```bash
sudo dnf install gnome-keyring libsecret
```

Restart the machine or sign out of the graphical desktop and sign back in.

#### Arch Linux

```bash
sudo pacman -S gnome-keyring libsecret
```

Restart the machine or sign out of the graphical desktop and sign back in.

### Verify the service

Run this command from the same desktop session that will run BaudBound:

```bash
busctl --user list | grep org.freedesktop.secrets
```

A working service displays a process ID and the credential manager process. A result containing only `activatable` means the service is installed but is not currently running.

You can test encrypted storage with:

```bash
printf 'baudbound-test' | secret-tool store --label='BaudBound test' application baudbound-test
```

Remove the test value afterward:

```bash
secret-tool clear application baudbound-test
```

Remote desktop and VNC sessions may have a separate desktop session. The Secret Service must be running inside the same user session as BaudBound. A credential vault that works during a local login might not automatically start inside a VNC session.

Headless services do not use the desktop credential vault. They use the protected `BAUDBOUND_SECRET_KEY` environment value described in [Secrets](secrets.md).

## Automatic installation and updates

These commands install BaudBound when it is missing and update it when an older version is installed. The downloaded release file is checked against the SHA-256 digest published by GitHub before it is opened or installed.

This is the recommended method for most users. Choose the operating system below and run its single command.
{.is-success}

A SHA-256 digest is a long value calculated from a file. It works like a digital fingerprint. The installation script calculates the fingerprint of the downloaded file and compares it with the fingerprint published for that release. The script stops when they do not match.

### Choose your operating system {.tabset}

#### Windows

Open PowerShell and run:

```powershell
irm https://get.baudbound.app/windows | iex
```

The script downloads the current Windows installer from the official BaudBound GitHub release and verifies it before opening the normal setup window. Quit BaudBound before running the command when you are updating an existing installation.

#### Linux

Open a terminal and run:

```bash
curl -fsSL https://get.baudbound.app/linux | sh
```

The command above uses `curl` to download the installer, so `curl` must already be available. If the terminal says that `curl` was not found, install it with the command for your distribution and then run the BaudBound installation command again.

On Debian or Ubuntu, install `curl` with:

```bash
sudo apt install curl
```

On Fedora, install `curl` with:

```bash
sudo dnf install curl
```

After `curl` starts the installer, the script confirms that the machine uses 64-bit x86 Linux and reads the distribution information from `/etc/os-release`. Debian and Ubuntu receive the `.deb` package through APT. Fedora receives the `.rpm` package through DNF.

The script checks every command it needs before downloading BaudBound. If a command is missing, the script stops and names it. It also stops without downloading anything when the distribution or architecture is unsupported.

The selected package is downloaded from the official GitHub Release. Its SHA-256 digest, package name, version, and architecture are checked before APT or DNF starts. The package manager asks for administrator approval and installs the command, application-menu launcher, and icons.

The automatic installer does not install the AppImage. On an unsupported distribution, follow the portable AppImage instructions below.

The installer refuses to replace a newer installed version with an older GitHub Release. It stops before APT or DNF starts and leaves the current installation unchanged.

If the old per-user AppImage installation is detected, the installer stops instead of creating duplicate commands or application-menu entries. Follow [Switching from AppImage to a native package](#switching-from-appimage-to-a-native-package) first.

## Manual installation

Use this section when you do not want to run the hosted installation script. These steps download and install the same official GitHub release files manually.

Manual installation takes more steps and requires you to replace files yourself when updating. Use it only when you prefer not to run the automatic installation command.
{.is-info}

### Choose your operating system {.tabset}

#### Windows

1. Download the `.exe` setup file from the latest published release on the [BaudBound GitHub Releases page](https://github.com/NATroutter/BaudBound/releases).
2. Open the downloaded file and complete the installer.
3. Start **BaudBound** from the Start menu.
4. Confirm that the Dashboard opens without an error banner.

The desktop interface uses Microsoft Edge WebView2. It is already installed on current Windows systems. If the application reports that WebView2 is missing, install the current WebView2 Runtime from Microsoft and launch BaudBound again.

The installer provides the desktop application. It does not guarantee that `baudbound` is added to every terminal's `PATH`. Use the Start menu for normal desktop use. CLI users can run the installed executable directly or add its installation directory to `PATH` themselves.

#### Linux

Use the native package for Debian, Ubuntu, or Fedora. It installs the desktop launcher and the `baudbound` command through the operating system package manager.

##### Debian and Ubuntu package

1. Open the [latest BaudBound GitHub Release](https://github.com/NATroutter/BaudBound/releases/latest) and download the file ending in `_amd64.deb`.
2. Open a terminal and move to the directory containing the download:

```bash
cd "$HOME/Downloads"
```

3. Confirm that the directory contains only the BaudBound package you intend to install:

```bash
ls -1 Baudbound_*_amd64.deb
```

4. Ask APT to verify dependencies and install the local package:

```bash
sudo apt install ./Baudbound_*_amd64.deb
```

APT installs a newer package over an older BaudBound version. You do not need to remove the old version first.

##### Fedora package

1. Open the [latest BaudBound GitHub Release](https://github.com/NATroutter/BaudBound/releases/latest) and download the file ending in `.x86_64.rpm`.
2. Open a terminal and move to the directory containing the download:

```bash
cd "$HOME/Downloads"
```

3. Confirm that the directory contains only the BaudBound package you intend to install:

```bash
ls -1 Baudbound-*.x86_64.rpm
```

4. Ask DNF to verify dependencies and install the local package:

```bash
sudo dnf install ./Baudbound-*.x86_64.rpm
```

DNF installs a newer package over an older BaudBound version. You do not need to remove the old version first.

##### Portable AppImage

Use the AppImage when the automatic installer does not support the distribution or when you specifically want a portable per-user installation. The AppImage is available only from the [GitHub Releases page](https://github.com/NATroutter/BaudBound/releases/latest). It is not offered by `get.baudbound.app`.

The AppImage contains both the BaudBound desktop application and its CLI. Starting it without a command opens the desktop application. Adding a command such as `--version` or `doctor` uses the CLI.

The application menu launcher does not need special arguments. `baudbound --gui` is available when you need to request the graphical application explicitly from a terminal. The `--autostart` flag is reserved for the login entry created by BaudBound and should not be added to a normal launcher.

The steps below install BaudBound only for your user account. They do not install a system package and do not require `sudo`.

##### Install the AppImage

1. Download the `.AppImage` from the latest published release on the [BaudBound GitHub Releases page](https://github.com/NATroutter/BaudBound/releases).

2. Open a terminal. Move into the Downloads directory because this is where most browsers save the file:

```bash
cd "$HOME/Downloads"
```

3. List the downloaded BaudBound AppImage. This check helps you avoid copying an older release by mistake:

```bash
ls -1 Baudbound_*.AppImage
```

Exactly one filename should be printed. If several files are printed, move the older files elsewhere or replace `Baudbound_*.AppImage` in the later copy command with the exact filename you want to install.

4. Create a permanent directory for the application:

```bash
mkdir -p "$HOME/.local/opt/baudbound"
```

5. Copy the downloaded AppImage into that directory. The destination has a stable name so updates can replace the same file later:

```bash
cp Baudbound_*.AppImage "$HOME/.local/opt/baudbound/BaudBound.AppImage"
```

6. Give the AppImage permission to run:

```bash
chmod 0755 "$HOME/.local/opt/baudbound/BaudBound.AppImage"
```

##### Create the terminal command

1. Create the standard per-user command directory:

```bash
mkdir -p "$HOME/.local/bin"
```

2. Create a symbolic link named `baudbound`. This lets you type `baudbound` instead of the full AppImage path:

```bash
ln -sfn "$HOME/.local/opt/baudbound/BaudBound.AppImage" "$HOME/.local/bin/baudbound"
```

The link remains unchanged when the AppImage is updated. Tauri's updater resolves the link to the real file and replaces `~/.local/opt/baudbound/BaudBound.AppImage`. See the AppImage documentation for the [`APPIMAGE` and `ARGV0` behavior](https://docs.appimage.org/packaging-guide/environment-variables.html).

3. Ask the shell where it finds the `baudbound` command:

```bash
command -v baudbound
```

The result should end with `.local/bin/baudbound`. If nothing is printed, add the following line to `~/.profile`. This tells new terminal sessions to search your per-user command directory:

```bash
export PATH="$HOME/.local/bin:$PATH"
```

Sign out and back in after changing `~/.profile`.

4. Print the installed version. This confirms that the AppImage can start:

```bash
baudbound --version
```

##### Create the desktop launcher

The following steps add BaudBound to the desktop application menu with its own icon. The launcher opens the graphical application without opening a terminal window.

1. Create the directory where applications installed for your user keep their menu launchers:

```bash
mkdir -p "$HOME/.local/share/applications"
```

2. Create the directory where the BaudBound icon will be stored:

```bash
mkdir -p "$HOME/.local/share/icons/hicolor/128x128/apps"
```

3. Create a temporary directory for extracting the icon bundled inside the AppImage:

```bash
desktop_assets_dir="$(mktemp -d)"
```

4. Move into that temporary directory. This keeps the extracted application files out of your Downloads directory:

```bash
cd "$desktop_assets_dir"
```

5. Extract the files bundled in the AppImage. This does not start or reinstall BaudBound:

```bash
"$HOME/.local/opt/baudbound/BaudBound.AppImage" --appimage-extract >/dev/null
```

6. Copy the bundled application icon into your local icon directory:

```bash
cp -L "$desktop_assets_dir/squashfs-root/.DirIcon" "$HOME/.local/share/icons/hicolor/128x128/apps/baudbound.png"
```

7. Return to your Downloads directory before removing the temporary extraction directory:

```bash
cd "$HOME/Downloads"
```

8. Remove only the temporary files that were extracted in step 5:

```bash
rm -rf "$desktop_assets_dir"
```

9. Create the application menu launcher. This single command writes the launcher settings and expands `$HOME` to your real home directory:

```bash
cat > "$HOME/.local/share/applications/app.baudbound.runner.desktop" <<EOF
[Desktop Entry]
Version=1.0
Type=Application
Name=BaudBound
Comment=Visual automation runner
Exec=$HOME/.local/opt/baudbound/BaudBound.AppImage
Icon=baudbound
Terminal=false
Categories=Utility;
StartupNotify=true
EOF
```

`Terminal=false` tells the desktop environment to open only the BaudBound window. The launcher should appear in the application menu after a short delay. Sign out and back in if the menu does not refresh. You can pin BaudBound to the taskbar, dock, or favorites from that menu.

##### Optional desktop icon

Many Linux desktops do not show files or launchers directly on the desktop. Skip this section when your desktop does not support desktop icons. The application menu launcher created above will still work.

1. Check whether `xdg-user-dir` is installed. This command finds the correct desktop directory even when it has a localized name:

```bash
command -v xdg-user-dir
```

If the command prints nothing, use the application menu launcher instead.

2. Store the desktop directory path in a temporary shell variable:

```bash
desktop_dir="$(xdg-user-dir DESKTOP)"
```

3. Copy the application menu launcher onto the desktop:

```bash
cp "$HOME/.local/share/applications/app.baudbound.runner.desktop" "$desktop_dir/BaudBound.desktop"
```

4. Allow the copied desktop launcher to run:

```bash
chmod 0755 "$desktop_dir/BaudBound.desktop"
```

Some desktop environments show an **Allow Launching** action the first time you use a new desktop launcher. Choose that action if it appears.

##### Open the desktop application

Open the application menu and choose **BaudBound**. The Dashboard should open without a terminal window. You can also open it from a terminal with this command:

```bash
baudbound
```

The terminal remains connected until you quit BaudBound when you start it this way. Use the application menu launcher for normal desktop use.

If the AppImage reports a FUSE error, use the matching distribution tab in [Linux FUSE packages](#linux-fuse-packages). If `baudbound --version` works, no additional FUSE package is needed.

AppImage compatibility depends on the host architecture, graphics session, and system libraries. Verify it on the intended machine before relying on it.

AppImages are portable executables and are not installed through the system package database. To run BaudBound continuously without the desktop application, follow [Linux Background Service](linux-background-service.md).
{.is-info}

## Linux FUSE packages {.tabset}

Use this section only when the AppImage reports a FUSE error.

### Debian and Ubuntu

Refresh the package information so `apt` knows which versions are currently available:

```bash
sudo apt update
```

Check which FUSE 2 runtime your distribution provides:

```bash
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

## Switching from AppImage to a native package

The native package and AppImage use the same runner data directory, so scripts and configuration do not need to be moved. The old AppImage command and launcher must be removed to avoid opening the wrong copy.

1. Stop active runs and the background runner. Open the tray menu and choose **Quit**.
2. Remove the old AppImage file:

```bash
rm -f "$HOME/.local/opt/baudbound/BaudBound.AppImage"
```

3. Remove the old terminal command link:

```bash
rm -f "$HOME/.local/bin/baudbound"
```

4. Remove either old application-menu launcher if it exists:

```bash
rm -f "$HOME/.local/share/applications/baudbound.desktop"
```

```bash
rm -f "$HOME/.local/share/applications/app.baudbound.runner.desktop"
```

5. Remove the old login launcher if it exists:

```bash
rm -f "$HOME/.config/autostart/BaudBound.desktop"
```

These commands remove only the old application files and launchers. They do not delete installed scripts, configuration, secrets, logs, or run history. Run the automatic installer after cleanup.

## Removing a native package

The application command and native package database identifier are both `baudbound`.

On Debian or Ubuntu, remove the application files owned by APT with:

```bash
sudo apt remove baudbound
```

On Fedora, remove the application files owned by DNF with:

```bash
sudo dnf remove baudbound
```

Package removal leaves the runner data in your user profile. This protects scripts, configuration, secrets, logs, and run history when reinstalling. Delete that data separately only when you intentionally want a complete cleanup.

## First launch

The first launch creates the runner home and a default `config.toml` automatically. You do not need to run `baudbound config init` for a normal desktop installation.

Open **Doctor** in the desktop navigation. A successful check confirms that the configuration and runner storage are accessible and shows which native actions the current machine supports.

Print the installed version to confirm which release is running:

```bash
baudbound --version
```

Run the built-in checks for configuration, storage, and supported native features:

```bash
baudbound doctor
```

Print the configuration file path when you need to find or back up the active settings:

```bash
baudbound config path
```

## Updates

### Windows and AppImage updates

The desktop updater can install updates for the Windows installer and portable Linux AppImage.

1. Start BaudBound normally.
2. When an update is available, review the version and release notes.
3. Choose **Download update** and wait for download and signature verification to finish.
4. Choose **Restart and install**.
5. After BaudBound opens again, confirm the version on the About page.

The AppImage must remain writable by the current user. The per-user installation above satisfies that requirement. Do not change its owner to `root`.

### Debian and RPM updates

BaudBound can check for a newer release from a `.deb` or `.rpm` installation, but it does not replace files owned by APT or DNF. The update dialog and About page therefore show release notes, a link to the latest GitHub Release, and the hosted installer command. They do not show **Download update** or **Restart and install**.

Before updating, stop active runs from the Runs page. Stop the desktop background runner from the Service page. Open the tray menu and choose **Quit**. Closing only the window may leave BaudBound running in the tray.

Run the same command used for installation:

```bash
curl -fsSL https://get.baudbound.app/linux | sh
```

The script detects the distribution again and asks APT or DNF to upgrade the installed package. The package manager requests administrator approval. Open BaudBound normally after the command completes.

You can also download the newer native package manually and run the same `apt install` or `dnf install` command shown in [Manual installation](#manual-installation).

APT and DNF cannot discover BaudBound updates during a normal system upgrade because BaudBound does not yet provide a package repository. Rerun the hosted command or install the newer downloaded package.

### Manual AppImage update

Use these steps only for an AppImage installation when the update dialog cannot complete the update.

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

This option requires `curl` and `jq`. First, check that `curl` is installed. It downloads release information and the AppImage:

```bash
command -v curl
```

Check that `jq` is installed. It safely reads the AppImage URL from GitHub's structured release information:

```bash
command -v jq
```

Both commands must print a path. If either prints nothing, install that command with the distribution's package manager or use the web browser option.

Fetch the AppImage URL from GitHub's latest published release and store it in `APPIMAGE_DOWNLOAD_URL`:

```bash
APPIMAGE_DOWNLOAD_URL="$(curl --fail --silent --show-error "https://api.github.com/repos/NATroutter/BaudBound/releases/latest" | jq -er '.assets | map(select(.name | endswith(".AppImage"))) | if length == 1 then .[0].browser_download_url else error("expected exactly one AppImage asset") end')"
```

Print the stored URL so you can confirm that it points to the official BaudBound GitHub release:

```bash
printf '%s\n' "$APPIMAGE_DOWNLOAD_URL"
```

The printed URL must begin with `https://github.com/NATroutter/BaudBound/releases/download/` and end with `.AppImage`.

Create the Downloads directory if it does not already exist:

```bash
mkdir -p "$HOME/Downloads"
```

Download the AppImage to a predictable filename in that directory:

```bash
curl --fail --location --output "$HOME/Downloads/BaudBound.AppImage" "$APPIMAGE_DOWNLOAD_URL"
```

If the API is unavailable, rate-limited, or does not contain exactly one AppImage, the URL command fails instead of choosing an arbitrary asset. Use the web browser option in that case. A failed download leaves the current installation unchanged.

After completing either option, continue with step 4. Both options produce the same `~/Downloads/BaudBound.AppImage` file used by the remaining update steps.

4. Confirm that the downloaded file exists:

```text
ls -lh "$HOME/Downloads/BaudBound.AppImage"
```

5. Copy the downloaded AppImage over the current installation:

```bash
cp "$HOME/Downloads/BaudBound.AppImage" "$HOME/.local/opt/baudbound/BaudBound.AppImage"
```

Restore its executable permission because downloaded files may not keep that permission:

```bash
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
