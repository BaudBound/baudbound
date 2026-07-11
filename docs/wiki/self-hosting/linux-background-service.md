---
title: Linux Background Service
description: Run BaudBound continuously under systemd, OpenRC, or runit.
tags: [self-hosting, runner, service, linux]
---
# Linux Background Service

Run `baudbound serve` under the service manager provided by the distribution. The installation preparation below covers Debian, Ubuntu, Fedora, Arch, Gentoo, and glibc-based Void Linux. Service definitions are provided for systemd, OpenRC, and runit.

Use this guide only when BaudBound must continue listening for triggers after you sign out. For ordinary desktop use, start the desktop-owned background runner from the application's **Service** view instead.

## Before you begin

You need a 64-bit Linux machine, administrator access through `sudo`, and a downloaded BaudBound AppImage. First launch the AppImage manually and confirm that it runs on the machine. Do not create a service for an AppImage that already fails when started directly.

Choose only the service-manager tab used by the machine:

Run each check until one prints a path:

```text
command -v systemctl
command -v rc-service
command -v sv
```

Use the matching service-manager tab:

| Command that printed a path | Use |
| --- | --- |
| `command -v systemctl` | systemd |
| `command -v rc-service` | OpenRC |
| `command -v sv` | runit |

Debian, Ubuntu, Fedora, and Arch normally use systemd. If none of these checks succeeds, consult the operating-system documentation before continuing rather than installing another service manager only for BaudBound.

All variants below use:

- executable: `/opt/baudbound/BaudBound.AppImage`
- service account: `baudbound`
- runner home: `/var/lib/baudbound`
- optional secret environment: `/etc/baudbound/runner.env`

An AppImage is a portable executable rather than a traditional installed package. These instructions copy the downloaded release to a stable path so service definitions do not change with every version.
{.is-info}

