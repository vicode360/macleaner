import { invoke } from "@tauri-apps/api/core";
import {
  Box,
  CheckCircle2,
  ChevronDown,
  ChevronRight,
  FolderGit2,
  FolderOpen,
  FolderPlus,
  Hammer,
  Package,
  RefreshCw,
  ScanSearch,
  ShieldAlert,
  Terminal,
  Trash2,
  Wrench,
  X,
  createElement,
  type IconNode,
} from "lucide";
import { createDustBar } from "./dust-bar";
import { formatBytes, formatDays, formatInt, getCopy } from "./i18n";
import type {
  AppLanguage,
  BrewCleanAction,
  BrewCleanResult,
  DevProject,
  DevProjectsReport,
  DevTrashResult,
  DockerPruneResult,
  DockerPruneScope,
  DockerReport,
  ToolchainsReport,
  XcodeActionResult,
  XcodeReport,
} from "./types";

export type DevSubTab = "projects" | "xcode" | "docker" | "tools";
type AgeFilter = "all" | 30 | 90;

export interface DevConfirmRequest {
  isPermanent: boolean;
  getLabel: () => string;
  getActionLabel: (inProgress: boolean) => string;
  onConfirm: () => Promise<void>;
}

export interface CleanedPathItem {
  path: string;
  freedBytes: number;
}

export interface DevPanelDeps {
  container: HTMLElement;
  getLanguage: () => AppLanguage;
  isGlobalBusy: () => boolean;
  requestConfirm: (req: DevConfirmRequest) => void;
  clearConfirm: () => void;
  showToast: (message: string) => void;
  revealInFinder: (path: string) => Promise<void>;
  onStateChange: () => void;
  onCleaned: (trashedPaths: string[], cleanedItems?: CleanedPathItem[]) => Promise<void>;
}

export interface DevPanelController {
  activate: () => void;
  render: () => void;
  isBusy: () => boolean;
  isScanning: () => boolean;
}

interface DevPanelState {
  activeSubTab: DevSubTab;
  ageFilter: AgeFilter;
  roots: string[];
  projectsReport: DevProjectsReport | null;
  xcodeReport: XcodeReport | null;
  dockerReport: DockerReport | null;
  toolsReport: ToolchainsReport | null;
  loaded: Record<DevSubTab, boolean>;
  loading: Record<DevSubTab, boolean>;
  errors: Record<DevSubTab, string | null>;
  mutating: boolean;
  pickingRoot: boolean;
  brewPreviewExpanded: boolean;
}

function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  className?: string,
  text?: string,
): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  if (className) {
    node.className = className;
  }
  if (text !== undefined) {
    node.textContent = text;
  }
  return node;
}

function icon(iconNode: IconNode, size = 13): SVGElement {
  return createElement(iconNode, {
    width: size,
    height: size,
    "stroke-width": 1.75,
    "aria-hidden": "true",
  });
}

function shortenHomePath(rawPath: string): string {
  return rawPath.replace(/^\/Users\/[^/]+/, "~");
}

function basenameOf(rawPath: string): string {
  return rawPath.split("/").filter(Boolean).pop() ?? rawPath;
}

function normalizeMacPath(rawPath: string): string {
  return rawPath.replace(/\/+$/, "").toLowerCase();
}

