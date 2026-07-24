import { listen } from "@tauri-apps/api/event";
import {
  CircleAlert,
  Database,
  Eye,
  ListFilter,
  Plus,
  RefreshCw,
  Trash2,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { toast } from "sonner";

import { DetailDialog } from "@/components/detail-dialog";
import { EmptyState } from "@/components/empty-state";
import { ExternalLink } from "@/components/external-link";
import { LazyMarkdownContent } from "@/components/lazy-markdown-content";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
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
import { useDesktopTime } from "@/lib/time-format";
import {
  compareSemanticVersions,
  meetsMinimumRunnerVersion,
  repositoryDisplayName,
  repositoryScriptState,
  repositoryUrlForDisplay,
  type RepositoryScriptState,
} from "@/lib/repository-browser";
import {
  addScriptRepository,
  cancelRepositoryRequest,
  getRepositoryScript,
  getRepositoryScriptFilterOptions,
  getRepositorySources,
  prepareRepositoryScript,
  previewScriptRepository,
  queryRepositoryScripts,
  refreshAllScriptRepositories,
  refreshScriptRepository,
  removeScriptRepository,
  repositoryChangedEvent,
  repositoryProgressEvent,
  setScriptRepositoryEnabled,
  type DashboardPayload,
  type RemotePackageReview,
  type RepositoryRefreshProgress,
  type RepositoryPreview,
  type RepositoryScriptFilterOptions,
  type RepositoryScriptQuery,
  type RepositoryScriptRecord,
  type RepositoryScriptSummary,
  type RepositorySource,
} from "@/lib/runner-api";
import { RemotePackageDialog } from "@/views/remote-package-dialog";

const pageSize = 50;

type ScriptBrowserFilters = {
  capabilities: string[];
  installed: string[];
  permissions: string[];
  repository: string[];
  risk: string[];
  target: string[];
};

const defaultFilters: ScriptBrowserFilters = {
  capabilities: [],
  installed: [],
  permissions: [],
  repository: [],
  risk: [],
  target: [],
};

export function BrowseScriptsView({
  busyActions,
  dashboard,
  runAction,
}: {
  busyActions: Set<string>;
  dashboard: DashboardPayload;
  runAction: DashboardAction;
}) {
  const [repositories, setRepositories] = useState<RepositorySource[]>([]);
  const [filterOptions, setFilterOptions] =
    useState<RepositoryScriptFilterOptions>({
      capabilities: [],
      permissions: [],
    });
  const [scripts, setScripts] = useState<RepositoryScriptSummary[]>([]);
  const [total, setTotal] = useState(0);
  const [search, setSearch] = useState("");
  const [sort, setSort] =
    useState<RepositoryScriptQuery["sort"]>("name");
  const [filters, setFilters] = useState<ScriptBrowserFilters>(defaultFilters);
  const [filterDraft, setFilterDraft] =
    useState<ScriptBrowserFilters>(defaultFilters);
  const [filtersOpen, setFiltersOpen] = useState(false);
  const [managementOpen, setManagementOpen] = useState(false);
  const [page, setPage] = useState(0);
  const [loading, setLoading] = useState(true);
  const [repositoryBusy, setRepositoryBusy] = useState<Set<string>>(new Set());
  const [addOpen, setAddOpen] = useState(false);
  const [removeSource, setRemoveSource] = useState<RepositorySource | null>(null);
  const [mismatchIncident, setMismatchIncident] = useState<{
    message: string;
    script: RepositoryScriptSummary;
  } | null>(null);
  const [repositoryUrl, setRepositoryUrl] = useState("");
  const [repositoryPreview, setRepositoryPreview] = useState<RepositoryPreview | null>(null);
  const [detailScript, setDetailScript] = useState<RepositoryScriptRecord | null>(null);
  const [preparedReview, setPreparedReview] = useState<RemotePackageReview | undefined>();
  const [packageDialogOpen, setPackageDialogOpen] = useState(false);
  const [progress, setProgress] = useState<RepositoryRefreshProgress | null>(null);
  const queryRevision = useRef(0);
  const { formatUnixSeconds } = useDesktopTime();

  const query = useMemo<RepositoryScriptQuery>(
    () => ({
      capabilities: filters.capabilities,
      direction: "ascending",
      installed: filters.installed.map((value) => value === "installed"),
      limit: pageSize,
      offset: page * pageSize,
      permissions: filters.permissions,
      repository_urls: filters.repository,
      risk_levels: filters.risk,
      search,
      sort,
      target_runtimes: filters.target,
    }),
    [filters, page, search, sort],
  );

  const loadRepositories = useCallback(async () => {
    setRepositories(await getRepositorySources());
  }, []);

  const loadFilterOptions = useCallback(async () => {
    setFilterOptions(await getRepositoryScriptFilterOptions());
  }, []);

  const loadScripts = useCallback(async () => {
    const revision = ++queryRevision.current;
    const result = await queryRepositoryScripts(query);
    if (revision !== queryRevision.current) return;
    setScripts(result.items);
    setTotal(result.total);
  }, [query]);

  useEffect(() => {
    setLoading(true);
    Promise.all([loadRepositories(), loadFilterOptions(), loadScripts()])
      .catch((error) => toast.error(`Could not load script repositories: ${String(error)}`))
      .finally(() => setLoading(false));
  }, [loadFilterOptions, loadRepositories, loadScripts]);

  useEffect(() => {
    let unlistenChanged: (() => void) | undefined;
    let unlistenProgress: (() => void) | undefined;
    void listen<string>(repositoryChangedEvent, () => {
      setProgress(null);
      void Promise.all([loadRepositories(), loadFilterOptions(), loadScripts()]);
    }).then((dispose) => {
      unlistenChanged = dispose;
    });
    void listen<RepositoryRefreshProgress>(
      repositoryProgressEvent,
      ({ payload }) => setProgress(payload),
    ).then((dispose) => {
      unlistenProgress = dispose;
    });
    return () => {
      unlistenChanged?.();
      unlistenProgress?.();
    };
  }, [loadFilterOptions, loadRepositories, loadScripts]);

  useEffect(() => {
    setPage(0);
  }, [filters, search, sort]);

  useEffect(() => {
    setFilters((current) => {
      const repository = current.repository.filter((url) =>
        repositories.some((source) => source.url === url),
      );
      return repository.length === current.repository.length
        ? current
        : { ...current, repository };
    });
  }, [repositories]);

  useEffect(() => {
    setFilters((current) => {
      const permissions = current.permissions.filter((value) =>
        filterOptions.permissions.includes(value),
      );
      const capabilities = current.capabilities.filter((value) =>
        filterOptions.capabilities.includes(value),
      );
      if (
        permissions.length === current.permissions.length &&
        capabilities.length === current.capabilities.length
      ) {
        return current;
      }
      return { ...current, capabilities, permissions };
    });
  }, [filterOptions]);

  async function runRepositoryAction(
    id: string,
    action: (requestId: string) => Promise<unknown>,
    success: string,
  ): Promise<boolean> {
    const requestId = crypto.randomUUID();
    setRepositoryBusy((current) => new Set(current).add(id));
    try {
      await action(requestId);
      await Promise.all([
        loadRepositories(),
        loadFilterOptions(),
        loadScripts(),
      ]);
      toast.success(success);
      return true;
    } catch (error) {
      await Promise.all([
        loadRepositories(),
        loadFilterOptions(),
        loadScripts(),
      ]);
      toast.error(String(error));
      return false;
    } finally {
      setRepositoryBusy((current) => {
        const next = new Set(current);
        next.delete(id);
        return next;
      });
      setProgress(null);
    }
  }

  async function previewRepository() {
    const url = repositoryUrl.trim();
    if (!url) return;
    const requestId = crypto.randomUUID();
    setRepositoryBusy((current) => new Set(current).add("preview"));
    try {
      setRepositoryPreview(
        await previewScriptRepository(requestId, url),
      );
    } catch (error) {
      toast.error(`Could not preview the repository: ${String(error)}`);
    } finally {
      setRepositoryBusy((current) => {
        const next = new Set(current);
        next.delete("preview");
        return next;
      });
      setProgress(null);
    }
  }

  function addRepository() {
    if (!repositoryPreview) return;
    void runRepositoryAction(
      "add",
      (requestId) =>
        addScriptRepository(
          requestId,
          repositoryPreview.url,
        ),
      "Repository added.",
    ).then((succeeded) => {
      if (!succeeded) return;
      setRepositoryUrl("");
      setRepositoryPreview(null);
      setAddOpen(false);
    });
  }

  function refreshAll() {
    void runRepositoryAction(
      "refresh-all",
      async (requestId) => {
        const result = await refreshAllScriptRepositories(requestId);
        if (result.failures.length > 0) {
          throw new Error(
            `${result.failures.length} repositories could not be refreshed. Their previous cached scripts remain available.`,
          );
        }
      },
      "Repositories refreshed.",
    );
  }

  async function reviewScript(script: RepositoryScriptSummary) {
    const id = `package:${script.repository_url}:${script.script_id}`;
    setRepositoryBusy((current) => new Set(current).add(id));
    try {
      const review = await prepareRepositoryScript(
        script.repository_url,
        script.script_id,
        crypto.randomUUID(),
      );
      setPreparedReview(review);
      setPackageDialogOpen(true);
    } catch (error) {
      const message = String(error);
      toast.error(`Could not prepare the package: ${message}`);
      if (message.includes("does not match the downloaded package")) {
        setMismatchIncident({ message, script });
      }
      await Promise.all([loadRepositories(), loadScripts()]);
    } finally {
      setRepositoryBusy((current) => {
        const next = new Set(current);
        next.delete(id);
        return next;
      });
    }
  }

  async function openScriptDetails(script: RepositoryScriptSummary) {
    const id = `details:${script.repository_url}:${script.script_id}`;
    setRepositoryBusy((current) => new Set(current).add(id));
    try {
      setDetailScript(
        await getRepositoryScript(script.repository_url, script.script_id),
      );
    } catch (error) {
      toast.error(`Could not load the script details: ${String(error)}`);
    } finally {
      setRepositoryBusy((current) => {
        const next = new Set(current);
        next.delete(id);
        return next;
      });
    }
  }

  const installedById = useMemo(
    () =>
      new Map(
        dashboard.runner.scripts.map((script) => [
          script.installed.id,
          script,
        ]),
      ),
    [dashboard.runner.scripts],
  );
  const pageCount = Math.max(1, Math.ceil(total / pageSize));
  const activeFilterCount = [
    filters.repository.length > 0,
    filters.risk.length > 0,
    filters.installed.length > 0,
    filters.target.length > 0,
    filters.permissions.length > 0,
    filters.capabilities.length > 0,
  ].filter(Boolean).length;
  const progressRepository = progress
    ? repositories.find((source) => source.url === progress.repository_url)
    : undefined;

  return (
    <div className="grid gap-4">
      <Card>
        <CardHeader>
          <div className="flex flex-wrap items-center justify-between gap-3">
            <div>
              <CardTitle>Available scripts</CardTitle>
              <p className="mt-1 text-xs text-muted-foreground">
                Repository security claims are previews. BaudBound validates the downloaded package before installation.
              </p>
            </div>
            <div className="flex flex-wrap items-center gap-2">
              <Badge variant="muted">{total} results</Badge>
              <Button
                onClick={() => setManagementOpen(true)}
                size="sm"
                variant="outline"
              >
                <Database />
                Repository management
              </Button>
            </div>
          </div>
          <div className="mt-3 flex flex-wrap gap-2">
            <Input
              className="h-9 min-w-64 flex-1 py-0"
              onChange={(event) => setSearch(event.target.value)}
              placeholder="Search name, summary, author, tags, or description"
              value={search}
            />
            <div className="w-full sm:w-48">
              <FilterSelect
                label="Sort scripts"
                onValueChange={(value) =>
                  setSort(value as RepositoryScriptQuery["sort"])
                }
                value={sort}
                values={[
                  ["name", "Sort: Name"],
                  ["author", "Sort: Author"],
                  ["published", "Sort: Published"],
                  ["version", "Sort: Version"],
                  ["risk", "Sort: Risk"],
                  ["repository", "Sort: Repository"],
                ]}
              />
            </div>
            <Button
              onClick={() => {
                setFilterDraft(filters);
                setFiltersOpen(true);
              }}
              variant="outline"
            >
              <ListFilter />
              Filters{activeFilterCount > 0 ? ` (${activeFilterCount})` : ""}
            </Button>
          </div>
        </CardHeader>
        <CardContent className="overflow-x-auto p-0 max-[1100px]:p-3">
          {loading ? (
            <EmptyState>Loading repository scripts...</EmptyState>
          ) : scripts.length === 0 ? (
            <EmptyState>No scripts match the current filters.</EmptyState>
          ) : (
            <table className="responsive-table w-full border-collapse text-sm">
              <thead>
                <tr className="border-b border-border text-left text-xs uppercase text-muted-foreground">
                  <th className="px-4 py-2">Script</th>
                  <th className="px-3 py-2">Repository</th>
                  <th className="px-3 py-2">Target runtimes</th>
                  <th className="px-3 py-2">Risk</th>
                  <th className="px-3 py-2">Version</th>
                  <th className="px-4 py-2 text-right">Actions</th>
                </tr>
              </thead>
              <tbody>
                {scripts.map((script) => {
                  const installedScript = installedById.get(script.script_id);
                  const installedMetadata = installedScript?.metadata;
                  const installed = script.installed || Boolean(installedScript);
                  const installedFromThisRepository =
                    !installedMetadata ||
                    installedMetadata.repository_url === script.repository_url;
                  const targetCompatible = script.target_runtimes.some((runtime) =>
                    dashboard.runner.supported_target_runtimes.includes(runtime),
                  );
                  const versionCompatible = meetsMinimumRunnerVersion(
                    dashboard.runner.runner_version,
                    script.minimum_runner_version,
                  );
                  const compatible = targetCompatible && versionCompatible;
                  const versionComparison = installedMetadata
                    ? compareSemanticVersions(script.version, installedMetadata.version)
                    : 1;
                  const updateAvailable =
                    installed &&
                    Boolean(installedMetadata) &&
                    installedFromThisRepository &&
                    versionComparison > 0;
                  const mismatchNeedsRefresh =
                    Boolean(script.information_mismatch) &&
                    script.information_mismatch_refresh_required;
                  const canReview =
                    !mismatchNeedsRefresh &&
                    compatible &&
                    (!installed ||
                      updateAvailable ||
                      Boolean(script.information_mismatch));
                  const state = repositoryScriptState({
                    compatible,
                    informationMismatch: Boolean(script.information_mismatch),
                    installed,
                    installedFromThisRepository,
                    updateAvailable,
                  });
                  const packageBusy = repositoryBusy.has(
                    `package:${script.repository_url}:${script.script_id}`,
                  );
                  const detailsBusy = repositoryBusy.has(
                    `details:${script.repository_url}:${script.script_id}`,
                  );
                  return (
                    <tr
                      className="border-b border-border last:border-b-0"
                      key={`${script.repository_url}:${script.script_id}`}
                    >
                      <td className="px-4 py-3" data-label="Script">
                        <div className="font-medium">{script.name}</div>
                        <div className="mt-1 max-w-xl text-xs text-muted-foreground">
                          {script.summary}
                        </div>
                      </td>
                      <td className="px-3 py-3" data-label="Repository">
                        <div className="flex flex-wrap items-center gap-2">
                          <span>{script.repository_name}</span>
                          {script.official ? <Badge variant="good">Official</Badge> : null}
                        </div>
                      </td>
                      <td className="px-3 py-3" data-label="Target runtimes">
                        {script.target_runtimes.join(", ")}
                      </td>
                      <td className="px-3 py-3" data-label="Risk">
                        <RiskBadge risk={script.risk_level} />
                      </td>
                      <td className="px-3 py-3" data-label="Version">
                        <div>{script.version}</div>
                        <RepositoryStateBadge className="mt-1" state={state} />
                      </td>
                      <td className="px-4 py-3" data-label="Actions">
                        <div className="flex justify-end gap-2 max-[1280px]:justify-start">
                          <Button
                            aria-label={`View ${script.name}`}
                            disabled={detailsBusy}
                            onClick={() => void openScriptDetails(script)}
                            className="size-8 p-0"
                            size="sm"
                            title="View details"
                            variant="outline"
                          >
                            <Eye />
                          </Button>
                          <Button
                            disabled={!canReview || packageBusy}
                            onClick={() => void reviewScript(script)}
                            size="sm"
                            title={
                              mismatchNeedsRefresh
                                ? "The repository information did not match the downloaded package. Refresh the repository before trying again."
                                : script.information_mismatch
                                  ? "Download the package again and verify it against the refreshed repository information."
                                : !targetCompatible
                                  ? "This script does not support a target runtime available on this runner."
                                  : !versionCompatible
                                    ? `This script requires BaudBound ${script.minimum_runner_version} or newer.`
                                  : !canReview && installed
                                    ? "This version is already installed or belongs to another repository."
                                    : undefined
                            }
                          >
                            {script.information_mismatch
                              ? mismatchNeedsRefresh
                                ? "Refresh required"
                                : "Verify package"
                              : updateAvailable
                              ? "Review update"
                              : installed
                                ? "Installed"
                                : "Install"}
                          </Button>
                        </div>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          )}
          <div className="flex flex-wrap items-center justify-between gap-3 border-t border-border px-4 py-3 text-xs text-muted-foreground">
            <span>
              {total === 0 ? 0 : page * pageSize + 1} to{" "}
              {Math.min(total, (page + 1) * pageSize)} of {total}
            </span>
            <div className="flex gap-2">
              <Button
                disabled={page === 0}
                onClick={() => setPage((value) => Math.max(0, value - 1))}
                size="sm"
                variant="outline"
              >
                Previous
              </Button>
              <Button
                disabled={page + 1 >= pageCount}
                onClick={() => setPage((value) => value + 1)}
                size="sm"
                variant="outline"
              >
                Next
              </Button>
            </div>
          </div>
        </CardContent>
      </Card>

      <Dialog onOpenChange={setManagementOpen} open={managementOpen}>
        <DialogContent className="grid max-h-[85vh] w-[min(calc(100vw-2rem),960px)] grid-rows-[auto_minmax(0,1fr)_auto] overflow-hidden">
          <DialogHeader>
            <DialogTitle>Repository management</DialogTitle>
            <DialogDescription>
              Manage the script repositories available in the browser.
            </DialogDescription>
          </DialogHeader>
          <div className="grid content-start gap-2 overflow-y-auto pr-1">
            {repositories.length === 0 ? (
              <EmptyState>No script repositories are configured.</EmptyState>
            ) : (
              repositories.map((source) => {
                const displayName = repositoryDisplayName(source);
                return (
                  <div
                    className="grid items-center gap-3 rounded-md border border-border bg-background px-3 py-2 text-sm sm:grid-cols-[auto_minmax(0,1fr)_auto]"
                    key={source.url}
                  >
                    <Switch
                      aria-label={`Enable ${displayName}`}
                      checked={source.enabled}
                      disabled={repositoryBusy.has(source.url)}
                      onCheckedChange={(enabled) =>
                        void runRepositoryAction(
                          source.url,
                          () => setScriptRepositoryEnabled(source.url, enabled),
                          enabled
                            ? "Repository enabled."
                            : "Repository disabled.",
                        )
                      }
                      size="sm"
                    />
                    <div className="min-w-0">
                      <div className="flex min-w-0 flex-wrap items-center gap-1.5">
                        <span className="truncate font-medium">{displayName}</span>
                        {source.official ? (
                          <Badge variant="good">Official</Badge>
                        ) : null}
                        <Badge variant="muted">
                          {source.script_count} scripts
                        </Badge>
                        {source.information_mismatch_count > 0 ? (
                          <Badge variant="destructive">
                            Information mismatch
                          </Badge>
                        ) : null}
                      </div>
                      <div className="mt-1 flex min-w-0 flex-wrap gap-x-3 gap-y-1 text-xs text-muted-foreground">
                        <span
                          className="min-w-0 truncate font-mono"
                          title={repositoryUrlForDisplay(source.url)}
                        >
                          {repositoryUrlForDisplay(source.url)}
                        </span>
                        <span className="shrink-0">
                          {source.last_success_at_unix
                            ? `Refreshed ${formatUnixSeconds(source.last_success_at_unix)}`
                            : "Not refreshed yet"}
                        </span>
                      </div>
                      {source.last_error ? (
                        <p
                          className="mt-1 truncate text-xs text-baud-amber"
                          title={source.last_error}
                        >
                          Refresh failed. Cached data is still available. {source.last_error}
                        </p>
                      ) : null}
                    </div>
                    <div className="flex items-center justify-end gap-2">
                      <Button
                        aria-label={`Refresh ${displayName}`}
                        className="size-8 p-0"
                        disabled={repositoryBusy.has(source.url)}
                        onClick={() =>
                          void runRepositoryAction(
                            source.url,
                            (requestId) =>
                              refreshScriptRepository(requestId, source.url),
                            "Repository refreshed.",
                          )
                        }
                        size="sm"
                        title="Refresh repository"
                        variant="outline"
                      >
                        <RefreshCw
                          className={
                            repositoryBusy.has(source.url) ? "animate-spin" : ""
                          }
                        />
                      </Button>
                      {!source.official ? (
                        <Button
                          aria-label={`Remove ${displayName}`}
                          className="size-8 p-0 text-destructive"
                          onClick={() => setRemoveSource(source)}
                          size="sm"
                          title="Remove repository"
                          variant="outline"
                        >
                          <Trash2 />
                        </Button>
                      ) : null}
                    </div>
                  </div>
                );
              })
            )}
            {progress ? (
              <div className="flex flex-wrap items-center justify-between gap-2 rounded-md border border-border bg-background px-3 py-2 text-xs text-muted-foreground">
                <span>
                  {refreshStageLabel(progress.stage)}{" "}
                  {progressRepository
                    ? repositoryDisplayName(progressRepository)
                    : "repository"}
                </span>
                {!["startup", "automatic"].includes(progress.request_id) ? (
                  <Button
                    onClick={() =>
                      void cancelRepositoryRequest(progress.request_id)
                    }
                    size="sm"
                    variant="outline"
                  >
                    Cancel
                  </Button>
                ) : null}
              </div>
            ) : null}
          </div>
          <DialogFooter>
            <Button
              disabled={repositoryBusy.has("refresh-all")}
              onClick={refreshAll}
              variant="outline"
            >
              <RefreshCw
                className={
                  repositoryBusy.has("refresh-all") ? "animate-spin" : ""
                }
              />
              Refresh all
            </Button>
            <Button onClick={() => setAddOpen(true)}>
              <Plus />
              Add repository
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog onOpenChange={setFiltersOpen} open={filtersOpen}>
        <DialogContent className="sm:max-w-2xl">
          <DialogHeader>
            <DialogTitle>Script filters</DialogTitle>
            <DialogDescription>
              Choose which repository scripts appear in the browser.
            </DialogDescription>
          </DialogHeader>
          <div className="grid gap-4 sm:grid-cols-2">
            <FilterField label="Repository">
              <MultiSelect
                onChange={(repository) =>
                  setFilterDraft((current) => ({ ...current, repository }))
                }
                options={repositories.map((source) => ({
                  label: repositoryDisplayName(source),
                  value: source.url,
                }))}
                placeholder="All repositories"
                value={filterDraft.repository}
              />
            </FilterField>
            <FilterField label="Risk">
              <MultiSelect
                onChange={(risk) =>
                  setFilterDraft((current) => ({ ...current, risk }))
                }
                options={[
                  { label: "Low risk", value: "low" },
                  { label: "Medium risk", value: "medium" },
                  { label: "High risk", value: "high" },
                  { label: "Dangerous", value: "dangerous" },
                ]}
                placeholder="All risks"
                value={filterDraft.risk}
              />
            </FilterField>
            <FilterField label="Installation state">
              <MultiSelect
                onChange={(installed) =>
                  setFilterDraft((current) => ({ ...current, installed }))
                }
                options={[
                  { label: "Installed", value: "installed" },
                  { label: "Not installed", value: "not_installed" },
                ]}
                placeholder="Any state"
                value={filterDraft.installed}
              />
            </FilterField>
            <FilterField label="Target runtime">
              <MultiSelect
                onChange={(target) =>
                  setFilterDraft((current) => ({ ...current, target }))
                }
                options={dashboard.runner.supported_target_runtimes.map(
                  (runtime) => ({ label: runtime, value: runtime }),
                )}
                placeholder="All targets"
                value={filterDraft.target}
              />
            </FilterField>
            <FilterField label="Permission">
              <MultiSelect
                onChange={(permissions) =>
                  setFilterDraft((current) => ({
                    ...current,
                    permissions,
                  }))
                }
                options={filterOptions.permissions.map((permission) => ({
                  label: permission,
                  value: permission,
                }))}
                placeholder="All permissions"
                value={filterDraft.permissions}
              />
            </FilterField>
            <FilterField label="Capability">
              <MultiSelect
                onChange={(capabilities) =>
                  setFilterDraft((current) => ({
                    ...current,
                    capabilities,
                  }))
                }
                options={filterOptions.capabilities.map((capability) => ({
                  label: capability,
                  value: capability,
                }))}
                placeholder="All capabilities"
                value={filterDraft.capabilities}
              />
            </FilterField>
          </div>
          <DialogFooter>
            <Button
              onClick={() => setFilterDraft(defaultFilters)}
              variant="outline"
            >
              Clear filters
            </Button>
            <Button onClick={() => setFiltersOpen(false)} variant="outline">
              Cancel
            </Button>
            <Button
              onClick={() => {
                setFilters(filterDraft);
                setFiltersOpen(false);
              }}
            >
              Apply filters
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog
        onOpenChange={(open) => {
          setAddOpen(open);
          if (!open) setRepositoryPreview(null);
        }}
        open={addOpen}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Add script repository</DialogTitle>
            <DialogDescription>
              Enter a public HTTPS address ending in repository.json. Adding it contacts that server and stores the validated script list locally.
            </DialogDescription>
          </DialogHeader>
          <Input
            autoCapitalize="none"
            autoComplete="off"
            disabled={repositoryPreview !== null}
            onChange={(event) => {
              setRepositoryUrl(event.target.value);
              setRepositoryPreview(null);
            }}
            placeholder="https://example.com/repository.json"
            value={repositoryUrl}
          />
          {repositoryPreview ? (
            <div className="grid gap-2 rounded-md border border-border bg-background p-3 text-sm">
              <div className="flex flex-wrap items-center gap-2">
                <span className="font-medium">{repositoryPreview.name}</span>
                <Badge variant="muted">
                  {repositoryPreview.script_count} scripts
                </Badge>
              </div>
              {repositoryPreview.description ? (
                <p className="text-muted-foreground">
                  {repositoryPreview.description}
                </p>
              ) : null}
              <p className="break-all font-mono text-xs text-muted-foreground">
                {repositoryUrlForDisplay(repositoryPreview.url)}
              </p>
              <p className="text-xs text-baud-amber">
                BaudBound downloads and validates the repository again before saving it.
              </p>
            </div>
          ) : null}
          <DialogFooter>
            {repositoryPreview ? (
              <Button onClick={() => setRepositoryPreview(null)} variant="outline">
                Back
              </Button>
            ) : (
              <Button onClick={() => setAddOpen(false)} variant="outline">
                Cancel
              </Button>
            )}
            <Button
              disabled={
                !repositoryUrl.trim() ||
                repositoryBusy.has("add") ||
                repositoryBusy.has("preview")
              }
              onClick={
                repositoryPreview
                  ? addRepository
                  : () => void previewRepository()
              }
            >
              {repositoryPreview ? "Add repository" : "Preview repository"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog
        onOpenChange={(open) => {
          if (!open) setRemoveSource(null);
        }}
        open={removeSource !== null}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Remove repository?</DialogTitle>
            <DialogDescription>
              Cached browser entries from this repository will be removed. Scripts already installed from it will remain installed, but their update source will be unavailable.
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button onClick={() => setRemoveSource(null)} variant="outline">
              Keep repository
            </Button>
            <Button
              onClick={() => {
                if (!removeSource) return;
                const source = removeSource;
                setRemoveSource(null);
                void runRepositoryAction(
                  source.url,
                  () => removeScriptRepository(source.url),
                  "Repository removed.",
                );
              }}
              variant="destructive"
            >
              Remove repository
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog
        onOpenChange={(open) => {
          if (!open) setMismatchIncident(null);
        }}
        open={mismatchIncident !== null}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Repository information mismatch</DialogTitle>
            <DialogDescription>
              The repository reported information that does not match the validated package.
              This can be caused by stale repository data, a publisher mistake, or tampering.
              BaudBound rejected the package and did not install or update it.
            </DialogDescription>
          </DialogHeader>
          {mismatchIncident ? (
            <div className="grid gap-2 rounded-md border border-destructive/40 bg-destructive/10 p-3 text-sm">
              <div className="font-medium">{mismatchIncident.script.name}</div>
              <div className="text-muted-foreground">
                Repository: {mismatchIncident.script.repository_name}
              </div>
              <div className="break-words font-mono text-xs text-destructive">
                {mismatchIncident.message}
              </div>
            </div>
          ) : null}
          <DialogFooter>
            <Button onClick={() => setMismatchIncident(null)} variant="outline">
              Keep repository
            </Button>
            {mismatchIncident && !mismatchIncident.script.official ? (
              <Button
                onClick={() => {
                  const source = repositories.find(
                    (repository) =>
                      repository.url === mismatchIncident.script.repository_url,
                  );
                  setMismatchIncident(null);
                  if (source) setRemoveSource(source);
                }}
                variant="destructive"
              >
                Remove repository
              </Button>
            ) : null}
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <DetailDialog
        description={
          detailScript
            ? `${detailScript.repository_name} | ${detailScript.script_id}`
            : "Repository script information"
        }
        onOpenChange={(open) => {
          if (!open) setDetailScript(null);
        }}
        open={detailScript !== null}
        title={detailScript?.name ?? "Script details"}
      >
        {detailScript ? <RepositoryScriptDetails script={detailScript} /> : null}
      </DetailDialog>

      <RemotePackageDialog
        busyActions={busyActions}
        onInstalled={() => void loadScripts()}
        onOpenChange={(open) => {
          setPackageDialogOpen(open);
          if (!open) setPreparedReview(undefined);
        }}
        open={packageDialogOpen}
        operation={preparedReview?.operation ?? "import"}
        preparedReview={preparedReview}
        runAction={runAction}
      />
    </div>
  );
}

function RepositoryScriptDetails({
  script,
}: {
  script: RepositoryScriptRecord;
}) {
  const entry = script.entry;
  return (
    <div className="grid gap-4">
      <div className="grid gap-3 md:grid-cols-3">
        <InfoCard label="Repository" value={script.repository_name} />
        <InfoCard label="Version" value={entry.latest.version} />
        <InfoCard label="Target runtimes" value={entry.target_runtimes.join(", ")} />
        <InfoCard label="Minimum runner" value={entry.minimum_runner_version} />
        <InfoCard label="Risk" value={entry.risk_level} />
        <InfoCard label="Published" value={entry.latest.published_at} />
        <InfoCard label="Author" value={entry.author || "Not provided"} />
        <InfoCard label="License" value={entry.license || "Not provided"} />
      </div>
      {entry.tags.length > 0 ? (
        <Card>
          <CardHeader>
            <CardTitle>Tags</CardTitle>
          </CardHeader>
          <CardContent className="flex flex-wrap gap-2">
            {entry.tags.map((tag) => (
              <Badge key={tag} variant="muted">
                {tag}
              </Badge>
            ))}
          </CardContent>
        </Card>
      ) : null}
      <Card>
        <CardHeader>
          <CardTitle>Description</CardTitle>
        </CardHeader>
        <CardContent>
          <LazyMarkdownContent source={entry.description || entry.summary} />
        </CardContent>
      </Card>
      <div className="grid gap-4 md:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle>Permissions</CardTitle>
          </CardHeader>
          <CardContent className="flex flex-wrap gap-2">
            {entry.permissions.map((permission) => (
              <Badge key={permission} variant="muted">
                {permission}
              </Badge>
            ))}
          </CardContent>
        </Card>
        <Card>
          <CardHeader>
            <CardTitle>Capabilities</CardTitle>
          </CardHeader>
          <CardContent className="flex flex-wrap gap-2">
            {entry.capabilities.map((capability) => (
              <Badge key={capability} variant="muted">
                {capability}
              </Badge>
            ))}
          </CardContent>
        </Card>
      </div>
      <Card>
        <CardHeader>
          <CardTitle>Release notes</CardTitle>
        </CardHeader>
        <CardContent>
          <LazyMarkdownContent source={entry.latest.release_notes || "No release notes were provided."} />
        </CardContent>
      </Card>
      <div className="flex flex-wrap gap-2">
        {entry.website ? (
          <ExternalLink className="text-sm" href={entry.website}>
            Website
          </ExternalLink>
        ) : null}
        {entry.source ? (
          <ExternalLink className="text-sm" href={entry.source}>
            Source
          </ExternalLink>
        ) : null}
      </div>
      <div className="flex items-start gap-2 rounded-md border border-baud-amber/30 bg-baud-amber/10 p-3 text-sm text-baud-amber">
        <CircleAlert className="mt-0.5 size-4 shrink-0" />
        Repository permissions, capabilities, and risk are unverified previews. The downloaded package is validated separately before installation.
      </div>
    </div>
  );
}

function InfoCard({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-md border border-border bg-background p-3">
      <div className="text-xs text-muted-foreground">{label}</div>
      <div className="mt-1 break-words text-sm">{value}</div>
    </div>
  );
}

function FilterSelect({
  label,
  onValueChange,
  value,
  values,
}: {
  label: string;
  onValueChange: (value: string) => void;
  value: string;
  values: string[][];
}) {
  return (
    <Select onValueChange={onValueChange} value={value}>
      <SelectTrigger aria-label={label} className="h-9 py-0">
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        {values.map(([itemValue, label]) => (
          <SelectItem key={itemValue} value={itemValue}>
            {label}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}

function FilterField({
  children,
  label,
}: {
  children: React.ReactNode;
  label: string;
}) {
  return (
    <div className="grid gap-1.5">
      <span className="text-xs font-medium text-muted-foreground">{label}</span>
      {children}
    </div>
  );
}

function RepositoryStateBadge({
  className,
  state,
}: {
  className?: string;
  state: RepositoryScriptState;
}) {
  if (state === "unavailable") {
    return <Badge className={className} variant="destructive">Unavailable</Badge>;
  }
  if (state === "incompatible") {
    return <Badge className={className} variant="medium">Incompatible</Badge>;
  }
  if (state === "update_available") {
    return <Badge className={className} variant="medium">Update available</Badge>;
  }
  if (state === "installed_elsewhere") {
    return <Badge className={className} variant="muted">Installed from another repository</Badge>;
  }
  return null;
}

function RiskBadge({ risk }: { risk: string }) {
  const variant =
    risk === "low"
      ? "good"
      : risk === "medium"
        ? "medium"
        : "destructive";
  return <Badge variant={variant}>{risk}</Badge>;
}

function refreshStageLabel(stage: RepositoryRefreshProgress["stage"]) {
  if (stage === "downloading") return "Downloading";
  if (stage === "validating") return "Validating";
  if (stage === "replacing_cache") return "Updating cache for";
  return "Refreshed";
}
