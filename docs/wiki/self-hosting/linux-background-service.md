---
title: Linux Background Service
description: Run BaudBound continuously under systemd, OpenRC, or runit.
tags: [self-hosting, runner, service, linux]
---
# Linux Background Service

This guide runs `baudbound serve` in the background under your normal Linux user account. The service and your terminal commands therefore use the same configuration, installed scripts, approvals, variables, and run history. You do not need a separate BaudBound account or a second runner home.

Use this setup on a headless machine or whenever triggers must remain active after you close the terminal. For ordinary desktop use, start the background runner from BaudBound's **Service** view instead.

Never run BaudBound as `root`. Scripts receive the permissions of the account running the service, so use a normal account with access only to the files and devices its automations require.
{.is-warning}

## Before you begin

Complete [Installation and Updates](../runner/installation.md) for the Linux user that will own the service. These commands must succeed without `sudo`:

```text
command -v baudbound
baudbound --version
baudbound doctor
baudbound config path
```

`command -v baudbound` should normally print a path ending in `.local/bin/baudbound`. Keep the configuration path printed by `baudbound config path`; the service must use that same runner home.

Print the runner home itself and keep the result for the service-manager instructions:

```text
dirname "$(baudbound config path)"
```

The usual result is `~/.local/share/BaudBound/runner`. A system with a customized `XDG_DATA_HOME` can print a different path; always use the path from this command.

Identify the service manager installed by the distribution. Run each check until one prints a path:

```text
command -v systemctl
command -v rc-service
command -v sv
```

| Command that printed a path | Service manager |
| --- | --- |
| `command -v systemctl` | systemd |
| `command -v rc-service` | OpenRC |
| `command -v sv` | runit |

Debian, Ubuntu, Fedora, and Arch normally use systemd. Gentoo commonly uses OpenRC, while Void Linux uses runit. If none of the checks succeeds, consult the distribution documentation instead of installing another service manager only for BaudBound.

Do not start the desktop-owned background runner and an operating-system service for the same user at the same time. They would load the same scripts and can deliver triggers twice or compete for listener ports.
{.is-warning}

## Distribution preparation {.tabset}

