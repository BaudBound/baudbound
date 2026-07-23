import { Plus, RefreshCcw, RotateCcw, Save, Trash2 } from "lucide-react";
import { type ReactNode, useCallback, useEffect, useMemo, useState } from "react";

import { TomlCodeEditor } from "@/components/toml-code-editor";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { MultiSelect } from "@/components/ui/multi-select";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import type { DashboardAction } from "@/lib/app-types";
import { formatCount } from "@/lib/count-format";
import {
  BIND_ADDRESS_MAX_LENGTH,
  RUNNER_CONFIG_MAX_BYTES,
  RUNNER_TEXT_INPUT_MAX_LENGTH,
  SERIAL_DEVICE_ID_MAX_LENGTH,
  SERIAL_METADATA_MAX_LENGTH,
  utf8Length,
} from "@/lib/input-limits";
import {
  type DashboardPayload,
  type RunnerConfig,
  type SerialDeviceSettings,
  readRunnerConfig,
  resetRunnerConfig,
  saveRunnerConfig,
  saveRunnerConfigModel,
} from "@/lib/runner-api";
import { cn } from "@/lib/utils";
import { BrowserOriginField } from "@/views/config/browser-origin-field";
import {
  ConfigGroupHeading,
  DesktopConfiguration,
  SharedConfiguration,
} from "@/views/configuration-preferences";
import { normalizeSerialDeviceId } from "@/views/tools/serial-device-model";

type ConfigMode = "simple" | "advanced";

const defaultSerialDevice: SerialDeviceSettings = {
  auto_reconnect: true,
  auto_rebind_port: false,
  baud_rate: 9_600,
  data_bits: 8,
  dtr_on_open: "deasserted",
  flow_control: "none",
  manufacturer: null,
  max_message_bytes: 1_048_576,
  message_gap_ms: 100,
  open_stabilization_ms: 500,
  parity: "none",
  port: "",
  product_id: null,
  product: null,
  read_mode: "idle_gap",
  serial_number: null,
  stop_bits: "1",
  validate_usb_identity: false,
  vendor_id: null,
};

