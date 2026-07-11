---
title: Linux Background Service
description: Run BaudBound continuously under systemd, OpenRC, or runit.
tags: [self-hosting, runner, service, linux]
---
# Linux Background Service

Run `baudbound serve` under the service manager provided by the distribution. The installation preparation below covers Debian, Ubuntu, Fedora, and Arch. Service definitions are provided for systemd, OpenRC, and runit.

All variants below use:

- executable: `/opt/baudbound/BaudBound.AppImage`
- service account: `baudbound`
- runner home: `/var/lib/baudbound`
- optional secret environment: `/etc/baudbound/runner.env`

An AppImage is a portable executable rather than a traditional installed package. These instructions copy the downloaded release to a stable path so service definitions do not change with every version.
{.is-info}

Download the current x86-64 AppImage from the project's GitHub release before continuing. Keep only the release you intend to install in the current directory when using the wildcard commands below.

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

## Install and initialize BaudBound

The package names above follow the AppImage project's [FUSE troubleshooting guidance](https://docs.appimage.org/user-guide/troubleshooting/fuse.html) and Fedora's current [`fuse-libs` package](https://packages.fedoraproject.org/pkgs/fuse/fuse-libs/).

Create the application, configuration, and state directories, then install the downloaded AppImage:

```text
sudo install -d -m 0755 /opt/baudbound /etc/baudbound
sudo install -d -o baudbound -g baudbound -m 0750 /var/lib/baudbound
sudo install -m 0755 BaudBound_*.AppImage /opt/baudbound/BaudBound.AppImage
sudo -u baudbound env BAUDBOUND_HOME=/var/lib/baudbound /opt/baudbound/BaudBound.AppImage config init
sudo -u baudbound env BAUDBOUND_HOME=/var/lib/baudbound /opt/baudbound/BaudBound.AppImage --version
```

The downloaded filename can differ between releases. The destination remains `/opt/baudbound/BaudBound.AppImage`; replace it atomically with the newer AppImage during a manual update.

Edit `/var/lib/baudbound/config.toml`, then keep it writable by `baudbound` when automatic serial-port rebinding is enabled. Add the account to the distribution's serial-device group, commonly `dialout` or `uucp`, when scripts use serial ports.

For headless secrets, generate a key:

```text
/opt/baudbound/BaudBound.AppImage secret generate-key
```

Create the environment file with restricted permissions, then use `sudoedit` to paste the printed assignment into it:

```text
sudo install -m 0600 -o root -g root /dev/null /etc/baudbound/runner.env
sudoedit /etc/baudbound/runner.env
```

Omit the file when no installed script uses secrets. CLI commands that access secret values must receive the same key through the host's secret-management process.

Do not run the desktop-owned background runner and an operating-system service against the same `BAUDBOUND_HOME`. Two service owners can deliver triggers twice and conflict over listener ports.
{.is-warning}

## Service manager {.tabset}

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

OpenRC service scripts live in `/etc/init.d`; Alpine documents service control through `rc-service` and boot enablement through `rc-update` in its [OpenRC guide](https://wiki.alpinelinux.org/wiki/OpenRC).

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

Use graceful stop and restart commands from the selected service manager. The runner handles `baudbound serve` as a foreground process and reloads durable script lifecycle changes automatically. Restart the service after editing `config.toml` so listener settings are reconstructed from the new configuration.