export function createDevPanel(deps: DevPanelDeps): DevPanelController {
  const devState: DevPanelState = {
    activeSubTab: "projects",
    ageFilter: "all",
    roots: [],
    projectsReport: null,
    xcodeReport: null,
    dockerReport: null,
    toolsReport: null,
    loaded: { projects: false, xcode: false, docker: false, tools: false },
    loading: { projects: false, xcode: false, docker: false, tools: false },
    errors: { projects: null, xcode: null, docker: null, tools: null },
    mutating: false,
    pickingRoot: false,
    brewPreviewExpanded: false,
  };

  let bodyEl: HTMLElement | null = null;
  let lastRenderedSubTab: DevSubTab = devState.activeSubTab;

  function isBusy(): boolean {
    return devState.mutating || devState.pickingRoot;
  }

  function isScanning(): boolean {
    return (
      devState.loading.projects ||
      devState.loading.xcode ||
      devState.loading.docker ||
      devState.loading.tools
    );
  }

  function isActionDisabled(): boolean {
    return deps.isGlobalBusy() || isBusy() || devState.loading[devState.activeSubTab];
  }

  function notify(originSub?: DevSubTab): void {
    deps.onStateChange();
    if (!deps.container.hidden && (!originSub || originSub === devState.activeSubTab)) {
      render();
    }
  }

  async function loadProjects(force = false): Promise<void> {
    if (devState.loading.projects || (!force && devState.loaded.projects)) {
      return;
    }
    devState.loading.projects = true;
    devState.errors.projects = null;
    notify("projects");

    try {
      const roots = await invoke<string[]>("get_dev_project_roots");
      devState.roots = roots;
      if (roots.length === 0) {
        devState.projectsReport = {
          roots: [],
          projects: [],
          totalReclaimableBytes: 0,
          scanDurationMs: 0,
        };
      } else {
        const report = await invoke<DevProjectsReport>("scan_dev_projects");
        devState.roots = report.roots;
        devState.projectsReport = report;
      }
      devState.loaded.projects = true;
    } catch (err) {
      devState.errors.projects = String(err);
      if (force) {
        deps.showToast(getCopy(deps.getLanguage()).dev.projects.scanError(String(err)));
      }
    } finally {
      devState.loading.projects = false;
      notify("projects");
    }
  }

  async function loadXcode(force = false): Promise<void> {
    if (devState.loading.xcode || (!force && devState.loaded.xcode)) {
      return;
    }
    devState.loading.xcode = true;
    devState.errors.xcode = null;
    notify("xcode");

    try {
      devState.xcodeReport = await invoke<XcodeReport>("inspect_xcode");
      devState.loaded.xcode = true;
    } catch (err) {
      devState.errors.xcode = String(err);
      if (force) {
        deps.showToast(getCopy(deps.getLanguage()).dev.xcode.actionError(String(err)));
      }
    } finally {
      devState.loading.xcode = false;
      notify("xcode");
    }
  }

  async function loadDocker(force = false): Promise<void> {
    if (devState.loading.docker || (!force && devState.loaded.docker)) {
      return;
    }
    devState.loading.docker = true;
    devState.errors.docker = null;
    notify("docker");

    try {
      devState.dockerReport = await invoke<DockerReport>("inspect_docker_containers");
      devState.loaded.docker = true;
    } catch (err) {
      devState.errors.docker = String(err);
      if (force) {
        deps.showToast(getCopy(deps.getLanguage()).dev.loadError(String(err)));
      }
    } finally {
      devState.loading.docker = false;
      notify("docker");
    }
  }

  async function loadTools(force = false): Promise<void> {
    if (devState.loading.tools || (!force && devState.loaded.tools)) {
      return;
    }
    devState.loading.tools = true;
    devState.errors.tools = null;
    notify("tools");

    try {
      devState.toolsReport = await invoke<ToolchainsReport>("inspect_dev_toolchains");
      devState.loaded.tools = true;
    } catch (err) {
      devState.errors.tools = String(err);
      if (force) {
        deps.showToast(getCopy(deps.getLanguage()).dev.loadError(String(err)));
      }
    } finally {
      devState.loading.tools = false;
      notify("tools");
    }
  }

  function ensureSubTabLoaded(sub: DevSubTab, force = false): void {
    if (sub === "projects") {
      void loadProjects(force);
    } else if (sub === "xcode") {
      void loadXcode(force);
    } else if (sub === "docker") {
      void loadDocker(force);
    } else {
      void loadTools(force);
    }
  }

  async function pickRootFolder(): Promise<void> {
    if (devState.pickingRoot || devState.mutating || devState.loading.projects) {
      return;
    }
    devState.pickingRoot = true;
    notify("projects");

    let shouldReload = false;
    try {
      const added = await invoke<string | null>("pick_dev_root_folder");
      if (added) {
        deps.showToast(getCopy(deps.getLanguage()).dev.projects.rootAdded(basenameOf(added)));
        shouldReload = true;
      }
    } catch (err) {
      deps.showToast(getCopy(deps.getLanguage()).dev.projects.rootError(String(err)));
    } finally {
      devState.pickingRoot = false;
      notify("projects");
    }
    if (shouldReload) {
      void loadProjects(true);
    }
  }

  async function removeRootFolder(path: string): Promise<void> {
    if (devState.mutating || devState.loading.projects) {
      return;
    }
    if (!deps.isGlobalBusy()) {
      deps.clearConfirm();
    }
    devState.mutating = true;
    notify("projects");

    let shouldReload = false;
    try {
      const nextRoots = await invoke<string[]>("remove_dev_project_root", { path });
      devState.roots = nextRoots;
      deps.showToast(getCopy(deps.getLanguage()).dev.projects.rootRemoved(basenameOf(path)));
      if (nextRoots.length === 0) {
        devState.projectsReport = {
          roots: [],
          projects: [],
          totalReclaimableBytes: 0,
          scanDurationMs: 0,
        };
      } else {
        shouldReload = true;
      }
    } catch (err) {
      deps.showToast(getCopy(deps.getLanguage()).dev.projects.rootError(String(err)));
    } finally {
      devState.mutating = false;
      notify("projects");
    }
    if (shouldReload) {
      void loadProjects(true);
    }
  }

  function reconcileProjectsAfterTrash(trashedPaths: string[]): void {
    if (!devState.projectsReport || trashedPaths.length === 0) {
      return;
    }
    const removed = new Set(trashedPaths.map(normalizeMacPath));
    const nextProjects: DevProject[] = [];
    for (const project of devState.projectsReport.projects) {
      const remainingArtifacts = project.artifacts.filter(
        (a) => !removed.has(normalizeMacPath(a.path)),
      );
      if (remainingArtifacts.length === 0) {
        continue;
      }
      const totalArtifactBytes = remainingArtifacts.reduce((sum, a) => sum + a.sizeBytes, 0);
      nextProjects.push({
        ...project,
        artifacts: remainingArtifacts,
        totalArtifactBytes,
      });
    }
    nextProjects.sort(
      (a, b) => b.totalArtifactBytes - a.totalArtifactBytes || a.name.localeCompare(b.name),
    );
    devState.projectsReport = {
      ...devState.projectsReport,
      projects: nextProjects,
      totalReclaimableBytes: nextProjects.reduce((sum, p) => sum + p.totalArtifactBytes, 0),
    };
  }

  async function executeTrashProjectArtifacts(paths: string[]): Promise<void> {
    if (devState.mutating || paths.length === 0) {
      return;
    }
    devState.mutating = true;
    notify("projects");

    try {
      const sizeByPath = new Map<string, number>();
      if (devState.projectsReport) {
        for (const project of devState.projectsReport.projects) {
          for (const artifact of project.artifacts) {
            sizeByPath.set(normalizeMacPath(artifact.path), artifact.sizeBytes);
          }
        }
      }

      const result = await invoke<DevTrashResult>("trash_dev_project_artifacts", { paths });
      const lang = deps.getLanguage();
      const toasts = getCopy(lang).toasts;

      if (result.trashedPaths.length > 0) {
        const cleanedItems: CleanedPathItem[] = result.trashedPaths.map((p) => ({
          path: p,
          freedBytes: sizeByPath.get(normalizeMacPath(p)) ?? 0,
        }));
        reconcileProjectsAfterTrash(result.trashedPaths);
        await deps.onCleaned(result.trashedPaths, cleanedItems);
      }

      if (result.failed.length > 0 && result.trashedPaths.length === 0) {
        deps.showToast(toasts.trashNoneMoved(result.failed[0].reason));
      } else if (result.failed.length > 0) {
        deps.showToast(toasts.trashPartial(result.trashedPaths.length, result.failed.length));
      } else {
        deps.showToast(toasts.trashMoved(formatBytes(result.freedBytes, lang)));
      }
    } catch (err) {
      deps.showToast(getCopy(deps.getLanguage()).toasts.trashError(String(err)));
    } finally {
      devState.mutating = false;
      notify("projects");
    }
  }

  async function executeTrashXcodeArea(path: string): Promise<void> {
    if (devState.mutating) {
      return;
    }
    devState.mutating = true;
    notify("xcode");

    let shouldReload = false;
    try {
      const result = await invoke<XcodeActionResult>("trash_xcode_items", { paths: [path] });
      const lang = deps.getLanguage();
      const copy = getCopy(lang);
      if (result.failed.length > 0 && result.deletedCount === 0) {
        deps.showToast(copy.toasts.trashNoneMoved(result.failed[0].reason));
      } else {
        deps.showToast(copy.toasts.trashMoved(formatBytes(result.freedBytes, lang)));
        await deps.onCleaned([path], [{ path, freedBytes: result.freedBytes }]);
        shouldReload = true;
      }
    } catch (err) {
      deps.showToast(getCopy(deps.getLanguage()).dev.xcode.actionError(String(err)));
    } finally {
      devState.mutating = false;
      notify("xcode");
    }
    if (shouldReload) {
      void loadXcode(true);
    }
  }

  async function executeDeleteUnavailableSimulators(): Promise<void> {
    if (devState.mutating) {
      return;
    }
    devState.mutating = true;
    notify("xcode");

    let shouldReload = false;
    try {
      const result = await invoke<XcodeActionResult>("delete_unavailable_simulators");
      const lang = deps.getLanguage();
      deps.showToast(
        getCopy(lang).dev.xcode.unavailableCleaned(
          result.deletedCount,
          formatBytes(result.freedBytes, lang),
        ),
      );
      await deps.onCleaned([]);
      shouldReload = true;
    } catch (err) {
      deps.showToast(getCopy(deps.getLanguage()).dev.xcode.actionError(String(err)));
    } finally {
      devState.mutating = false;
      notify("xcode");
    }
    if (shouldReload) {
      void loadXcode(true);
    }
  }

  async function executeDeleteRuntime(identifier: string, name: string): Promise<void> {
    if (devState.mutating) {
      return;
    }
    devState.mutating = true;
    notify("xcode");

    let shouldReload = false;
    try {
      const result = await invoke<XcodeActionResult>("delete_simulator_runtime", { identifier });
      const lang = deps.getLanguage();
      deps.showToast(
        getCopy(lang).dev.xcode.runtimeDeleted(name, formatBytes(result.freedBytes, lang)),
      );
      await deps.onCleaned([]);
      shouldReload = true;
    } catch (err) {
      deps.showToast(getCopy(deps.getLanguage()).dev.xcode.actionError(String(err)));
    } finally {
      devState.mutating = false;
      notify("xcode");
    }
    if (shouldReload) {
      void loadXcode(true);
    }
  }

  async function executeDockerPrune(scope: DockerPruneScope): Promise<void> {
    if (devState.mutating) {
      return;
    }
    devState.mutating = true;
    notify("docker");

    let shouldReload = false;
    try {
      const result = await invoke<DockerPruneResult>("prune_docker_scope", { scope });
      const lang = deps.getLanguage();
      deps.showToast(getCopy(lang).dev.docker.pruneSuccess(formatBytes(result.reclaimedBytes, lang)));
      await deps.onCleaned([]);
      shouldReload = true;
    } catch (err) {
      deps.showToast(getCopy(deps.getLanguage()).dev.docker.pruneError(String(err)));
    } finally {
      devState.mutating = false;
      notify("docker");
    }
    if (shouldReload) {
      void loadDocker(true);
    }
  }

  async function executeBrewClean(action: BrewCleanAction): Promise<void> {
    if (devState.mutating) {
      return;
    }
    devState.mutating = true;
    notify("tools");

    let shouldReload = false;
    try {
      const result = await invoke<BrewCleanResult>("run_homebrew_clean", { action });
      const lang = deps.getLanguage();
      const label = action === "cleanup" ? "brew cleanup" : "brew autoremove";
      deps.showToast(
        getCopy(lang).dev.tools.brewSuccess(label, formatBytes(result.freedBytes, lang)),
      );
      await deps.onCleaned([]);
      shouldReload = true;
    } catch (err) {
      deps.showToast(getCopy(deps.getLanguage()).dev.tools.brewError(String(err)));
    } finally {
      devState.mutating = false;
      notify("tools");
    }
    if (shouldReload) {
      void loadTools(true);
    }
  }

  async function executeTrashToolchainCache(path: string, name: string): Promise<void> {
    if (devState.mutating) {
      return;
    }
    devState.mutating = true;
    notify("tools");

    let shouldReload = false;
    try {
      const freed = await invoke<number>("trash_dev_toolchain_cache", { path });
      const lang = deps.getLanguage();
      deps.showToast(getCopy(lang).dev.tools.cacheTrashed(name, formatBytes(freed, lang)));
      await deps.onCleaned([path], [{ path, freedBytes: freed }]);
      shouldReload = true;
    } catch (err) {
      deps.showToast(getCopy(deps.getLanguage()).toasts.trashError(String(err)));
    } finally {
      devState.mutating = false;
      notify("tools");
    }
    if (shouldReload) {
      void loadTools(true);
    }
  }

  function renderLoadingCard(text: string): HTMLElement {
    const card = el("div", "empty-state dev-loading-state");
    const spinner = el("span", "dev-spinner");
    spinner.append(icon(RefreshCw, 20));
    card.append(spinner, el("p", "muted", text));
    return card;
  }

  function renderProjectsBody(target: HTMLElement): void {
    const lang = deps.getLanguage();
    const copy = getCopy(lang);
    const pCopy = copy.dev.projects;
    const disabled = isActionDisabled();

    const rootsPanel = el("section", "aqua-panel dev-compact-panel");
    const rootsHead = el("div", "surface-row");
    const rootsTitle = el("div", "heading-with-icon");
    rootsTitle.append(icon(FolderGit2, 13), el("h2", "section-heading", pCopy.rootsTitle));

    const rootsActions = el("div", "dev-inline-actions");
    const addBtn = el("button", "btn btn-ghost btn-xs");
    addBtn.type = "button";
    addBtn.disabled = disabled || devState.pickingRoot;
    addBtn.append(icon(FolderPlus, 11), document.createTextNode(pCopy.addFolder));
    addBtn.addEventListener("click", () => {
      void pickRootFolder();
    });

    const scanBtn = el("button", "btn btn-secondary btn-xs");
    scanBtn.type = "button";
    scanBtn.disabled = disabled || devState.roots.length === 0;
    scanBtn.append(
      icon(ScanSearch, 11),
      document.createTextNode(devState.loading.projects ? pCopy.scanning : pCopy.scan),
    );
    scanBtn.addEventListener("click", () => {
      if (!deps.isGlobalBusy() && !devState.mutating) {
        deps.clearConfirm();
      }
      void loadProjects(true);
    });

    rootsActions.append(addBtn, scanBtn);
    rootsHead.append(rootsTitle, rootsActions);
    rootsPanel.append(rootsHead);

    if (devState.roots.length > 0) {
      const chipsWrap = el("div", "dev-roots-chips");
      for (const root of devState.roots) {
        const chip = el("span", "dev-root-chip");
        const shortLabel = el("span", "dev-root-path", shortenHomePath(root));
        shortLabel.title = root;

        const rmBtn = el("button", "dev-chip-remove");
        rmBtn.type = "button";
        rmBtn.disabled = disabled;
        rmBtn.setAttribute("aria-label", pCopy.removeRootAria(basenameOf(root)));
        rmBtn.title = pCopy.removeRootAria(basenameOf(root));
        rmBtn.append(icon(X, 10));
        rmBtn.addEventListener("click", () => {
          void removeRootFolder(root);
        });

        chip.append(shortLabel, rmBtn);
        chipsWrap.append(chip);
      }
      rootsPanel.append(chipsWrap);
    } else {
      rootsPanel.append(el("p", "muted", pCopy.noRootsHint));
    }

    target.append(rootsPanel);

    if (devState.loading.projects && !devState.projectsReport) {
      target.append(renderLoadingCard(copy.dev.loading));
      return;
    }

    if (devState.errors.projects && !devState.projectsReport) {
      const errPanel = el("section", "aqua-panel");
      errPanel.append(el("p", "inline-error", copy.dev.loadError(devState.errors.projects)));
      target.append(errPanel);
      return;
    }

    if (devState.roots.length === 0) {
      const empty = el("div", "empty-state");
      empty.append(
        icon(FolderPlus, 22),
        el("h2", "section-heading", pCopy.noRootsTitle),
        el("p", "muted", pCopy.noRootsDesc),
      );
      const cta = el("button", "btn btn-primary");
      cta.type = "button";
      cta.disabled = disabled;
      cta.append(icon(FolderPlus, 13), document.createTextNode(pCopy.addFolder));
      cta.addEventListener("click", () => {
        void pickRootFolder();
      });
      empty.append(cta);
      target.append(empty);
      return;
    }

    const report = devState.projectsReport;
    if (!report || report.projects.length === 0) {
      const empty = el("div", "empty-state");
      empty.append(
        icon(CheckCircle2, 22),
        el("h2", "section-heading", pCopy.emptyTitle),
        el("p", "muted", pCopy.emptyDesc),
      );
      target.append(empty);
      return;
    }

    const filterBar = el("div", "items-toolbar");
    const pills = el("div", "filter-pills");
    const filters: Array<{ id: AgeFilter; label: string }> = [
      { id: "all", label: pCopy.filterAll },
      { id: 30, label: pCopy.filter30 },
      { id: 90, label: pCopy.filter90 },
    ];
    for (const f of filters) {
      const pill = el("button", "filter-pill", f.label);
      const active = devState.ageFilter === f.id;
      pill.type = "button";
      pill.classList.toggle("is-active", active);
      pill.setAttribute("aria-pressed", active ? "true" : "false");
      pill.addEventListener("click", () => {
        devState.ageFilter = f.id;
        render();
      });
      pills.append(pill);
    }
    filterBar.append(pills);

    const bulkThreshold = devState.ageFilter === 90 ? 90 : 30;
    const bulkProjects = report.projects.filter((p) => p.inactiveDays >= bulkThreshold);
    const bulkPaths = bulkProjects.flatMap((p) => p.artifacts.map((a) => a.path));
    const bulkBytes = bulkProjects.reduce((sum, p) => sum + p.totalArtifactBytes, 0);

    if (bulkPaths.length > 0) {
      const bulkBtn = el("button", "btn btn-secondary btn-xs tabular");
      bulkBtn.type = "button";
      bulkBtn.disabled = disabled;
      bulkBtn.append(icon(Trash2, 11), document.createTextNode(pCopy.cleanOlder(bulkThreshold)));
      bulkBtn.addEventListener("click", () => {
        deps.requestConfirm({
          isPermanent: false,
          getLabel: () =>
            getCopy(deps.getLanguage()).dev.projects.confirmTrashBulk(
              bulkPaths.length,
              formatBytes(bulkBytes, deps.getLanguage()),
              bulkThreshold,
            ),
          getActionLabel: (inProgress) => {
            const c = getCopy(deps.getLanguage());
            return inProgress ? c.items.moving : c.items.moveToTrash;
          },
          onConfirm: () => executeTrashProjectArtifacts(bulkPaths),
        });
      });
      filterBar.append(bulkBtn);
    }
    target.append(filterBar);

    const filteredProjects = report.projects.filter((p) =>
      devState.ageFilter === "all" ? true : p.inactiveDays >= devState.ageFilter,
    );

    if (filteredProjects.length === 0) {
      const empty = el("div", "empty-state");
      empty.append(
        el("h2", "section-heading", pCopy.filterEmptyTitle),
        el("p", "muted", pCopy.filterEmptyDesc),
      );
      target.append(empty);
      return;
    }

    const filteredTotalBytes = filteredProjects.reduce((sum, p) => sum + p.totalArtifactBytes, 0);
    const metaStrip = el("div", "dev-summary-strip");
    metaStrip.append(
      el(
        "span",
        "muted tabular",
        pCopy.summaryLine(
          formatInt(filteredProjects.length, lang),
          formatBytes(filteredTotalBytes, lang),
        ),
      ),
      el("span", "muted tabular", `${formatInt(report.scanDurationMs, lang)} ms`),
    );
    target.append(metaStrip);

    const maxProjectBytes = Math.max(1, ...filteredProjects.map((p) => p.totalArtifactBytes));
    const list = el("div", "dev-list");

    for (const project of filteredProjects) {
      const row = el("article", "item-row");
      const top = el("div", "item-top");

      const titleGroup = el("div", "item-title-group");
      const ecoBadge = el("span", "dev-eco-badge", project.kind);
      const nameEl = el("span", "item-name", project.name);
      nameEl.title = project.path;
      const ageBadge = el(
        "span",
        "dev-age-chip tabular",
        pCopy.inactiveShort(project.inactiveDays),
      );
      ageBadge.title = formatDays(project.inactiveDays, lang);
      titleGroup.append(ecoBadge, nameEl, ageBadge);

      const rightTop = el("div", "dev-inline-actions");
      rightTop.append(el("span", "item-size", formatBytes(project.totalArtifactBytes, lang)));

      const revealProjectBtn = el("button", "btn btn-ghost btn-icon-xs");
      revealProjectBtn.type = "button";
      revealProjectBtn.disabled = isBusy();
      revealProjectBtn.setAttribute("aria-label", copy.items.revealInFinder);
      revealProjectBtn.title = copy.items.revealInFinder;
      revealProjectBtn.append(icon(FolderOpen, 11));
      revealProjectBtn.addEventListener("click", () => {
        void deps.revealInFinder(project.path);
      });
      rightTop.append(revealProjectBtn);

      top.append(titleGroup, rightTop);
      row.append(
        top,
        createDustBar(project.totalArtifactBytes, maxProjectBytes, "accent"),
      );

      const artifactsWrap = el("div", "dev-artifacts-list");
      for (const artifact of project.artifacts) {
        const artRow = el("div", "dev-artifact-row");
        const artTop = el("div", "subitem-top");
        const artName = el("span", "subitem-name", artifact.name);
        artName.title = artifact.path;

        const artRight = el("div", "subitem-actions");
        artRight.append(el("span", "item-size", formatBytes(artifact.sizeBytes, lang)));

        const artRevealBtn = el("button", "btn btn-ghost btn-icon-xs");
        artRevealBtn.type = "button";
        artRevealBtn.disabled = isBusy();
        artRevealBtn.setAttribute("aria-label", copy.items.revealInFinder);
        artRevealBtn.title = copy.items.revealInFinder;
        artRevealBtn.append(icon(FolderOpen, 11));
        artRevealBtn.addEventListener("click", () => {
          void deps.revealInFinder(artifact.path);
        });

        const artTrashBtn = el("button", "btn btn-ghost btn-xs");
        artTrashBtn.type = "button";
        artTrashBtn.disabled = disabled;
        artTrashBtn.append(icon(Trash2, 11), document.createTextNode(copy.items.trash));
        artTrashBtn.addEventListener("click", () => {
          deps.requestConfirm({
            isPermanent: false,
            getLabel: () =>
              getCopy(deps.getLanguage()).dev.projects.confirmTrashArtifact(
                artifact.name,
                project.name,
                formatBytes(artifact.sizeBytes, deps.getLanguage()),
              ),
            getActionLabel: (inProgress) => {
              const c = getCopy(deps.getLanguage());
              return inProgress ? c.items.moving : c.items.moveToTrash;
            },
            onConfirm: () => executeTrashProjectArtifacts([artifact.path]),
          });
        });

        artRight.append(artRevealBtn, artTrashBtn);
        artTop.append(artName, artRight);

        artRow.append(
          artTop,
          createDustBar(
            artifact.sizeBytes,
            Math.max(1, project.totalArtifactBytes),
            "low",
            true,
            project.totalArtifactBytes,
          ),
        );
        artifactsWrap.append(artRow);
      }

      row.append(artifactsWrap);
      list.append(row);
    }

    target.append(list);
  }

  function renderXcodeBody(target: HTMLElement): void {
    const lang = deps.getLanguage();
    const copy = getCopy(lang);
    const xCopy = copy.dev.xcode;
    const disabled = isActionDisabled();

    if (devState.loading.xcode && !devState.xcodeReport) {
      target.append(renderLoadingCard(copy.dev.loading));
      return;
    }

    if (devState.errors.xcode && !devState.xcodeReport) {
      const errPanel = el("section", "aqua-panel");
      errPanel.append(el("p", "inline-error", copy.dev.loadError(devState.errors.xcode)));
      target.append(errPanel);
      return;
    }

    const report = devState.xcodeReport;
    if (
      !report ||
      (!report.installed &&
        report.areas.length === 0 &&
        report.unavailableSimulators.length === 0 &&
        report.runtimes.length === 0)
    ) {
      const empty = el("div", "empty-state");
      empty.append(
        icon(Hammer, 22),
        el("h2", "section-heading", xCopy.notInstalledTitle),
        el("p", "muted", xCopy.notInstalledDesc),
      );
      target.append(empty);
      return;
    }

    const summaryCard = el("section", "aqua-panel dev-compact-panel");
    const summaryHead = el("div", "surface-row");
    const titleWrap = el("div", "heading-with-icon");
    titleWrap.append(icon(Hammer, 13), el("h2", "section-heading", xCopy.summaryTitle));
    summaryHead.append(
      titleWrap,
      el("span", "reclaim-safe", formatBytes(report.totalReclaimableBytes, lang)),
    );
    summaryCard.append(summaryHead);
    target.append(summaryCard);

    if (
      report.areas.length === 0 &&
      report.unavailableSimulators.length === 0 &&
      report.runtimes.length === 0
    ) {
      const clean = el("div", "empty-state");
      clean.append(
        icon(CheckCircle2, 22),
        el("h2", "section-heading", xCopy.allCleanTitle),
        el("p", "muted", xCopy.allCleanDesc),
      );
      target.append(clean);
      return;
    }

    if (report.unavailableSimulators.length > 0) {
      const card = el("section", "aqua-panel");
      const head = el("div", "surface-row");
      const left = el("div", "heading-with-icon");
      left.append(
        el("h3", "section-heading", xCopy.unavailableTitle),
        el(
          "span",
          "snapshot-count-pill has-items tabular",
          formatInt(report.unavailableSimulators.length, lang),
        ),
      );
      head.append(left, el("span", "item-size", formatBytes(report.unavailableTotalBytes, lang)));
      card.append(head, el("p", "muted", xCopy.unavailableDesc));

      const preview = el("div", "ext-row");
      for (const sim of report.unavailableSimulators.slice(0, 3)) {
        preview.append(el("span", "ext-chip", sim.name));
      }
      if (report.unavailableSimulators.length > 3) {
        preview.append(
          el("span", "ext-chip", xCopy.moreCount(report.unavailableSimulators.length - 3)),
        );
      }
      card.append(preview);

      const actionRow = el("div", "dev-card-footer");
      const cleanBtn = el("button", "btn btn-secondary btn-xs");
      cleanBtn.type = "button";
      cleanBtn.disabled = disabled;
      cleanBtn.append(icon(Trash2, 11), document.createTextNode(xCopy.cleanUnavailable));
      cleanBtn.addEventListener("click", () => {
        deps.requestConfirm({
          isPermanent: true,
          getLabel: () =>
            getCopy(deps.getLanguage()).dev.xcode.confirmUnavailable(
              report.unavailableSimulators.length,
              formatBytes(report.unavailableTotalBytes, deps.getLanguage()),
            ),
          getActionLabel: (inProgress) => {
            const c = getCopy(deps.getLanguage()).dev;
            return inProgress ? c.deletingAction : c.deleteAction;
          },
          onConfirm: () => executeDeleteUnavailableSimulators(),
        });
      });
      actionRow.append(cleanBtn);
      card.append(actionRow);
      target.append(card);
    }

    if (report.runtimes.length > 0) {
      const card = el("section", "aqua-panel");
      const head = el("div", "surface-row");
      const left = el("div", "heading-with-icon");
      left.append(
        el("h3", "section-heading", xCopy.runtimesTitle),
        el("span", "snapshot-count-pill tabular", formatInt(report.runtimes.length, lang)),
      );
      head.append(left);
      card.append(head);

      const maxRuntimeBytes = Math.max(1, ...report.runtimes.map((r) => r.sizeBytes));
      const list = el("div", "dev-sublist");
      for (const rt of report.runtimes) {
        const row = el("div", "subitem-row");
        const top = el("div", "subitem-top");
        const titleGroup = el("div", "item-title-group");
        const nameEl = el("span", "subitem-name", rt.name);
        nameEl.title = `${rt.identifier} (${rt.build})`;
        titleGroup.append(nameEl);
        if (rt.build) {
          titleGroup.append(el("span", "ext-chip", rt.build));
        }

        const right = el("div", "subitem-actions");
        right.append(el("span", "item-size", formatBytes(rt.sizeBytes, lang)));

        const delBtn = el("button", "btn btn-ghost btn-xs");
        delBtn.type = "button";
        delBtn.disabled = disabled;
        delBtn.append(icon(Trash2, 11), document.createTextNode(copy.dev.deleteAction));
        delBtn.addEventListener("click", () => {
          deps.requestConfirm({
            isPermanent: true,
            getLabel: () =>
              getCopy(deps.getLanguage()).dev.xcode.confirmRuntime(
                rt.name,
                formatBytes(rt.sizeBytes, deps.getLanguage()),
              ),
            getActionLabel: (inProgress) => {
              const c = getCopy(deps.getLanguage()).dev;
              return inProgress ? c.deletingAction : c.deleteAction;
            },
            onConfirm: () => executeDeleteRuntime(rt.identifier, rt.name),
          });
        });
        right.append(delBtn);

        top.append(titleGroup, right);
        row.append(top, createDustBar(rt.sizeBytes, maxRuntimeBytes, "med"));
        list.append(row);
      }

      card.append(list);
      target.append(card);
    }

    if (report.areas.length > 0) {
      const card = el("section", "aqua-panel");
      const head = el("div", "surface-row");
      head.append(
        el("h3", "section-heading", xCopy.areasTitle),
        el("span", "snapshot-count-pill tabular", formatInt(report.areas.length, lang)),
      );
      card.append(head);

      const maxAreaBytes = Math.max(1, ...report.areas.map((a) => a.sizeBytes));
      const list = el("div", "dev-sublist");
      for (const area of report.areas) {
        const row = el("div", "subitem-row");
        const top = el("div", "subitem-top");
        const titleGroup = el("div", "item-title-group");
        titleGroup.append(
          el("span", "dev-eco-badge", xCopy.categoryLabels[area.category] ?? area.category),
          el("span", "subitem-name", area.name),
        );
        titleGroup.title = area.path;
        top.append(titleGroup, el("span", "item-size", formatBytes(area.sizeBytes, lang)));

        const isArchive =
          area.category.toLowerCase().includes("archive") ||
          area.category.toLowerCase().includes("support");

        const bottom = el("div", "subitem-bottom");
        bottom.append(
          el("span", "subitem-reason tabular", formatDays(area.inactiveDays, lang)),
        );

        const actions = el("div", "subitem-actions");
        const revealBtn = el("button", "btn btn-ghost btn-icon-xs");
        revealBtn.type = "button";
        revealBtn.disabled = isBusy();
        revealBtn.setAttribute("aria-label", copy.items.revealInFinder);
        revealBtn.title = copy.items.revealInFinder;
        revealBtn.append(icon(FolderOpen, 11));
        revealBtn.addEventListener("click", () => {
          void deps.revealInFinder(area.path);
        });

        const trashBtn = el("button", "btn btn-ghost btn-xs");
        trashBtn.type = "button";
        trashBtn.disabled = disabled;
        trashBtn.append(icon(Trash2, 11), document.createTextNode(copy.items.trash));
        trashBtn.addEventListener("click", () => {
          deps.requestConfirm({
            isPermanent: false,
            getLabel: () =>
              getCopy(deps.getLanguage()).dev.xcode.confirmTrashArea(
                area.name,
                formatBytes(area.sizeBytes, deps.getLanguage()),
              ),
            getActionLabel: (inProgress) => {
              const c = getCopy(deps.getLanguage());
              return inProgress ? c.items.moving : c.items.moveToTrash;
            },
            onConfirm: () => executeTrashXcodeArea(area.path),
          });
        });

        actions.append(revealBtn, trashBtn);
        bottom.append(actions);

        row.append(
          top,
          createDustBar(area.sizeBytes, maxAreaBytes, isArchive ? "med" : "low"),
          bottom,
        );
        list.append(row);
      }

      card.append(list);
      target.append(card);
    }
  }

  function renderDockerBody(target: HTMLElement): void {
    const lang = deps.getLanguage();
    const copy = getCopy(lang);
    const dCopy = copy.dev.docker;
    const disabled = isActionDisabled();

    if (devState.loading.docker && !devState.dockerReport) {
      target.append(renderLoadingCard(copy.dev.loading));
      return;
    }

    if (devState.errors.docker && !devState.dockerReport) {
      const errPanel = el("section", "aqua-panel");
      errPanel.append(el("p", "inline-error", copy.dev.loadError(devState.errors.docker)));
      target.append(errPanel);
      return;
    }

    const report = devState.dockerReport;
    if (!report || !report.installed) {
      const empty = el("div", "empty-state");
      empty.append(
        icon(Box, 22),
        el("h2", "section-heading", dCopy.notInstalledTitle),
        el("p", "muted", dCopy.notInstalledDesc),
      );
      target.append(empty);
      return;
    }

    if (!report.daemonRunning) {
      const empty = el("div", "empty-state");
      empty.append(
        icon(Box, 22),
        el("h2", "section-heading", dCopy.stoppedTitle),
        el("p", "muted", dCopy.stoppedDesc(report.engineName || "Docker")),
      );
      if (report.error) {
        empty.append(el("p", "inline-error", report.error));
      }
      const retryBtn = el("button", "btn btn-secondary btn-xs");
      retryBtn.type = "button";
      retryBtn.disabled = devState.loading.docker || isBusy() || deps.isGlobalBusy();
      retryBtn.append(icon(RefreshCw, 11), document.createTextNode(dCopy.retry));
      retryBtn.addEventListener("click", () => {
        void loadDocker(true);
      });
      empty.append(retryBtn);
      target.append(empty);
      return;
    }

    const engineCard = el("section", "aqua-panel dev-compact-panel");
    const engineHead = el("div", "surface-row");
    const engineTitle = el("div", "heading-with-icon");
    engineTitle.append(
      icon(Box, 13),
      el("h2", "section-heading", report.engineName || "Docker"),
    );
    if (report.orbstackDetected) {
      engineTitle.append(el("span", "ai-chip", "OrbStack"));
    }
    engineHead.append(
      engineTitle,
      el("span", "reclaim-safe", formatBytes(report.totalReclaimableBytes, lang)),
    );

    const statsRow = el("div", "dev-summary-strip");
    statsRow.append(
      el(
        "span",
        "muted tabular",
        `${dCopy.footprintLabel}: ${formatBytes(report.diskFootprintBytes, lang)}`,
      ),
      el("span", "muted", dCopy.reclaimableLabel),
    );
    engineCard.append(engineHead, statsRow);
    target.append(engineCard);

    const resCard = el("section", "aqua-panel");
    resCard.append(el("h3", "section-heading", dCopy.resourcesTitle));

    const maxBytes = Math.max(
      1,
      ...report.resources.map((r) => Math.max(r.sizeBytes, r.reclaimableBytes)),
    );
    const list = el("div", "dev-sublist");

    for (const row of report.resources) {
      const kindLabel = dCopy.kindLabels[row.kind] ?? row.kind;
      const item = el("div", "subitem-row");
      const top = el("div", "subitem-top");
      const titleGroup = el("div", "item-title-group");
      titleGroup.append(el("span", "subitem-name", kindLabel));
      if (row.totalCount > 0) {
        titleGroup.append(
          el(
            "span",
            "ext-chip",
            dCopy.activeOfTotal(
              formatInt(row.activeCount, lang),
              formatInt(row.totalCount, lang),
            ),
          ),
        );
      }
      top.append(titleGroup, el("span", "item-size", formatBytes(row.sizeBytes, lang)));

      const dust = createDustBar(
        row.readOnly ? row.sizeBytes : row.reclaimableBytes,
        maxBytes,
        row.readOnly ? "high" : row.reclaimableBytes > 0 ? "med" : "low",
      );

      const bottom = el("div", "subitem-bottom");

      if (row.readOnly) {
        const noteWrap = el("div", "dev-readonly-wrap");
        const badge = el("span", "readonly-badge");
        badge.append(icon(ShieldAlert, 10), document.createTextNode(dCopy.readOnlyBadge));
        noteWrap.append(badge, el("span", "subitem-reason", dCopy.volumesSafetyNote));
        bottom.append(noteWrap);
      } else {
        const reclaimLabel =
          row.reclaimableBytes > 0
            ? dCopy.reclaimablePct(
                formatBytes(row.reclaimableBytes, lang),
                Math.round(row.reclaimablePercent),
              )
            : dCopy.cleanStatus;
        bottom.append(el("span", "subitem-reason tabular", reclaimLabel));

        if (row.pruneScope !== null && row.reclaimableBytes > 0) {
          const scope = row.pruneScope;
          const pruneBtn = el("button", "btn btn-ghost btn-xs");
          pruneBtn.type = "button";
          pruneBtn.disabled = disabled;
          pruneBtn.append(icon(Trash2, 11), document.createTextNode(dCopy.clean));
          pruneBtn.addEventListener("click", () => {
            deps.requestConfirm({
              isPermanent: true,
              getLabel: () => {
                const c = getCopy(deps.getLanguage()).dev.docker;
                const localizedKind = c.kindLabels[row.kind] ?? row.kind;
                return c.confirmPrune(
                  localizedKind,
                  formatBytes(row.reclaimableBytes, deps.getLanguage()),
                );
              },
              getActionLabel: (inProgress) => {
                const c = getCopy(deps.getLanguage()).dev;
                return inProgress ? c.cleaningAction : c.cleanAction;
              },
              onConfirm: () => executeDockerPrune(scope),
            });
          });
          bottom.append(pruneBtn);
        }
      }

      item.append(top, dust, bottom);
      list.append(item);
    }

    resCard.append(list);
    target.append(resCard);
  }

  function renderToolsBody(target: HTMLElement): void {
    const lang = deps.getLanguage();
    const copy = getCopy(lang);
    const tCopy = copy.dev.tools;
    const disabled = isActionDisabled();

    if (devState.loading.tools && !devState.toolsReport) {
      target.append(renderLoadingCard(copy.dev.loading));
      return;
    }

    if (devState.errors.tools && !devState.toolsReport) {
      const errPanel = el("section", "aqua-panel");
      errPanel.append(el("p", "inline-error", copy.dev.loadError(devState.errors.tools)));
      target.append(errPanel);
      return;
    }

    const report = devState.toolsReport;
    if (!report) {
      return;
    }

    const brew = report.homebrew;
    const brewCard = el("section", "aqua-panel");
    const brewHead = el("div", "surface-row");
    const brewTitle = el("div", "heading-with-icon");
    brewTitle.append(icon(Package, 13), el("h2", "section-heading", tCopy.brewTitle));
    if (brew.installed) {
      brewTitle.append(el("span", "ai-chip", tCopy.dryRunBadge));
    }
    brewHead.append(brewTitle);
    if (brew.installed && brew.reclaimableBytes > 0) {
      brewHead.append(el("span", "reclaim-safe", formatBytes(brew.reclaimableBytes, lang)));
    }
    brewCard.append(brewHead);

    if (!brew.installed) {
      brewCard.append(el("p", "muted", tCopy.brewNotInstalled));
    } else if (
      brew.outdatedCount === 0 &&
      brew.reclaimableBytes === 0 &&
      brew.autoremovePackages.length === 0
    ) {
      brewCard.append(el("p", "muted", tCopy.brewClean));
    } else {
      if (brew.outdatedCount > 0 || brew.reclaimableBytes > 0) {
        brewCard.append(
          el(
            "p",
            "muted tabular",
            tCopy.brewOutdatedSummary(
              formatInt(brew.outdatedCount, lang),
              formatBytes(brew.reclaimableBytes, lang),
            ),
          ),
        );

        const brewActions = el("div", "action-row");
        if (brew.previewLines.length > 0) {
          const togglePreviewBtn = el("button", "btn btn-ghost btn-xs");
          togglePreviewBtn.type = "button";
          togglePreviewBtn.setAttribute("aria-expanded", String(devState.brewPreviewExpanded));
          togglePreviewBtn.append(
            icon(devState.brewPreviewExpanded ? ChevronDown : ChevronRight, 11),
            document.createTextNode(
              devState.brewPreviewExpanded
                ? tCopy.hidePreview
                : tCopy.showPreview(brew.previewLines.length),
            ),
          );
          togglePreviewBtn.addEventListener("click", () => {
            devState.brewPreviewExpanded = !devState.brewPreviewExpanded;
            render();
          });
          brewActions.append(togglePreviewBtn);
        }

        const cleanupBtn = el("button", "btn btn-secondary btn-xs");
        cleanupBtn.type = "button";
        cleanupBtn.disabled = disabled;
        cleanupBtn.append(icon(Terminal, 11), document.createTextNode(tCopy.runCleanup));
        cleanupBtn.addEventListener("click", () => {
          deps.requestConfirm({
            isPermanent: true,
            getLabel: () =>
              getCopy(deps.getLanguage()).dev.tools.confirmBrewCleanup(
                formatBytes(brew.reclaimableBytes, deps.getLanguage()),
              ),
            getActionLabel: (inProgress) => {
              const c = getCopy(deps.getLanguage()).dev;
              return inProgress ? c.runningAction : c.runAction;
            },
            onConfirm: () => executeBrewClean("cleanup"),
          });
        });
        brewActions.append(cleanupBtn);
        brewCard.append(brewActions);

        if (devState.brewPreviewExpanded && brew.previewLines.length > 0) {
          const pre = el("pre", "dev-preview-box", brew.previewLines.join("\n"));
          brewCard.append(pre);
        }
      }

      if (brew.autoremovePackages.length > 0) {
        const autoBlock = el("div", "dev-brew-autoremove");
        autoBlock.append(
          el("p", "muted", tCopy.autoremoveTitle(brew.autoremovePackages.length)),
        );
        const pkgChips = el("div", "ext-row");
        for (const pkg of brew.autoremovePackages.slice(0, 8)) {
          pkgChips.append(el("span", "ext-chip", pkg));
        }
        if (brew.autoremovePackages.length > 8) {
          pkgChips.append(el("span", "ext-chip", `+${brew.autoremovePackages.length - 8}`));
        }
        const autoBtn = el("button", "btn btn-secondary btn-xs");
        autoBtn.type = "button";
        autoBtn.disabled = disabled;
        autoBtn.append(icon(Terminal, 11), document.createTextNode(tCopy.runAutoremove));
        autoBtn.addEventListener("click", () => {
          deps.requestConfirm({
            isPermanent: true,
            getLabel: () =>
              getCopy(deps.getLanguage()).dev.tools.confirmBrewAutoremove(
                brew.autoremovePackages.length,
              ),
            getActionLabel: (inProgress) => {
              const c = getCopy(deps.getLanguage()).dev;
              return inProgress ? c.runningAction : c.runAction;
            },
            onConfirm: () => executeBrewClean("autoremove"),
          });
        });
        const autoFooter = el("div", "dev-card-footer");
        autoFooter.append(autoBtn);
        autoBlock.append(pkgChips, autoFooter);
        brewCard.append(autoBlock);
      }
    }
    target.append(brewCard);

    const cachesCard = el("section", "aqua-panel");
    const cachesHead = el("div", "surface-row");
    const cachesTitle = el("div", "heading-with-icon");
    cachesTitle.append(icon(Wrench, 13), el("h2", "section-heading", tCopy.cachesTitle));
    const cachesTotal = report.caches.reduce((sum, c) => sum + c.sizeBytes, 0);
    cachesHead.append(
      cachesTitle,
      el("span", "item-size", formatBytes(cachesTotal, lang)),
    );
    cachesCard.append(cachesHead);

    if (report.caches.length === 0) {
      cachesCard.append(el("p", "muted", tCopy.cachesEmpty));
    } else {
      const maxCacheBytes = Math.max(1, ...report.caches.map((c) => c.sizeBytes));
      const list = el("div", "dev-sublist");
      for (const cache of report.caches) {
        const row = el("div", "subitem-row");
        const top = el("div", "subitem-top");
        const titleGroup = el("div", "item-title-group");
        titleGroup.append(
          el("span", "dev-eco-badge", cache.ecosystem),
          el("span", "subitem-name", cache.name),
        );
        top.append(titleGroup, el("span", "item-size", formatBytes(cache.sizeBytes, lang)));

        const bottom = el("div", "subitem-bottom");
        const pathHint = el("span", "subitem-reason dev-path-hint", shortenHomePath(cache.path));
        pathHint.title = cache.path;

        const actions = el("div", "subitem-actions");
        const revealBtn = el("button", "btn btn-ghost btn-icon-xs");
        revealBtn.type = "button";
        revealBtn.disabled = isBusy();
        revealBtn.setAttribute("aria-label", copy.items.revealInFinder);
        revealBtn.title = copy.items.revealInFinder;
        revealBtn.append(icon(FolderOpen, 11));
        revealBtn.addEventListener("click", () => {
          void deps.revealInFinder(cache.path);
        });

        const trashBtn = el("button", "btn btn-ghost btn-xs");
        trashBtn.type = "button";
        trashBtn.disabled = disabled;
        trashBtn.append(icon(Trash2, 11), document.createTextNode(copy.items.trash));
        trashBtn.addEventListener("click", () => {
          deps.requestConfirm({
            isPermanent: false,
            getLabel: () =>
              getCopy(deps.getLanguage()).dev.tools.confirmTrashCache(
                cache.name,
                formatBytes(cache.sizeBytes, deps.getLanguage()),
              ),
            getActionLabel: (inProgress) => {
              const c = getCopy(deps.getLanguage());
              return inProgress ? c.items.moving : c.items.moveToTrash;
            },
            onConfirm: () => executeTrashToolchainCache(cache.path, cache.name),
          });
        });

        actions.append(revealBtn, trashBtn);
        bottom.append(pathHint, actions);

        row.append(top, createDustBar(cache.sizeBytes, maxCacheBytes, "low"), bottom);
        list.append(row);
      }
      cachesCard.append(list);
    }

    target.append(cachesCard);
  }

  function render(): void {
    const lang = deps.getLanguage();
    const copy = getCopy(lang);
    const prevScroll =
      bodyEl && lastRenderedSubTab === devState.activeSubTab ? bodyEl.scrollTop : 0;

    deps.container.replaceChildren();

    const toolbar = el("div", "dev-toolbar");
    const subnav = el("div", "dev-subnav");
    subnav.setAttribute("role", "tablist");
    subnav.setAttribute("aria-label", copy.tabs.dev);
    const subTabs: Array<{ id: DevSubTab; label: string }> = [
      { id: "projects", label: copy.dev.subtabs.projects },
      { id: "xcode", label: copy.dev.subtabs.xcode },
      { id: "docker", label: copy.dev.subtabs.docker },
      { id: "tools", label: copy.dev.subtabs.tools },
    ];

    const switchSubTab = (targetSub: DevSubTab, focusTarget = false): void => {
      if (devState.activeSubTab !== targetSub) {
        devState.activeSubTab = targetSub;
        if (!isBusy() && !deps.isGlobalBusy()) {
          deps.clearConfirm();
        }
        render();
        ensureSubTabLoaded(targetSub, false);
      }
      if (focusTarget) {
        const nextBtn = deps.container.querySelector<HTMLButtonElement>(
          `.dev-subpill[data-subtab="${targetSub}"]`,
        );
        nextBtn?.focus();
      }
    };

    for (const sub of subTabs) {
      const btn = el("button", "filter-pill dev-subpill", sub.label);
      const isSelected = devState.activeSubTab === sub.id;
      btn.type = "button";
      btn.dataset.subtab = sub.id;
      btn.setAttribute("role", "tab");
      btn.setAttribute("aria-selected", String(isSelected));
      btn.classList.toggle("is-active", isSelected);
      btn.addEventListener("click", () => {
        switchSubTab(sub.id, false);
      });
      btn.addEventListener("keydown", (event) => {
        if (event.key !== "ArrowRight" && event.key !== "ArrowLeft") {
          return;
        }
        event.preventDefault();
        const idx = subTabs.findIndex((s) => s.id === sub.id);
        const delta = event.key === "ArrowRight" ? 1 : -1;
        const nextSub = subTabs[(idx + delta + subTabs.length) % subTabs.length].id;
        switchSubTab(nextSub, true);
      });
      subnav.append(btn);
    }

    const refreshBtn = el("button", "btn btn-ghost btn-icon-xs");
    refreshBtn.type = "button";
    const activeLoading = devState.loading[devState.activeSubTab];
    refreshBtn.disabled =
      activeLoading || devState.mutating || devState.pickingRoot || deps.isGlobalBusy();
    refreshBtn.classList.toggle("is-spinning", activeLoading);
    refreshBtn.setAttribute("aria-label", copy.dev.refreshAria);
    refreshBtn.title = copy.dev.refreshAria;
    refreshBtn.append(icon(RefreshCw, 11));
    refreshBtn.addEventListener("click", () => {
      if (!deps.isGlobalBusy() && !devState.mutating) {
        deps.clearConfirm();
      }
      ensureSubTabLoaded(devState.activeSubTab, true);
    });

    toolbar.append(subnav, refreshBtn);

    const body = el("div", "dev-body");
    if (devState.activeSubTab === "projects") {
      renderProjectsBody(body);
    } else if (devState.activeSubTab === "xcode") {
      renderXcodeBody(body);
    } else if (devState.activeSubTab === "docker") {
      renderDockerBody(body);
    } else {
      renderToolsBody(body);
    }

    deps.container.append(toolbar, body);
    bodyEl = body;
    lastRenderedSubTab = devState.activeSubTab;
    body.scrollTop = prevScroll;
  }

  function activate(): void {
    render();
    ensureSubTabLoaded(devState.activeSubTab, false);
  }

  return {
    activate,
    render,
    isBusy,
    isScanning,
  };
}