Choose the tab for the installed distribution. These packages are needed only when the AppImage reports a FUSE error. If `baudbound --version` already works, continue to [Prepare the runner](#prepare-the-runner).

### Debian and Ubuntu

Update package metadata, then check which FUSE 2 runtime is available:

```text
sudo apt update
apt-cache policy libfuse2t64 libfuse2
```

Compare the `Candidate` lines. Install the package that shows a version instead of `(none)`.

Ubuntu 24.04 and newer normally use:

```text
sudo apt install -y libfuse2t64
```

Debian and older Ubuntu releases normally use:

```text
sudo apt install -y libfuse2
```

Install only one of these packages. If neither has a candidate, confirm that the standard distribution repositories are enabled.

### Fedora

Install Fedora's FUSE 2 compatibility library:

```text
sudo dnf install -y fuse-libs
```

### Arch Linux

Perform a full system upgrade and install FUSE 2. Arch does not support partial upgrades:

```text
sudo pacman -Syu --needed fuse2
```

### Gentoo

Install the FUSE 2 slot:

```text
sudo emerge --ask sys-fs/fuse:0
```

### Void Linux

These instructions support the 64-bit glibc edition of Void Linux. The published AppImage is not supported on Void's musl edition.

```text
sudo xbps-install -S fuse
```

The package names follow the AppImage project's [FUSE troubleshooting guidance](https://docs.appimage.org/user-guide/troubleshooting/fuse.html), Fedora's [`fuse-libs` package](https://packages.fedoraproject.org/pkgs/fuse/fuse-libs/), Gentoo's [`sys-fs/fuse` package](https://packages.gentoo.org/packages/sys-fs/fuse), and Void's official [`fuse` package template](https://github.com/void-linux/void-packages/blob/master/srcpkgs/fuse/template).

## Prepare the runner

BaudBound creates its configuration automatically. Inspect the path and current configuration:

```text
baudbound config path
baudbound config print
```

Edit the `config.toml` path printed by the first command. Review enabled trigger families, listener ports, target runtimes, and serial devices before starting an unattended service.

Import and approve scripts with the normal CLI. For example:

```text
baudbound script import "$HOME/Downloads/automation.bbs"
baudbound script list
baudbound script inspect SCRIPT
baudbound script approve SCRIPT
baudbound script enable SCRIPT
```

Replace `SCRIPT` with the script name or ID shown by `script list`. The same commands continue to work after the background service is enabled.

### Optional secret key

Skip this section when no installed script uses runner secrets.

Generate the encryption key:

```text
baudbound secret generate-key
```

Create a private environment file and open it in your preferred editor:

```text
mkdir -p "$HOME/.config/baudbound"
touch "$HOME/.config/baudbound/runner.env"
chmod 0600 "$HOME/.config/baudbound/runner.env"
```

Paste the complete `BAUDBOUND_SECRET_KEY=...` assignment printed by `secret generate-key` into `~/.config/baudbound/runner.env`. The service definitions below load this file when it exists.

Commands that read or change encrypted secrets must receive the same key. Load it into the current terminal before running those commands:

```text
set -a
. "$HOME/.config/baudbound/runner.env"
set +a
baudbound secret list SCRIPT
```

The key is required to decrypt the stored values. Back it up in a password manager or another protected secret store; losing it makes existing encrypted values unrecoverable.

## Configure the service {.tabset}

Complete exactly one tab. Do not enable more than one BaudBound service for the same user.

### systemd

systemd supports services owned and managed by an ordinary user. Create the user-unit directory:

```text
mkdir -p "$HOME/.config/systemd/user"
```

Create `~/.config/systemd/user/baudbound.service` with this content:

```ini
[Unit]
Description=BaudBound automation runner
Wants=network-online.target
After=network-online.target

[Service]
Type=simple
WorkingDirectory=%h
Environment=BAUDBOUND_HOME=%h/.local/share/BaudBound/runner
EnvironmentFile=-%h/.config/baudbound/runner.env
ExecStart=%h/.local/opt/baudbound/BaudBound.AppImage serve
Restart=on-failure
RestartSec=5s
TimeoutStopSec=30s

[Install]
WantedBy=default.target
```

The `BAUDBOUND_HOME` line above uses BaudBound's standard Linux path. If `dirname "$(baudbound config path)"` printed a different path, replace the value after `BAUDBOUND_HOME=` with that absolute path before continuing.

Load and start it:

```text
systemctl --user daemon-reload
systemctl --user enable --now baudbound.service
systemctl --user status baudbound.service
```

The status should report `active (running)`. Follow live logs with:

```text
journalctl --user -u baudbound.service -f
```

Press `Ctrl+C` to stop following logs without stopping BaudBound.

A user service normally stops when the user signs out. To keep it running after logout and start it during boot, enable lingering once:

```text
sudo loginctl enable-linger "$USER"
```

This command changes login management for only the current account. The service itself still runs without root privileges. The unit behavior is described by the official [systemd user-service documentation](https://www.freedesktop.org/software/systemd/man/latest/systemd.service.html).

### OpenRC

OpenRC service definitions are system-wide, but this one runs BaudBound under your normal account. Record your account and group names:

```text
id -un
id -gn
printf '%s\n' "$HOME"
```

Create `/etc/init.d/baudbound`, replacing every `YOUR_USER`, `YOUR_GROUP`, and `/home/YOUR_USER` value with the output from those commands:

```sh
#!/sbin/openrc-run

description="BaudBound automation runner"
command="/home/YOUR_USER/.local/opt/baudbound/BaudBound.AppImage"
command_args="serve"
command_user="YOUR_USER:YOUR_GROUP"
directory="/home/YOUR_USER"
supervisor="supervise-daemon"
respawn_delay=5
respawn_max=0

depend() {
    need net
}

export BAUDBOUND_HOME="/home/YOUR_USER/.local/share/BaudBound/runner"
if [ -r /home/YOUR_USER/.config/baudbound/runner.env ]; then
    set -a
    . /home/YOUR_USER/.config/baudbound/runner.env
    set +a
fi
```

Also replace the `BAUDBOUND_HOME` value when `dirname "$(baudbound config path)"` printed a different runner home.

Enable and start it:

```text
sudo chmod 0755 /etc/init.d/baudbound
sudo rc-update add baudbound default
sudo rc-service baudbound start
sudo rc-service baudbound status
```

The status should report that BaudBound is started. Gentoo documents service scripts in its [OpenRC guide](https://wiki.gentoo.org/wiki/OpenRC).

### runit

runit service definitions are system-wide, but this one runs BaudBound under your normal account. Record your account and group names:

```text
id -un
id -gn
printf '%s\n' "$HOME"
```

Create `/etc/sv/baudbound/run`, replacing every `YOUR_USER`, `YOUR_GROUP`, and `/home/YOUR_USER` value with the output from those commands:

```sh
#!/bin/sh

export BAUDBOUND_HOME="/home/YOUR_USER/.local/share/BaudBound/runner"
if [ -r /home/YOUR_USER/.config/baudbound/runner.env ]; then
    set -a
    . /home/YOUR_USER/.config/baudbound/runner.env
    set +a
fi

cd /home/YOUR_USER || exit 1
exec chpst -u YOUR_USER:YOUR_GROUP /home/YOUR_USER/.local/opt/baudbound/BaudBound.AppImage serve
```

Also replace the `BAUDBOUND_HOME` value when `dirname "$(baudbound config path)"` printed a different runner home.

Enable and inspect it on Void Linux:

```text
sudo chmod 0755 /etc/sv/baudbound/run
sudo ln -s /etc/sv/baudbound /var/service/baudbound
sudo sv status baudbound
```

The status should report `run`. Void documents service directories and `/var/service` enablement in its [Services and Daemons guide](https://docs.voidlinux.org/config/services/).

## Day-to-day operation

Use normal BaudBound commands from your user account. No account switching, custom home argument, or full executable path is needed:

```text
baudbound script import "$HOME/Downloads/automation.bbs"
baudbound script list
baudbound script inspect SCRIPT
baudbound script approve SCRIPT
baudbound script enable SCRIPT
```

The running service detects durable script imports, updates, approvals, and enablement changes automatically. Restart it after changing `config.toml` because listener and trigger services must be reconstructed.

Use only the commands for the configured service manager:

| Task | systemd | OpenRC | runit |
| --- | --- | --- | --- |
| Status | `systemctl --user status baudbound` | `sudo rc-service baudbound status` | `sudo sv status baudbound` |
| Restart | `systemctl --user restart baudbound` | `sudo rc-service baudbound restart` | `sudo sv restart baudbound` |
| Stop | `systemctl --user stop baudbound` | `sudo rc-service baudbound stop` | `sudo sv down baudbound` |
| Start | `systemctl --user start baudbound` | `sudo rc-service baudbound start` | `sudo sv up baudbound` |

## Updating the headless runner

Stop the service before replacing the AppImage. Run only the matching command:

**systemd**

```text
systemctl --user stop baudbound.service
```

**OpenRC**

```text
sudo rc-service baudbound stop
```

**runit**

```text
sudo sv down baudbound
```

Confirm the service is stopped with the matching status command from the table above.

Download the new AppImage from the latest published release using either option.

### Option A: Web browser

1. Open the [BaudBound GitHub Releases page](https://github.com/NATroutter/BaudBound/releases).
2. Open the latest published release and download its `.AppImage` file.
3. Move the file to `~/Downloads/BaudBound.AppImage` on the service machine.

### Option B: Terminal

This option requires `curl` and `jq`. Both checks must print a path:

```text
command -v curl
command -v jq
```

Fetch the AppImage URL from GitHub's latest published release:

```text
APPIMAGE_DOWNLOAD_URL="$(curl --fail --silent --show-error "https://api.github.com/repos/NATroutter/BaudBound/releases/latest" | jq -er '.assets | map(select(.name | endswith(".AppImage"))) | if length == 1 then .[0].browser_download_url else error("expected exactly one AppImage asset") end')"
printf '%s\n' "$APPIMAGE_DOWNLOAD_URL"
```

The printed URL must use the official BaudBound GitHub repository and end in `.AppImage`. Download it:

```text
mkdir -p "$HOME/Downloads"
curl --fail --location --output "$HOME/Downloads/BaudBound.AppImage" "$APPIMAGE_DOWNLOAD_URL"
```

If the command fails, use the web-browser option instead of choosing an arbitrary asset.

After either download method, replace the installed user-owned AppImage:

```text
ls -lh "$HOME/Downloads/BaudBound.AppImage"
cp "$HOME/Downloads/BaudBound.AppImage" "$HOME/.local/opt/baudbound/BaudBound.AppImage"
chmod 0755 "$HOME/.local/opt/baudbound/BaudBound.AppImage"
baudbound --version
```

The final command must print the intended new version. Start the service with the matching start command from the table and confirm its status before leaving it unattended.

Manual replacement does not use Tauri's automatic signature verification. Download only from the official GitHub Releases page.
{.is-warning}

## Remove the background service

Removing the service definition does not remove installed scripts or runner data.

Choose only the configured service manager:

**systemd**

```text
systemctl --user disable --now baudbound.service
rm "$HOME/.config/systemd/user/baudbound.service"
systemctl --user daemon-reload
```

If lingering was enabled only for BaudBound and no other user service needs it, disable it:

```text
sudo loginctl disable-linger "$USER"
```

**OpenRC**

```text
sudo rc-service baudbound stop
sudo rc-update del baudbound default
sudo rm /etc/init.d/baudbound
```

**runit**

```text
sudo sv down baudbound
sudo rm /var/service/baudbound
sudo rm -r /etc/sv/baudbound
```

The normal per-user BaudBound installation remains available after removing the service. To remove the application or erase its data, follow [Installation and Updates](../runner/installation.md) and [Storage, Backups, and Recovery](../runner/storage-backups.md). Back up the runner home before deleting it.
