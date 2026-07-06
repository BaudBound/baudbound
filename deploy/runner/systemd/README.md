# BaudBound Runner systemd Template

This folder contains a production-oriented Linux `systemd` template for running the headless BaudBound runner.

The service runs `baudbound serve`, keeps trigger listeners alive, and automatically reloads trigger registrations when scripts are imported, updated, removed, enabled, or disabled.

## Files

```text
baudbound-runner.service  systemd unit
runner.env                environment file used by the unit
runner.toml               runner configuration template
```

## Install

Build and install the CLI binary:

```bash
cargo build --release -p baudbound-runner-cli
sudo install -m 0755 target/release/baudbound /usr/local/bin/baudbound
```

Create a dedicated service account:

```bash
sudo useradd --system --home /var/lib/baudbound --shell /usr/sbin/nologin baudbound
```

Install the config and unit files:

```bash
sudo install -d -m 0755 /etc/baudbound
sudo install -m 0644 deploy/runner/systemd/runner.env /etc/baudbound/runner.env
sudo install -m 0644 deploy/runner/systemd/runner.toml /etc/baudbound/runner.toml
sudo install -m 0644 deploy/runner/systemd/baudbound-runner.service /etc/systemd/system/baudbound-runner.service
```

Enable and start the service:

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now baudbound-runner
sudo systemctl status baudbound-runner
```

## Operating The Runner

Use the same storage root as the service when importing or managing scripts:

```bash
sudo -u baudbound BAUDBOUND_HOME=/var/lib/baudbound/runner baudbound import ./script.bbs
sudo -u baudbound BAUDBOUND_HOME=/var/lib/baudbound/runner baudbound approve <script-id-or-name>
sudo -u baudbound BAUDBOUND_HOME=/var/lib/baudbound/runner baudbound list
sudo -u baudbound BAUDBOUND_HOME=/var/lib/baudbound/runner baudbound logs --limit 20
```

The running service reloads trigger registrations automatically, so script imports, updates, removals, enables, and disables do not require a service restart.

View service logs:

```bash
journalctl -u baudbound-runner -f
```

## Serial Devices

Serial Input Trigger nodes store only a logical `deviceId`. Configure the physical port and hardware options on the runner:

```toml
[serial.devices.main_controller]
port = "COM3"
baud_rate = 115200
data_bits = 8
parity = "none"
stop_bits = "1"
flow_control = "none"
read_mode = "line"
auto_reconnect = true
validate_usb_identity = false
# vendor_id = "1A86"
# product_id = "7523"
```

Use the same `deviceId` in Serial Input and Serial Write nodes. Restart the service after changing serial device mappings because they are runner configuration.

## Webhooks

Webhook hosting is disabled by default. To enable it, edit `/etc/baudbound/runner.toml`:

```toml
[triggers]
webhooks_enabled = true

[webhooks]
bind = "127.0.0.1"
port = 43891
```

Restart the service after changing `runner.toml`:

```bash
sudo systemctl restart baudbound-runner
```

Script package changes reload automatically, but runner configuration changes currently require a service restart.