Download the current x86-64 AppImage from the [BaudBound GitHub Releases page](https://github.com/NATroutter/BaudBound/releases). Open a terminal in the download directory and verify that exactly one matching file is present:

```text
cd ~/Downloads
ls -1 BaudBound_*.AppImage
```

If more than one filename is printed, move old releases elsewhere or replace the wildcard in later commands with the exact new filename.

Each distribution tab installs FUSE 2 and creates the same `baudbound` service account. If the group or user already exists, inspect the existing account instead of recreating it. It must use the `baudbound` group and `/var/lib/baudbound` home used by this guide.

## Distribution preparation {.tabset}

### Debian and Ubuntu

Update the package metadata, then check which FUSE 2 runtime the configured repositories provide:

```text
sudo apt update
apt-cache policy libfuse2t64 libfuse2
```

Compare the `Candidate` lines in the output. Install the package that has an available version rather than `(none)`.

Ubuntu 24.04 and newer normally provide `libfuse2t64`:

```text
sudo apt install -y libfuse2t64
```

Debian and older Ubuntu releases normally provide `libfuse2`:

```text
sudo apt install -y libfuse2
```

Install only the package available for the current distribution. If both candidates are `(none)`, verify that the standard distribution repositories are enabled before continuing.

Create the service identity:

```text
sudo groupadd --system baudbound
sudo useradd --system --gid baudbound --home-dir /var/lib/baudbound --create-home --shell "$(command -v nologin)" baudbound
```

### Fedora

Install Fedora's FUSE 2 compatibility libraries:

```text
sudo dnf install -y fuse-libs
```

Create the service identity:

```text
sudo groupadd --system baudbound
sudo useradd --system --gid baudbound --home-dir /var/lib/baudbound --create-home --shell "$(command -v nologin)" baudbound
```

### Arch Linux

Perform a full system upgrade and install FUSE 2. Arch does not support partial upgrades:

```text
sudo pacman -Syu --needed fuse2
```

Create the service identity:

```text
sudo groupadd --system baudbound
sudo useradd --system --gid baudbound --home-dir /var/lib/baudbound --create-home --shell "$(command -v nologin)" baudbound
```

### Gentoo

Install the FUSE 2 slot required by AppImage, then create the service identity:

```text
sudo emerge --ask sys-fs/fuse:0
sudo groupadd --system baudbound
sudo useradd --system --gid baudbound --home-dir /var/lib/baudbound --create-home --shell "$(command -v nologin)" baudbound
```

Choose this tab only for a Gentoo installation using OpenRC. Confirm the AppImage runs manually before creating the OpenRC service.

### Void Linux

These instructions require the glibc edition of 64-bit Void Linux. The published AppImage is not supported on Void's musl edition.

Install FUSE 2 and create the service identity:

```text
sudo xbps-install -S fuse
sudo groupadd --system baudbound
sudo useradd --system --gid baudbound --home-dir /var/lib/baudbound --create-home --shell "$(command -v nologin)" baudbound
```

Confirm the AppImage runs manually before creating the runit service.

## Install and initialize BaudBound

The package names above follow the AppImage project's [FUSE troubleshooting guidance](https://docs.appimage.org/user-guide/troubleshooting/fuse.html), Fedora's current [`fuse-libs` package](https://packages.fedoraproject.org/pkgs/fuse/fuse-libs/), Gentoo's [`sys-fs/fuse` package](https://packages.gentoo.org/packages/sys-fs/fuse), and Void's official [`fuse` package template](https://github.com/void-linux/void-packages/blob/master/srcpkgs/fuse/template).

Create the application, configuration, and state directories:

```text
sudo mkdir -p /opt/baudbound
sudo mkdir -p /etc/baudbound
sudo mkdir -p /var/lib/baudbound
sudo chown baudbound:baudbound /var/lib/baudbound
sudo chmod 0750 /var/lib/baudbound
```

Copy the downloaded AppImage to its stable service path, set its ownership and permissions, and create a system-wide `baudbound` command:

```text
sudo cp BaudBound_*.AppImage /opt/baudbound/BaudBound.AppImage
sudo chown root:root /opt/baudbound/BaudBound.AppImage
sudo chmod 0755 /opt/baudbound/BaudBound.AppImage
sudo ln -sfn /opt/baudbound/BaudBound.AppImage /usr/local/bin/baudbound
```

Initialize the runner home as the service account and verify the version:

```text
sudo -u baudbound env BAUDBOUND_HOME=/var/lib/baudbound /opt/baudbound/BaudBound.AppImage config init
sudo -u baudbound env BAUDBOUND_HOME=/var/lib/baudbound /opt/baudbound/BaudBound.AppImage --version
```

The final command must print the expected BaudBound version. Stop here and correct the file path, permissions, architecture, or missing libraries if it does not.

The downloaded filename can differ between releases. The destination always remains `/opt/baudbound/BaudBound.AppImage`.

Edit the generated configuration:

```text
sudoedit /var/lib/baudbound/config.toml
```

The default listener addresses use loopback and are not reachable from other machines. Review trigger families, ports, and target runtimes before starting the service. Keep this file writable by `baudbound` when automatic serial-port rebinding is enabled. Add the account to the distribution's serial-device group, commonly `dialout` or `uucp`, only when scripts use serial ports.

For headless secrets, generate a key:

```text
sudo -u baudbound env BAUDBOUND_HOME=/var/lib/baudbound /opt/baudbound/BaudBound.AppImage secret generate-key
```

Create the environment file with restricted permissions, then use `sudoedit` to paste the printed assignment into it:

```text
sudo touch /etc/baudbound/runner.env
sudo chown root:root /etc/baudbound/runner.env
sudo chmod 0600 /etc/baudbound/runner.env
sudoedit /etc/baudbound/runner.env
```

The generate command prints an assignment beginning with `BAUDBOUND_SECRET_KEY=`. Paste that entire assignment into `runner.env`, save it, and close the editor. Omit the file when no installed script uses secrets. CLI commands that access secret values must receive the same key through the host's secret-management process.

Do not run the desktop-owned background runner and an operating-system service against the same `BAUDBOUND_HOME`. Two service owners can deliver triggers twice and conflict over listener ports.
{.is-warning}

## Service manager {.tabset}

Complete exactly one tab. Do not install or enable multiple BaudBound service definitions for the same runner home.

### systemd

Create `/etc/systemd/system/baudbound.service`:

```ini
[Unit]
Description=BaudBound automation runner
Wants=network-online.target
After=network-online.target

[Service]
Type=simple
User=baudbound
Group=baudbound
WorkingDirectory=/var/lib/baudbound
Environment=BAUDBOUND_HOME=/var/lib/baudbound
EnvironmentFile=-/etc/baudbound/runner.env
ExecStart=/opt/baudbound/BaudBound.AppImage serve
Restart=on-failure
RestartSec=5s
TimeoutStopSec=30s

[Install]
WantedBy=multi-user.target
```

Load, enable, and inspect it:

```text
sudo systemctl daemon-reload
sudo systemctl enable --now baudbound.service
sudo systemctl status baudbound.service
sudo journalctl -u baudbound.service -f
```

`systemctl status` should report `active (running)`. The `journalctl` command follows live logs; press `Ctrl+C` to stop following logs without stopping BaudBound.

After changing the unit, run `daemon-reload` and restart it. Installed-script lifecycle changes reload automatically; changes to `config.toml` require a service restart.

The unit fields and restart behavior are defined by the official [systemd service documentation](https://www.freedesktop.org/software/systemd/man/latest/systemd.service.html).

### OpenRC

Create `/etc/init.d/baudbound`:

```sh
#!/sbin/openrc-run

description="BaudBound automation runner"
command="/opt/baudbound/BaudBound.AppImage"
command_args="serve"
command_user="baudbound:baudbound"
directory="/var/lib/baudbound"
supervisor="supervise-daemon"
respawn_delay=5
respawn_max=0

depend() {
    need net
}

export BAUDBOUND_HOME="/var/lib/baudbound"
if [ -r /etc/baudbound/runner.env ]; then
    set -a
    . /etc/baudbound/runner.env
    set +a
fi
```

Enable it:

```text
sudo chmod 0755 /etc/init.d/baudbound
sudo rc-update add baudbound default
sudo rc-service baudbound start
sudo rc-service baudbound status
```

OpenRC service scripts live in `/etc/init.d`. Gentoo documents OpenRC as its dependency-based service manager in the [Gentoo OpenRC guide](https://wiki.gentoo.org/wiki/OpenRC).

### runit

Create `/etc/sv/baudbound/run`:

```sh
#!/bin/sh

export BAUDBOUND_HOME="/var/lib/baudbound"
if [ -r /etc/baudbound/runner.env ]; then
    set -a
    . /etc/baudbound/runner.env
    set +a
fi

cd /var/lib/baudbound || exit 1
exec chpst -u baudbound:baudbound /opt/baudbound/BaudBound.AppImage serve
```

Enable and inspect it on Void Linux:

```text
sudo chmod 0755 /etc/sv/baudbound/run
sudo ln -s /etc/sv/baudbound /var/service/baudbound
sudo sv status baudbound
```

runit expects the `run` script to keep the service in the foreground and restarts it when it exits. Void documents service directories and `/var/service` enablement in its [Services and Daemons guide](https://docs.voidlinux.org/config/services/).

## Operating the service

Import, inspect, approve, and enable packages using the same `BAUDBOUND_HOME` and service account. Do not run the desktop-owned background runner and a system service against the same runner home; that can duplicate trigger delivery and conflict over listener ports.

For example, copy a package to a location readable by the service account, then run lifecycle commands as that account:

```text
sudo -u baudbound env BAUDBOUND_HOME=/var/lib/baudbound /opt/baudbound/BaudBound.AppImage script import /path/to/automation.bbs
sudo -u baudbound env BAUDBOUND_HOME=/var/lib/baudbound /opt/baudbound/BaudBound.AppImage script list
sudo -u baudbound env BAUDBOUND_HOME=/var/lib/baudbound /opt/baudbound/BaudBound.AppImage script inspect SCRIPT
sudo -u baudbound env BAUDBOUND_HOME=/var/lib/baudbound /opt/baudbound/BaudBound.AppImage script approve SCRIPT
sudo -u baudbound env BAUDBOUND_HOME=/var/lib/baudbound /opt/baudbound/BaudBound.AppImage script enable SCRIPT
```

Replace `/path/to/automation.bbs` and `SCRIPT` with real values. The service notices imported and enabled scripts at the configured reload interval.

Use graceful stop and restart commands from the selected service manager. The runner handles `baudbound serve` as a foreground process and reloads durable script lifecycle changes automatically. Restart the service after editing `config.toml` so listener settings are reconstructed from the new configuration.

## Updating the headless runner

The desktop update dialog is not used for a root-owned headless installation. Update it manually:

1. Stop BaudBound with the command for the active service manager:

**systemd**

```text
sudo systemctl stop baudbound.service
```

**OpenRC**

```text
sudo rc-service baudbound stop
```

**runit**

```text
sudo sv down baudbound
```

Run only one of the three commands above.

2. Confirm that the matching service status reports stopped, down, or inactive before replacing the executable.

3. Download the new AppImage using either method below.

### Download method {.tabset}

#### Web browser

Open the newest non-draft release on the [BaudBound GitHub Releases page](https://github.com/NATroutter/BaudBound/releases). Download its `.AppImage` file, transfer it to the server if the browser is on another machine, and save it as `~/Downloads/BaudBound.AppImage` for the administrator account running these commands.

#### Terminal with curl

Open the newest non-draft [BaudBound GitHub release](https://github.com/NATroutter/BaudBound/releases) and copy the link address of its `.AppImage` asset. Replace `APPIMAGE_DOWNLOAD_URL` with that complete copied URL:

```text
mkdir -p "$HOME/Downloads"
curl --fail --location --output "$HOME/Downloads/BaudBound.AppImage" "APPIMAGE_DOWNLOAD_URL"
```

4. Confirm that the downloaded file exists:

```text
ls -lh "$HOME/Downloads/BaudBound.AppImage"
```

5. Copy it directly over the installed AppImage, then restore the expected owner and permissions:

```text
sudo cp "$HOME/Downloads/BaudBound.AppImage" /opt/baudbound/BaudBound.AppImage
sudo chown root:root /opt/baudbound/BaudBound.AppImage
sudo chmod 0755 /opt/baudbound/BaudBound.AppImage
```

6. Verify the installed version before restarting the service:

```text
/opt/baudbound/BaudBound.AppImage --version
```

The command must print the intended new version. Correct the download, copy, ownership, or executable permission before continuing if it does not.

7. Start BaudBound using the active service manager:

**systemd**

```text
sudo systemctl start baudbound.service
sudo systemctl status baudbound.service
```

**OpenRC**

```text
sudo rc-service baudbound start
sudo rc-service baudbound status
```

**runit**

```text
sudo sv up baudbound
sudo sv status baudbound
```

Run only the matching pair. The final status must report that BaudBound is running before leaving the update unattended.

Manual replacement does not use Tauri's automatic signature-verification flow. Download only from the official GitHub Releases page and never install an AppImage received from an untrusted mirror or message attachment.

## Uninstall or remove the service

Decide first whether to retain `/var/lib/baudbound`. It contains installed packages, approvals, run history, persistent/global variables, encrypted secrets, service state, and configuration. Deleting it is permanent unless you have a tested backup. See [Storage, Backups, and Recovery](../runner/storage-backups.md).

Stop and disable exactly the service manager you configured:

**systemd**

```text
sudo systemctl disable --now baudbound.service
sudo rm /etc/systemd/system/baudbound.service
sudo systemctl daemon-reload
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

Run only the matching block. Remove the executable and system command after the service no longer appears as running:

```text
sudo rm /usr/local/bin/baudbound
sudo rm /opt/baudbound/BaudBound.AppImage
sudo rmdir /opt/baudbound
```

To retain runner state, leave `/var/lib/baudbound`, `/etc/baudbound/runner.env`, and the `baudbound` account in place. To erase the installation completely, back up anything needed, remove those directories, and then remove the service account with the distribution's native account tool. Account-removal syntax differs by distribution, so follow its documentation rather than copying a command for another system.
