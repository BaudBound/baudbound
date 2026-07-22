---
title: Linux Background Service
description: Run BaudBound continuously under systemd, OpenRC, or runit.
tags: [runner, service, linux]
---
# Linux Background Service

This guide runs `baudbound serve` in the background under your normal Linux user account. The service and your terminal commands therefore use the same configuration, installed scripts, approvals, variables, and run history. You do not need a separate BaudBound account or a second runner home.

Use this setup on a headless machine or whenever triggers must remain active after you close the terminal. For ordinary desktop use, start the background runner from BaudBound's **Service** view instead.

Never run BaudBound as `root`. Scripts receive the permissions of the account running the service, so use a normal account with access only to the files and devices its automations require.
{.is-warning}

## Before you begin

Complete [Installation and Updates](installation.md) for the Linux user that will own the service. These commands must succeed without `sudo`:

```text
command -v baudbound
baudbound --version
baudbound doctor
baudbound config path
```

`command -v baudbound` normally prints `/usr/bin/baudbound` for a Debian or RPM installation. A manual AppImage installation normally prints a path ending in `.local/bin/baudbound`. Keep this executable path and the configuration path printed by `baudbound config path`. The service must use the same executable and runner home.

Print the runner home itself and keep the result for the service-manager instructions:

```text
dirname "$(baudbound config path)"
```

The usual result is `~/.local/share/BaudBound/runner`. A system with a customized `XDG_DATA_HOME` can print a different path. Always use the path from this command.

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

Do not start the desktop background runner and an operating system service for the same user at the same time. They would load the same scripts and can deliver triggers twice or compete for listener ports.
{.is-warning}

## Configure the service {.tabset}

Complete exactly one tab. Do not enable more than one BaudBound service for the same user.

This page configures only the operating system service. Use [Configuration and Serial Devices](configuration.md) for runner settings, [Script Management](script-management.md) for packages, and [Secrets](secrets.md) for protected values.

### systemd

The system service is managed by an administrator, but the BaudBound process runs under your normal account. Record your account, group, and home directory values:

```text
id -un
id -gn
printf '%s\n' "$HOME"
```

Create `/etc/systemd/system/baudbound.service`, replacing every `YOUR_USER`, `YOUR_GROUP`, and `/home/YOUR_USER` value with the output from those commands:

```ini
[Unit]
Description=BaudBound automation runner
Wants=network-online.target
After=network-online.target

[Service]
Type=simple
User=YOUR_USER
Group=YOUR_GROUP
WorkingDirectory=/home/YOUR_USER
Environment=BAUDBOUND_HOME=/home/YOUR_USER/.local/share/BaudBound/runner
ExecStart=/usr/bin/baudbound serve
Restart=on-failure
RestartSec=5s
TimeoutStopSec=30s

[Install]
WantedBy=multi-user.target
```

The `BAUDBOUND_HOME` line uses BaudBound's standard Linux path. If `dirname "$(baudbound config path)"` printed a different path, replace the value after `BAUDBOUND_HOME=` with that absolute path before continuing. If `command -v baudbound` did not print `/usr/bin/baudbound`, replace the `ExecStart` executable with the exact path it printed.

Load and start it:

```text
sudo systemctl daemon-reload
sudo systemctl enable --now baudbound.service
sudo systemctl status baudbound.service
```

The status should report `active (running)`. Follow live logs with:

```text
sudo journalctl -u baudbound.service -f
```

Press `Ctrl+C` to stop following logs without stopping BaudBound.

The `sudo systemctl` commands manage the system unit. `User=YOUR_USER` and `Group=YOUR_GROUP` ensure that the BaudBound process itself does not run as `root`. The service starts during boot and continues after you sign out. The unit behavior is described by the official [systemd service documentation](https://www.freedesktop.org/software/systemd/man/latest/systemd.service.html).

### OpenRC

OpenRC service definitions are managed by the system, but this one runs BaudBound under your normal account. Record your account and group names:

```text
id -un
id -gn
printf '%s\n' "$HOME"
```

Create `/etc/init.d/baudbound`, replacing every `YOUR_USER`, `YOUR_GROUP`, and `/home/YOUR_USER` value with the output from those commands:

```sh
#!/sbin/openrc-run

description="BaudBound automation runner"
command="/usr/bin/baudbound"
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
```

Also replace the `BAUDBOUND_HOME` value when `dirname "$(baudbound config path)"` printed a different runner home. If `command -v baudbound` printed another path, use that exact path for `command`.

Enable and start it:

```text
sudo chmod 0755 /etc/init.d/baudbound
sudo rc-update add baudbound default
sudo rc-service baudbound start
sudo rc-service baudbound status
```

The status should report that BaudBound is started. Gentoo documents service scripts in its [OpenRC guide](https://wiki.gentoo.org/wiki/OpenRC).

### runit

runit service definitions are managed by the system, but this one runs BaudBound under your normal account. Record your account and group names:

```text
id -un
id -gn
printf '%s\n' "$HOME"
```

Create `/etc/sv/baudbound/run`, replacing every `YOUR_USER`, `YOUR_GROUP`, and `/home/YOUR_USER` value with the output from those commands:

```sh
#!/bin/sh

export BAUDBOUND_HOME="/home/YOUR_USER/.local/share/BaudBound/runner"

cd /home/YOUR_USER || exit 1
exec chpst -u YOUR_USER:YOUR_GROUP /usr/bin/baudbound serve
```

Also replace the `BAUDBOUND_HOME` value when `dirname "$(baudbound config path)"` printed a different runner home. If `command -v baudbound` printed another path, replace `/usr/bin/baudbound` with that exact path.

Enable and inspect it on Void Linux:

```text
sudo chmod 0755 /etc/sv/baudbound/run
sudo ln -s /etc/sv/baudbound /var/service/baudbound
sudo sv status baudbound
```

The status should report `run`. Void documents service directories and `/var/service` enablement in its [Services and Daemons guide](https://docs.voidlinux.org/config/services/).

## Service controls

Use only the commands for the configured service manager:

| Task | systemd | OpenRC | runit |
| --- | --- | --- | --- |
| Status | `sudo systemctl status baudbound` | `sudo rc-service baudbound status` | `sudo sv status baudbound` |
| Restart | `sudo systemctl restart baudbound` | `sudo rc-service baudbound restart` | `sudo sv restart baudbound` |
| Stop | `sudo systemctl stop baudbound` | `sudo rc-service baudbound stop` | `sudo sv down baudbound` |
| Start | `sudo systemctl start baudbound` | `sudo rc-service baudbound start` | `sudo sv up baudbound` |

## Remove the background service

Removing the service definition does not remove installed scripts or runner data.

Choose only the configured service manager:

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

The normal BaudBound installation for your user remains available after removing the service. To remove the application or erase its data, follow [Installation and Updates](installation.md) and [Storage, Backups, and Recovery](storage-backups.md). Back up the runner home before deleting it.