export function ConfigView({
  busyActions,
  dashboard,
  runAction,
}: {
  busyActions: Set<string>;
  dashboard: DashboardPayload;
  runAction: DashboardAction;
}) {
  const [mode, setMode] = useState<ConfigMode>("simple");
  const [config, setConfig] = useState<RunnerConfig | null>(null);
  const [savedConfig, setSavedConfig] = useState<RunnerConfig | null>(null);
  const [contents, setContents] = useState("");
  const [savedContents, setSavedContents] = useState("");
  const [loadError, setLoadError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [restartBackground, setRestartBackground] = useState(true);
  const [newDeviceId, setNewDeviceId] = useState("");
  const [launchAtLoginRegistered, setLaunchAtLoginRegistered] = useState(false);
  const [resetDialogOpen, setResetDialogOpen] = useState(false);

  const isSaving = busyActions.has("config-save");
  const isResetting = busyActions.has("config-reset");
  const isBackgroundRunning = dashboard.desktop_background.running;
  const isDirty =
    mode === "advanced"
      ? contents !== savedContents
      : JSON.stringify(config) !== JSON.stringify(savedConfig);
  const configBytes = useMemo(() => utf8Length(contents), [contents]);
  const configTooLarge = mode === "advanced" && configBytes > RUNNER_CONFIG_MAX_BYTES;

  const loadConfig = useCallback(async () => {
    setIsLoading(true);
    setLoadError(null);
    try {
      const payload = await readRunnerConfig();
      setConfig(payload.config);
      setSavedConfig(payload.config);
      setContents(payload.contents);
      setSavedContents(payload.contents);
      setLaunchAtLoginRegistered(payload.launch_at_login_registered);
    } catch (error) {
      setLoadError(String(error));
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadConfig();
  }, [loadConfig]);

  async function saveConfig() {
    if (!config) return;
    const saved =
      mode === "advanced"
        ? await runAction("config-save", () =>
            saveRunnerConfig(contents, restartBackground),
          )
        : await runAction("config-save", () =>
            saveRunnerConfigModel(config, restartBackground),
          );
    if (saved) {
      await loadConfig();
    }
  }

  async function resetConfig() {
    const reset = await runAction("config-reset", () => resetRunnerConfig(restartBackground));
    if (reset) {
      setResetDialogOpen(false);
      await loadConfig();
    }
  }

  function addSerialDevice() {
    if (!config) return;
    const id = normalizeSerialDeviceId(newDeviceId);
    if (!id || config.serial.devices[id]) return;
    setConfig({
      ...config,
      serial: {
        devices: {
          ...config.serial.devices,
          [id]: defaultSerialDevice,
        },
      },
    });
    setNewDeviceId("");
  }

  const lineCount = useMemo(() => contents.split("\n").length, [contents]);

  return (
    <div className="grid gap-4">
      <Card>
        <CardHeader className="flex flex-wrap items-center justify-between gap-3">
          <div className="min-w-0">
            <CardTitle>Runner configuration</CardTitle>
            <div className="mt-1 truncate text-xs text-muted-foreground">
              {dashboard.config_path}
            </div>
          </div>
          <ModeSwitch mode={mode} onChange={setMode} />
        </CardHeader>
        <CardContent className="grid gap-3">
          {loadError ? (
            <div className="rounded-md border border-destructive/40 bg-destructive/10 px-3 py-2 text-sm text-destructive">
              {loadError}
            </div>
          ) : null}

          <div className="flex flex-wrap items-center justify-between gap-3">
            <label className="flex items-start gap-3 text-sm text-muted-foreground">
              <Switch
                checked={restartBackground}
                className="mt-0.5"
                disabled={!isBackgroundRunning}
                onCheckedChange={setRestartBackground}
                size="sm"
              />
              <span>
                Restart desktop background runner after saving
                {!isBackgroundRunning ? " (available while it is running)" : ""}
              </span>
            </label>
            <div className="grid w-full grid-cols-3 gap-2 sm:w-auto sm:flex">
              <Button
                disabled={isLoading || isSaving || isResetting || !config}
                onClick={() => setResetDialogOpen(true)}
                variant="destructive"
              >
                <RotateCcw />
                Reset
              </Button>
              <Button
                disabled={isLoading || isSaving || isResetting}
                onClick={loadConfig}
                variant="outline"
              >
                <RefreshCcw className={cn(isLoading && "animate-spin")} />
                Reload
              </Button>
              <Button
                disabled={
                  !isDirty ||
                  configTooLarge ||
                  isSaving ||
                  isResetting ||
                  isLoading ||
                  !config
                }
                onClick={saveConfig}
              >
                <Save />
                {isSaving ? "Saving..." : "Save"}
              </Button>
            </div>
          </div>
        </CardContent>
      </Card>

      {!config ? (
        <Card>
          <CardContent className="text-sm text-muted-foreground">
            {isLoading ? "Loading configuration..." : "Configuration is unavailable."}
          </CardContent>
        </Card>
      ) : mode === "advanced" ? (
        <AdvancedConfigEditor
          byteCount={configBytes}
          contents={contents}
          disabled={isLoading}
          lineCount={lineCount}
          oversized={configTooLarge}
          onChange={setContents}
        />
      ) : (
        <SimpleConfigEditor
          config={config}
          launchAtLoginRegistered={launchAtLoginRegistered}
          newDeviceId={newDeviceId}
          onAddSerialDevice={addSerialDevice}
          onChange={setConfig}
          onNewDeviceIdChange={setNewDeviceId}
        />
      )}

      <Dialog
        onOpenChange={(open) => {
          if (!isResetting) setResetDialogOpen(open);
        }}
        open={resetDialogOpen}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Reset runner configuration?</DialogTitle>
            <DialogDescription>
              This replaces the current config.toml with BaudBound defaults. Desktop, shared,
              runner, network, trigger, and serial settings will be reset. Installed scripts,
              approvals, secrets, variables, and run history will not be removed.
            </DialogDescription>
          </DialogHeader>
          <p className="text-sm text-foreground">
            Any unsaved configuration changes will be discarded.
          </p>
          <DialogFooter>
            <Button
              disabled={isResetting}
              onClick={() => setResetDialogOpen(false)}
              variant="outline"
            >
              Cancel
            </Button>
            <Button disabled={isResetting} onClick={resetConfig} variant="destructive">
              <RotateCcw />
              {isResetting ? "Resetting..." : "Reset to defaults"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}

function ModeSwitch({
  mode,
  onChange,
}: {
  mode: ConfigMode;
  onChange: (mode: ConfigMode) => void;
}) {
  return (
    <div className="grid grid-cols-2 rounded-md border border-border bg-background p-1 text-sm">
      {(["simple", "advanced"] as const).map((item) => (
        <Button
          className={cn(
            "rounded px-3 py-1.5 text-muted-foreground transition-colors",
            mode === item && "bg-muted text-foreground",
          )}
          key={item}
          onClick={() => onChange(item)}
          size="sm"
          type="button"
          variant="subtle"
        >
          {item === "simple" ? "Simple" : "Advanced"}
        </Button>
      ))}
    </div>
  );
}

function SimpleConfigEditor({
  config,
  launchAtLoginRegistered,
  newDeviceId,
  onAddSerialDevice,
  onChange,
  onNewDeviceIdChange,
}: {
  config: RunnerConfig;
  launchAtLoginRegistered: boolean;
  newDeviceId: string;
  onAddSerialDevice: () => void;
  onChange: (config: RunnerConfig) => void;
  onNewDeviceIdChange: (value: string) => void;
}) {
  return (
    <div className="grid gap-4">
      <DesktopConfiguration
        config={config}
        launchAtLoginRegistered={launchAtLoginRegistered}
        onChange={onChange}
      />

      <SharedConfiguration config={config} onChange={onChange} />

      <ConfigGroupHeading
        description="Runtime identity, listeners, networking, and connected serial hardware."
        title="Runner configuration"
      />
      <Card>
        <CardHeader>
          <CardTitle>Runner</CardTitle>
        </CardHeader>
        <CardContent className="grid gap-4 md:grid-cols-2">
          <NumberField
            label="Trigger reload seconds"
            max={86_400}
            min={1}
            onChange={(trigger_reload_seconds) =>
              onChange({
                ...config,
                runner: { ...config.runner, trigger_reload_seconds },
              })
            }
            value={config.runner.trigger_reload_seconds}
          />
          <NumberField
            label="Maximum stored runs"
            max={10_000_000}
            min={1}
            onChange={(run_history_max_records) =>
              onChange({
                ...config,
                runner: { ...config.runner, run_history_max_records },
              })
            }
            value={config.runner.run_history_max_records}
          />
          <NumberField
            label="Run history age in days"
            max={36_500}
            min={1}
            onChange={(run_history_max_age_days) =>
              onChange({
                ...config,
                runner: { ...config.runner, run_history_max_age_days },
              })
            }
            value={config.runner.run_history_max_age_days}
          />
          <TargetRuntimeField
            className="md:col-span-2"
            onChange={(target_runtimes) =>
              onChange({
                ...config,
                runner: { ...config.runner, target_runtimes },
              })
            }
            value={config.runner.target_runtimes}
          />
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>External data limits</CardTitle>
        </CardHeader>
        <CardContent className="grid gap-4 md:grid-cols-3">
          <NumberField
            label="Maximum HTTP response bytes"
            max={4_294_967_296}
            min={1}
            onChange={(max_http_response_bytes) =>
              onChange({
                ...config,
                limits: { ...config.limits, max_http_response_bytes },
              })
            }
            value={config.limits.max_http_response_bytes}
          />
          <NumberField
            label="Maximum file download bytes"
            max={4_294_967_296}
            min={1}
            onChange={(max_file_download_bytes) =>
              onChange({
                ...config,
                limits: { ...config.limits, max_file_download_bytes },
              })
            }
            value={config.limits.max_file_download_bytes}
          />
          <NumberField
            label="Maximum file read bytes"
            max={4_294_967_296}
            min={1}
            onChange={(max_file_read_bytes) =>
              onChange({
                ...config,
                limits: { ...config.limits, max_file_read_bytes },
              })
            }
            value={config.limits.max_file_read_bytes}
          />
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Trigger listeners</CardTitle>
        </CardHeader>
        <CardContent className="grid gap-3 sm:grid-cols-2 xl:grid-cols-3">
          {triggerFields.map(([key, label]) => (
            <BooleanField
              checked={config.triggers[key]}
              key={key}
              label={label}
              onChange={(checked) =>
                onChange({
                  ...config,
                  triggers: { ...config.triggers, [key]: checked },
                })
              }
            />
          ))}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Network listeners</CardTitle>
        </CardHeader>
        <CardContent className="grid gap-5 lg:grid-cols-2">
          <NetworkSection
            allowBrowserOrigins={config.webhooks.allow_browser_origins}
            allowUnauthenticatedPublicBind={
              config.webhooks.allow_unauthenticated_public_bind
            }
            bind={config.webhooks.bind}
            maxBytes={config.webhooks.max_body_bytes}
            maxBytesLabel="Max body bytes"
            onBindChange={(bind) =>
              onChange({ ...config, webhooks: { ...config.webhooks, bind } })
            }
            onAllowBrowserOriginsChange={(allow_browser_origins) =>
              onChange({
                ...config,
                webhooks: { ...config.webhooks, allow_browser_origins },
              })
            }
            onAllowUnauthenticatedPublicBindChange={(allow_unauthenticated_public_bind) =>
              onChange({
                ...config,
                webhooks: { ...config.webhooks, allow_unauthenticated_public_bind },
              })
            }
            onMaxBytesChange={(max_body_bytes) =>
              onChange({
                ...config,
                webhooks: { ...config.webhooks, max_body_bytes },
              })
            }
            onPortChange={(port) =>
              onChange({ ...config, webhooks: { ...config.webhooks, port } })
            }
            port={config.webhooks.port}
            title="Webhooks"
          />
          <NetworkSection
            allowBrowserOrigins={config.websockets.allow_browser_origins}
            allowUnauthenticatedPublicBind={
              config.websockets.allow_unauthenticated_public_bind
            }
            bind={config.websockets.bind}
            maxConnections={config.websockets.max_connections}
            maxBytes={config.websockets.max_message_bytes}
            maxBytesLabel="Max message bytes"
            onBindChange={(bind) =>
              onChange({ ...config, websockets: { ...config.websockets, bind } })
            }
            onAllowBrowserOriginsChange={(allow_browser_origins) =>
              onChange({
                ...config,
                websockets: { ...config.websockets, allow_browser_origins },
              })
            }
            onAllowUnauthenticatedPublicBindChange={(allow_unauthenticated_public_bind) =>
              onChange({
                ...config,
                websockets: { ...config.websockets, allow_unauthenticated_public_bind },
              })
            }
            onMaxBytesChange={(max_message_bytes) =>
              onChange({
                ...config,
                websockets: { ...config.websockets, max_message_bytes },
              })
            }
            onMaxConnectionsChange={(max_connections) =>
              onChange({
                ...config,
                websockets: { ...config.websockets, max_connections },
              })
            }
            onPortChange={(port) =>
              onChange({ ...config, websockets: { ...config.websockets, port } })
            }
            port={config.websockets.port}
            title="WebSockets"
          />
        </CardContent>
      </Card>

      <Card>
        <CardHeader className="flex flex-wrap items-center justify-between gap-3">
          <CardTitle>Serial devices</CardTitle>
          <div className="grid w-full grid-cols-[minmax(0,1fr)_auto] gap-2 sm:w-auto">
            <Input
              maxLength={SERIAL_DEVICE_ID_MAX_LENGTH}
              onChange={(event) => onNewDeviceIdChange(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter") onAddSerialDevice();
              }}
              placeholder="Device ID"
              value={newDeviceId}
            />
            <Button
              disabled={
                !normalizeSerialDeviceId(newDeviceId) ||
                Boolean(config.serial.devices[normalizeSerialDeviceId(newDeviceId)])
              }
              onClick={onAddSerialDevice}
              variant="secondary"
            >
              <Plus />
              Add
            </Button>
          </div>
        </CardHeader>
        <CardContent className="grid gap-4">
          {Object.entries(config.serial.devices).length === 0 ? (
            <div className="rounded-md border border-border bg-background px-3 py-3 text-sm text-muted-foreground">
              No serial devices are configured.
            </div>
          ) : (
            Object.entries(config.serial.devices).map(([id, device]) => (
              <SerialDeviceCard
                device={device}
                id={id}
                key={id}
                onChange={(nextDevice) =>
                  onChange({
                    ...config,
                    serial: {
                      devices: { ...config.serial.devices, [id]: nextDevice },
                    },
                  })
                }
                onRemove={() => {
                  const { [id]: _removed, ...devices } = config.serial.devices;
                  onChange({ ...config, serial: { devices } });
                }}
              />
            ))
          )}
        </CardContent>
      </Card>
    </div>
  );
}

function NetworkSection({
  allowBrowserOrigins,
  allowUnauthenticatedPublicBind,
  bind,
  maxConnections,
  maxBytes,
  maxBytesLabel,
  onAllowBrowserOriginsChange,
  onAllowUnauthenticatedPublicBindChange,
  onBindChange,
  onMaxBytesChange,
  onMaxConnectionsChange,
  onPortChange,
  port,
  title,
}: {
  allowBrowserOrigins: string[];
  allowUnauthenticatedPublicBind: boolean;
  bind: string;
  maxConnections?: number;
  maxBytes: number;
  maxBytesLabel: string;
  onAllowBrowserOriginsChange: (value: string[]) => void;
  onAllowUnauthenticatedPublicBindChange: (value: boolean) => void;
  onBindChange: (value: string) => void;
  onMaxBytesChange: (value: number) => void;
  onMaxConnectionsChange?: (value: number) => void;
  onPortChange: (value: number) => void;
  port: number;
  title: string;
}) {
  return (
    <div className="grid gap-3 rounded-md border border-border bg-background p-3">
      <div className="text-sm font-medium">{title}</div>
      <TextField
        label="Bind address"
        maxLength={BIND_ADDRESS_MAX_LENGTH}
        onChange={onBindChange}
        value={bind}
      />
      <BrowserOriginField
        onChange={onAllowBrowserOriginsChange}
        value={allowBrowserOrigins}
      />
      <NumberField label="Port" max={65535} min={1} onChange={onPortChange} value={port} />
      <NumberField
        label={maxBytesLabel}
        max={4_294_967_296}
        min={1}
        onChange={onMaxBytesChange}
        value={maxBytes}
      />
      {maxConnections !== undefined && onMaxConnectionsChange ? (
        <NumberField
          label="Max concurrent connections"
          max={10_000}
          min={1}
          onChange={onMaxConnectionsChange}
          value={maxConnections}
        />
      ) : null}
      <BooleanField
        checked={allowUnauthenticatedPublicBind}
        label="Allow unauthenticated public bind"
        onChange={onAllowUnauthenticatedPublicBindChange}
      />
      {allowUnauthenticatedPublicBind ? (
        <div className="text-xs text-destructive">
          Unsafe override enabled. Unprotected triggers may be reached from other machines when the
          listener uses a public address.
        </div>
      ) : null}
    </div>
  );
}

function SerialDeviceCard({
  device,
  id,
  onChange,
  onRemove,
}: {
  device: SerialDeviceSettings;
  id: string;
  onChange: (device: SerialDeviceSettings) => void;
  onRemove: () => void;
}) {
  const hasCompleteUsbIdentity = Boolean(
    device.validate_usb_identity && device.vendor_id?.trim() && device.product_id?.trim(),
  );
  return (
    <div className="grid gap-4 rounded-md border border-border bg-background p-3">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div>
          <div className="text-sm font-medium">{id}</div>
          <div className="text-xs text-muted-foreground">Referenced by Serial Input nodes</div>
        </div>
        <Button onClick={onRemove} size="sm" variant="destructive">
          <Trash2 />
          Remove
        </Button>
      </div>
      <SerialConfigGroup
        description="Use the values specified in the manual for the physical device."
        title="Native serial port"
      >
        <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
          <TextField
            label="Port"
            maxLength={RUNNER_TEXT_INPUT_MAX_LENGTH}
            onChange={(port) => onChange({ ...device, port })}
            value={device.port}
          />
          <NumberField
            label="Baud rate"
            max={4_294_967_295}
            min={1}
            onChange={(baud_rate) => onChange({ ...device, baud_rate })}
            value={device.baud_rate}
          />
          <NumberField
            label="Data bits"
            max={8}
            min={5}
            onChange={(data_bits) => onChange({ ...device, data_bits })}
            value={device.data_bits}
          />
          <SelectField
            label="Parity"
            onChange={(parity) => onChange({ ...device, parity })}
            options={["none", "even", "odd"]}
            value={device.parity}
          />
          <SelectField
            label="Stop bits"
            onChange={(stop_bits) => onChange({ ...device, stop_bits })}
            options={["1", "2"]}
            value={device.stop_bits}
          />
          <SelectField
            label="Flow control"
            onChange={(flow_control) => onChange({ ...device, flow_control })}
            options={["none", "software", "hardware"]}
            value={device.flow_control}
          />
          <DtrOnOpenField
            onChange={(dtr_on_open) => onChange({ ...device, dtr_on_open })}
            value={device.dtr_on_open}
          />
        </div>
      </SerialConfigGroup>

      <SerialConfigGroup
        description="These values identify the physical USB device and are filled automatically by the serial scanner when available."
        title="Product information"
      >
        <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
          <TextField
            label="Vendor ID"
            maxLength={6}
            onChange={(vendor_id) => onChange({ ...device, vendor_id: nullableText(vendor_id) })}
            value={device.vendor_id ?? ""}
          />
          <TextField
            label="Product ID"
            maxLength={6}
            onChange={(product_id) =>
              onChange({ ...device, product_id: nullableText(product_id) })
            }
            value={device.product_id ?? ""}
          />
          <TextField
            label="Serial number"
            maxLength={SERIAL_METADATA_MAX_LENGTH}
            onChange={(serial_number) =>
              onChange({ ...device, serial_number: nullableText(serial_number) })
            }
            value={device.serial_number ?? ""}
          />
          <TextField
            label="Manufacturer"
            maxLength={SERIAL_METADATA_MAX_LENGTH}
            onChange={(manufacturer) =>
              onChange({ ...device, manufacturer: nullableText(manufacturer) })
            }
            value={device.manufacturer ?? ""}
          />
          <TextField
            label="Product"
            maxLength={SERIAL_METADATA_MAX_LENGTH}
            onChange={(product) => onChange({ ...device, product: nullableText(product) })}
            value={device.product ?? ""}
          />
        </div>
        {device.validate_usb_identity && !hasCompleteUsbIdentity ? (
          <div className="rounded-md border border-baud-amber/30 bg-baud-amber/10 px-3 py-2 text-xs text-baud-amber">
            Vendor ID and Product ID are required while USB identity validation is enabled.
          </div>
        ) : null}
      </SerialConfigGroup>

      <SerialConfigGroup
        description="BaudBound uses these settings to turn the incoming byte stream into trigger messages."
        title="Message framing"
      >
        <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
          <SerialReadModeField
            onChange={(read_mode) => onChange({ ...device, read_mode })}
            value={device.read_mode}
          />
          {device.read_mode === "idle_gap" ? (
            <NumberField
              label="Message gap in milliseconds"
              max={60_000}
              min={1}
              onChange={(message_gap_ms) => onChange({ ...device, message_gap_ms })}
              value={device.message_gap_ms}
            />
          ) : null}
          {device.read_mode !== "raw" ? (
            <NumberField
              label="Maximum message bytes"
              max={67_108_864}
              min={1}
              onChange={(max_message_bytes) => onChange({ ...device, max_message_bytes })}
              value={device.max_message_bytes}
            />
          ) : null}
        </div>
        <p className="text-xs text-muted-foreground">{serialReadModeDescription(device.read_mode)}</p>
      </SerialConfigGroup>

      <SerialConfigGroup
        description="Control how BaudBound reconnects, verifies, and prepares this device before reading messages."
        title="BaudBound serial behavior"
      >
        <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-3">
          <BooleanField
            checked={device.auto_reconnect}
            label="Reconnect automatically"
            onChange={(auto_reconnect) => onChange({ ...device, auto_reconnect })}
          />
          <BooleanField
            checked={device.validate_usb_identity}
            label="Validate USB identity"
            onChange={(validate_usb_identity) =>
              onChange({
                ...device,
                auto_rebind_port: validate_usb_identity ? device.auto_rebind_port : false,
                validate_usb_identity,
              })
            }
          />
          <BooleanField
            checked={device.auto_rebind_port}
            disabled={!hasCompleteUsbIdentity && !device.auto_rebind_port}
            label="Find a changed port automatically"
            onChange={(auto_rebind_port) => onChange({ ...device, auto_rebind_port })}
          />
          <NumberField
            label="Open stabilization in milliseconds"
            max={60_000}
            min={0}
            onChange={(open_stabilization_ms) =>
              onChange({ ...device, open_stabilization_ms })
            }
            value={device.open_stabilization_ms}
          />
        </div>
        <p className="text-xs text-muted-foreground">
          Deasserted DTR is recommended for scanners and controllers that reset when a serial port
          opens. The stabilization time lets the device become ready before BaudBound reads data.
        </p>
      </SerialConfigGroup>
    </div>
  );
}

function SerialConfigGroup({
  children,
  description,
  title,
}: {
  children: ReactNode;
  description: string;
  title: string;
}) {
  return (
    <section className="grid gap-3 border-t border-border pt-4">
      <div>
        <h4 className="text-sm font-medium">{title}</h4>
        <p className="mt-0.5 text-xs text-muted-foreground">{description}</p>
      </div>
      {children}
    </section>
  );
}

function SerialReadModeField({
  onChange,
  value,
}: {
  onChange: (value: string) => void;
  value: string;
}) {
  return (
    <label className="grid gap-1.5 text-sm">
      <span className="text-xs text-muted-foreground">Read mode</span>
      <Select onValueChange={onChange} value={value}>
        <SelectTrigger aria-label="Read mode">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value="idle_gap">Idle gap</SelectItem>
          <SelectItem value="line">Line endings</SelectItem>
          <SelectItem value="raw">Raw chunks</SelectItem>
        </SelectContent>
      </Select>
    </label>
  );
}

function DtrOnOpenField({
  onChange,
  value,
}: {
  onChange: (value: string) => void;
  value: string;
}) {
  return (
    <label className="grid gap-1.5 text-sm">
      <span className="text-xs text-muted-foreground">DTR when opening</span>
      <Select onValueChange={onChange} value={value}>
        <SelectTrigger aria-label="DTR when opening">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value="deasserted">Deasserted</SelectItem>
          <SelectItem value="asserted">Asserted</SelectItem>
          <SelectItem value="preserve">Preserve current state</SelectItem>
        </SelectContent>
      </Select>
    </label>
  );
}

function serialReadModeDescription(value: string) {
  if (value === "line") return "Dispatches after CR, LF, or CRLF and removes only the ending.";
  if (value === "raw") return "Dispatches native read chunks immediately. Chunks are not message boundaries.";
  return "Dispatches after the device has stopped sending for the configured message gap.";
}

function AdvancedConfigEditor({
  byteCount,
  contents,
  disabled,
  lineCount,
  onChange,
  oversized,
}: {
  byteCount: number;
  contents: string;
  disabled: boolean;
  lineCount: number;
  onChange: (contents: string) => void;
  oversized: boolean;
}) {
  return (
    <Card>
      <CardHeader className="flex flex-row items-center justify-between">
        <CardTitle>Raw TOML</CardTitle>
        <div className="text-xs text-muted-foreground">
          {formatCount(lineCount, "line")}, {formatCount(byteCount, "byte")}
        </div>
      </CardHeader>
      <CardContent className="grid gap-2">
        <TomlCodeEditor disabled={disabled} onChange={onChange} value={contents} />
        {oversized ? (
          <div className="text-xs text-destructive">
            The configuration is larger than the 1,048,576 byte limit and cannot be saved.
          </div>
        ) : null}
      </CardContent>
    </Card>
  );
}

function TextField({
  label,
  maxLength = RUNNER_TEXT_INPUT_MAX_LENGTH,
  onChange,
  value,
}: {
  label: string;
  maxLength?: number;
  onChange: (value: string) => void;
  value: string;
}) {
  return (
    <label className="grid gap-1.5 text-sm">
      <span className="text-xs text-muted-foreground">{label}</span>
      <Input
        maxLength={maxLength}
        onChange={(event) => onChange(event.target.value)}
        value={value}
      />
    </label>
  );
}

function NumberField({
  label,
  max,
  min = 0,
  onChange,
  value,
}: {
  label: string;
  max?: number;
  min?: number;
  onChange: (value: number) => void;
  value: number;
}) {
  return (
    <label className="grid gap-1.5 text-sm">
      <span className="text-xs text-muted-foreground">{label}</span>
      <Input
        max={max}
        min={min}
        onChange={(event) => onChange(clampNumber(event.target.valueAsNumber, min, max))}
        type="number"
        value={Number.isFinite(value) ? value : min}
      />
    </label>
  );
}

function SelectField({
  label,
  onChange,
  options,
  value,
}: {
  label: string;
  onChange: (value: string) => void;
  options: string[];
  value: string;
}) {
  return (
    <label className="grid gap-1.5 text-sm">
      <span className="text-xs text-muted-foreground">{label}</span>
      <Select onValueChange={onChange} value={value}>
        <SelectTrigger>
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {options.map((option) => (
            <SelectItem key={option} value={option}>
              {option}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </label>
  );
}

function TargetRuntimeField({
  className,
  onChange,
  value,
}: {
  className?: string;
  onChange: (value: string[]) => void;
  value: string[];
}) {
  return (
    <label className={cn("grid gap-1.5 text-sm", className)}>
      <span className="text-xs text-muted-foreground">Supported target runtimes</span>
      <MultiSelect
        onChange={onChange}
        options={targetRuntimeOptions}
        placeholder="Host defaults when empty"
        value={value}
      />
      <span className="text-xs text-muted-foreground">
        Leave empty to use this machine&apos;s default headless and desktop runtimes.
      </span>
    </label>
  );
}

function BooleanField({
  checked,
  disabled = false,
  label,
  onChange,
}: {
  checked: boolean;
  disabled?: boolean;
  label: string;
  onChange: (checked: boolean) => void;
}) {
  return (
    <label
      className={cn(
        "flex items-center justify-between gap-3 rounded-md border border-border bg-background px-3 py-2 text-sm",
        disabled && "opacity-60",
      )}
    >
      <span>{label}</span>
      <Switch
        aria-label={label}
        checked={checked}
        disabled={disabled}
        onCheckedChange={onChange}
        size="sm"
      />
    </label>
  );
}

const triggerFields = [
  ["schedules_enabled", "Schedules"],
  ["file_watch_enabled", "File watcher"],
  ["hotkeys_enabled", "Hotkeys"],
  ["process_watch_enabled", "Process watcher"],
  ["serial_enabled", "Serial input"],
  ["startup_enabled", "Startup"],
  ["webhooks_enabled", "Webhooks"],
  ["websockets_enabled", "WebSockets"],
] as const;

const targetRuntimeOptions = [
  "Generic Headless",
  "Windows Headless",
  "Linux Headless",
  "Generic Desktop",
  "Windows Desktop",
  "Linux Desktop",
].map((value) => ({ label: value, value }));

function nullableText(value: string) {
  const trimmed = value.trim();
  return trimmed.length > 0 ? trimmed : null;
}

function clampNumber(value: number, min: number, max?: number) {
  if (!Number.isFinite(value)) return min;
  const integer = Math.trunc(value);
  if (typeof max === "number") return Math.min(Math.max(integer, min), max);
  return Math.max(integer, min);
}
