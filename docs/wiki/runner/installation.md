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

1. Download the `.exe` setup file from the newest non-draft release on the [BaudBound GitHub Releases page](https://github.com/NATroutter/BaudBound/releases).
2. Open the downloaded file and complete the installer.
3. Start **BaudBound** from the Start menu.
4. Confirm that the Dashboard opens without an error banner.

The desktop interface uses Microsoft Edge WebView2. It is already installed on current Windows systems. If the application reports that WebView2 is missing, install the current WebView2 Runtime from Microsoft and launch BaudBound again.

The installer provides the desktop application. It does not guarantee that `baudbound` is added to every terminal's `PATH`. Use the Start menu for normal desktop use. CLI users can run the installed executable directly or add its installation directory to `PATH` themselves.

### Linux

An AppImage is a portable executable. The steps below keep it in your home directory and create a `baudbound` command for your user account. They do not install a system package and do not require `sudo`.

1. Download the `.AppImage` from the newest non-draft release on the [BaudBound GitHub Releases page](https://github.com/NATroutter/BaudBound/releases).
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

### Automatic update

This is the recommended update method for desktop users. No terminal commands are required.

1. Start BaudBound normally.
2. When an update is available, review the version and release notes in the update dialog.
3. Choose **Download update** and wait for the progress bar to finish.
4. Choose **Restart and install**.
5. After BaudBound opens again, confirm the new version in the application.

On Linux, automatic updates require the real AppImage to remain writable by the current user. The per-user installation above satisfies that requirement. Do not change its owner to `root` or point the desktop `baudbound` link at the root-owned headless service copy under `/opt`.

### Manual Linux fallback

Use these steps only when the update dialog cannot complete the update.

1. Open **Service** and stop the desktop background runner.
2. Open the tray menu and choose **Quit**. Closing only the window is not enough because BaudBound normally remains in the tray.
3. Open a terminal and confirm that no BaudBound process remains:

```text
pgrep -af 'BaudBound|baudbound'
```

No output means BaudBound has stopped. If a process is listed, return to the application or service manager and stop it normally before continuing.

4. Download the new AppImage using either method below.

#### Download method {.tabset}

##### Web browser

Open the newest non-draft release on the [BaudBound GitHub Releases page](https://github.com/NATroutter/BaudBound/releases). Download its `.AppImage` file, move it to `~/Downloads` if necessary, and rename the downloaded file to `BaudBound.AppImage`.

##### Terminal with curl

Open the newest non-draft [BaudBound GitHub release](https://github.com/NATroutter/BaudBound/releases) in a browser and copy the link address of its `.AppImage` asset. Replace `APPIMAGE_DOWNLOAD_URL` in this command with that complete copied URL:

```text
curl --fail --location --output "$HOME/Downloads/BaudBound.AppImage" "APPIMAGE_DOWNLOAD_URL"
```

`curl` prints an error and leaves the current installation unchanged when the download fails.

5. Confirm that the downloaded file exists:

```text
ls -lh "$HOME/Downloads/BaudBound.AppImage"
```

6. Copy it over the currently installed AppImage and restore executable permissions:

```text
cp "$HOME/Downloads/BaudBound.AppImage" "$HOME/.local/opt/baudbound/BaudBound.AppImage"
chmod 0755 "$HOME/.local/opt/baudbound/BaudBound.AppImage"
```

7. Verify the installed version, then launch BaudBound:

```text
baudbound --version
baudbound
```

The `baudbound` symlink does not change because it still points to the stable installed path. Manual replacement does not use Tauri's automatic signature-verification flow, so download only from the official GitHub Releases page. Headless operators should use the separate procedure in [Linux Background Service](../self-hosting/linux-background-service.md).

Continue with [Script Management](script-management.md).
