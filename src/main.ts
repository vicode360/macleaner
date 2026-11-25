import { Channel, invoke } from "@tauri-apps/api/core";
import {
  AlertTriangle,
  ArrowUpCircle,
  ChevronDown,
  ChevronRight,
  Code2,
  Cpu,
  EyeOff,
  FolderOpen,
  FolderSearch,
  Globe,
  HardDrive,
  History,
  RefreshCw,
  ScanSearch,
  SlidersHorizontal,
  Sparkles,
  Trash2,
  createElement,
  type IconNode,
} from "lucide";
import { createDevPanel, type CleanedPathItem, type DevConfirmRequest } from "./dev-panel";
import { createDustBar, riskToDustVariant } from "./dust-bar";
import {
  formatBytes,
  formatDays,
  formatInt,
  formatSnapshotAge,
  formatSnapshotTimestamp,
  getCopy,
} from "./i18n";
import type {
  AiConfig,
  AiProvider,
  ApfsSnapshot,
  AppLanguage,
  DiskHealth,
  RiskLevel,
  ScanEvent,
  ScanItem,
  ScanSummary,
  SnapshotDeleteResult,
  SnapshotListResult,
  TrashResult,
  UpdateCheckResult,
  UpdateInstallEvent,
} from "./types";

type TabId = "resumo" | "itens" | "dev" | "ajustes";
type RiskFilter = "todos" | RiskLevel;

type PendingConfirmAction =
  | {
      kind: "trash";
      originTab: TabId;
      isPermanent: false;
      paths: string[];
      getLabel: () => string;
    }
  | {
      kind: "snapshot";
      originTab: TabId;
      isPermanent: true;
      dateTokens: string[];
      getLabel: () => string;
    }
  | {
      kind: "dev";
      originTab: "dev";
      isPermanent: boolean;
      getLabel: () => string;
      getActionLabel: (inProgress: boolean) => string;
      onConfirm: () => Promise<void>;
    };

interface AppViewState {
  activeTab: TabId;
  appVersion: string;
  language: AppLanguage;
  updateResult: UpdateCheckResult | null;
  checkingUpdate: boolean;
  installingUpdate: boolean;
  updateProgressText: string;
  diskHealth: DiskHealth | null;
  summary: ScanSummary | null;
  snapshots: ApfsSnapshot[];
  snapshotsLoaded: boolean;
  snapshotsSupported: boolean;
  snapshotsNote: string | null;
  loadingSnapshots: boolean;
  deletingSnapshots: boolean;
  snapshotsExpanded: boolean;
  autostartEnabled: boolean;
  updatingAutostart: boolean;
  aiConfig: AiConfig | null;
  settingsDraft: AiConfig | null;
  installedOllamaModels: string[];
  expandedItemIds: Set<string>;
  scanning: boolean;
  scanProgressText: string;
  trashing: boolean;
  riskFilter: RiskFilter;
  highlightedItemId: string | null;
  pendingConfirm: PendingConfirmAction | null;
  settingsError: string | null;
  detectingModels: boolean;
  savingSettings: boolean;
}

const state: AppViewState = {
  activeTab: "resumo",
  appVersion: "0.3.0",
  language: "pt-br",
  updateResult: null,
  checkingUpdate: false,
  installingUpdate: false,
  updateProgressText: "",
  diskHealth: null,
  summary: null,
  snapshots: [],
  snapshotsLoaded: false,
  snapshotsSupported: true,
  snapshotsNote: null,
  loadingSnapshots: false,
  deletingSnapshots: false,
  snapshotsExpanded: false,
  autostartEnabled: false,
  updatingAutostart: false,
  aiConfig: null,
  settingsDraft: null,
  installedOllamaModels: [],
  expandedItemIds: new Set<string>(),
  scanning: false,
  scanProgressText: "",
  trashing: false,
  riskFilter: "todos",
  highlightedItemId: null,
  pendingConfirm: null,
  settingsError: null,
  detectingModels: false,
  savingSettings: false,
};

const panelResumo = mustEl<HTMLElement>("#panel-resumo");
const panelItens = mustEl<HTMLElement>("#panel-itens");
const panelDev = mustEl<HTMLElement>("#panel-dev");
const panelAjustes = mustEl<HTMLElement>("#panel-ajustes");
const brandIconSlot = mustEl<HTMLElement>("#brand-icon-slot");
const versionTag = mustEl<HTMLElement>("#version-tag");
const updateHeaderBtn = mustEl<HTMLButtonElement>("#update-header-btn");
const rescanIconBtn = mustEl<HTMLButtonElement>("#rescan-icon-btn");
const tabListEl = mustEl<HTMLElement>("#tab-list");
const tabResumoBtn = mustEl<HTMLButtonElement>("#tab-resumo");
const tabItensBtn = mustEl<HTMLButtonElement>("#tab-itens");
const tabDevBtn = mustEl<HTMLButtonElement>("#tab-dev");
const tabAjustesBtn = mustEl<HTMLButtonElement>("#tab-ajustes");
const confirmBar = mustEl<HTMLElement>("#confirm-bar");
const confirmPermanentBadge = mustEl<HTMLElement>("#confirm-permanent-badge");
const confirmMessage = mustEl<HTMLElement>("#confirm-message");
const confirmCancelBtn = mustEl<HTMLButtonElement>("#confirm-cancel-btn");
const confirmAcceptBtn = mustEl<HTMLButtonElement>("#confirm-accept-btn");
const toastEl = mustEl<HTMLElement>("#toast");
const tabButtons = [tabResumoBtn, tabItensBtn, tabDevBtn, tabAjustesBtn];

const devPanel = createDevPanel({
  container: panelDev,
  getLanguage: () => state.language,
  isGlobalBusy: () => state.scanning || state.trashing || state.deletingSnapshots,
  requestConfirm: (req: DevConfirmRequest) => {
    state.pendingConfirm = {
      kind: "dev",
      originTab: "dev",
      ...req,
    };
    updateConfirmBar();
  },
  clearConfirm: () => {
    if (state.pendingConfirm?.originTab === "dev" && !isMutating()) {
      state.pendingConfirm = null;
      updateConfirmBar();
    }
  },
  showToast,
  revealInFinder,
  onStateChange: () => {
    updateChrome();
    updateConfirmBar();
  },
  onCleaned: async (trashedPaths: string[], cleanedItems?: CleanedPathItem[]) => {
    reconcileSummaryAfterTrash(trashedPaths, cleanedItems);
    await refreshDiskHealth();
    syncAtmosphere();
    updateChrome();
    if (state.activeTab === "resumo") {
      renderResumo();
    } else if (state.activeTab === "itens") {
      renderItens();
    }
  },
});

function isMutating(): boolean {
  return state.trashing || state.deletingSnapshots || devPanel.isBusy();
}

function isScanningAny(): boolean {
  return state.scanning || devPanel.isScanning();
}

let toastTimer: number | undefined;

function mustEl<T extends Element>(selector: string): T {
  const node = document.querySelector<T>(selector);
  if (!node) {
    throw new Error(`Missing DOM element: ${selector}`);
  }
  return node;
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

function icon(iconNode: IconNode, size = 14): SVGElement {
  return createElement(iconNode, {
    width: size,
    height: size,
    "stroke-width": 1.75,
    "aria-hidden": "true",
  });
}

function showToast(message: string): void {
  if (toastTimer !== undefined) {
    window.clearTimeout(toastTimer);
  }
  toastEl.textContent = message;
  toastEl.hidden = false;
  toastTimer = window.setTimeout(() => {
    toastEl.hidden = true;
  }, 3400);
}

function syncAtmosphere(): void {
  const used = state.diskHealth?.usedPercent ?? 0;
  const health = used >= 90 ? "critical" : used >= 75 ? "warn" : "healthy";
  document.body.dataset.health = health;
}

const tabScrollTop: Record<TabId, number> = {
  resumo: 0,
  itens: 0,
  dev: 0,
  ajustes: 0,
};

function getTabScrollContainer(tab: TabId): HTMLElement | null {
  if (tab === "resumo") {
    return panelResumo;
  }
  if (tab === "itens") {
    return panelItens.querySelector<HTMLElement>(".items-list");
  }
  if (tab === "dev") {
    return panelDev.querySelector<HTMLElement>(".dev-body");
  }
  return panelAjustes;
}

function saveTabScroll(tab: TabId): void {
  const scroller = getTabScrollContainer(tab);
  if (scroller) {
    tabScrollTop[tab] = scroller.scrollTop;
  }
}

function restoreTabScroll(tab: TabId): void {
  const scroller = getTabScrollContainer(tab);
  if (scroller) {
    scroller.scrollTop = tabScrollTop[tab];
  }
}

function setActiveTab(tab: TabId): void {
  saveTabScroll(state.activeTab);
  state.activeTab = tab;
  for (const btn of tabButtons) {
    const isMatch = btn.dataset.tab === tab;
    btn.classList.toggle("is-active", isMatch);
    btn.setAttribute("aria-selected", String(isMatch));
  }
  panelResumo.hidden = tab !== "resumo";
  panelItens.hidden = tab !== "itens";
  panelDev.hidden = tab !== "dev";
  panelAjustes.hidden = tab !== "ajustes";
  if (state.pendingConfirm && state.pendingConfirm.originTab !== tab && !isMutating()) {
    state.pendingConfirm = null;
  }
  if (tab === "dev") {
    syncAtmosphere();
    updateChrome();
    updateConfirmBar();
    devPanel.activate();
    restoreTabScroll("dev");
    return;
  }
  renderActiveTab();
  restoreTabScroll(tab);
}

function updateChrome(): void {
  const copy = getCopy(state.language);
  document.documentElement.lang = copy.htmlLang;
  tabListEl.setAttribute("aria-label", copy.tabs.navLabel);

  if (brandIconSlot.childElementCount === 0) {
    brandIconSlot.append(icon(Sparkles, 14));
  }

  versionTag.textContent = `v${state.appVersion}`;
  rescanIconBtn.disabled = isScanningAny() || isMutating();
  rescanIconBtn.classList.toggle("is-spinning", state.scanning);
  rescanIconBtn.setAttribute("aria-label", copy.header.rescanAria);
  rescanIconBtn.title = copy.summary.scanDisk;
  if (rescanIconBtn.childElementCount === 0) {
    rescanIconBtn.append(icon(RefreshCw, 14));
  }

  updateHeaderBtn.disabled = state.checkingUpdate || state.installingUpdate;
  updateHeaderBtn.setAttribute("aria-label", copy.header.checkUpdateTitle);
  updateHeaderBtn.replaceChildren(icon(ArrowUpCircle, 12));
  if (state.installingUpdate) {
    updateHeaderBtn.append(
      document.createTextNode(state.updateProgressText || copy.header.installingUpdate),
    );
    updateHeaderBtn.classList.add("has-update");
    updateHeaderBtn.title = state.updateProgressText || copy.header.installingUpdate;
  } else if (state.checkingUpdate) {
    updateHeaderBtn.append(document.createTextNode(copy.header.checkingUpdate));
    updateHeaderBtn.classList.remove("has-update");
    updateHeaderBtn.title = copy.header.checkingUpdate;
  } else if (state.updateResult?.updateAvailable && state.updateResult.latestVersion) {
    updateHeaderBtn.append(
      document.createTextNode(copy.header.installVersion(state.updateResult.latestVersion)),
    );
    updateHeaderBtn.classList.add("has-update");
    updateHeaderBtn.title = copy.toasts.updateAvailable(
      state.updateResult.latestVersion,
      state.appVersion,
    );
  } else {
    updateHeaderBtn.append(document.createTextNode(copy.header.updateIdle));
    updateHeaderBtn.classList.remove("has-update");
    updateHeaderBtn.title = copy.header.checkUpdateTitle;
  }

  tabResumoBtn.replaceChildren(icon(HardDrive, 12), el("span", undefined, copy.tabs.resumo));

  const count = state.summary?.items.length ?? 0;
  const countLabel = count > 99 ? "99+" : formatInt(count, state.language);
  const countBadge = el("span", "tab-count", countLabel);
  countBadge.hidden = count === 0;
  tabItensBtn.replaceChildren(
    icon(FolderSearch, 12),
    el("span", undefined, copy.tabs.itens),
    countBadge,
  );

  tabDevBtn.replaceChildren(icon(Code2, 12), el("span", undefined, copy.tabs.dev));

  tabAjustesBtn.replaceChildren(
    icon(SlidersHorizontal, 12),
    el("span", undefined, copy.tabs.ajustes),
  );
}

function updateConfirmBar(): void {
  const copy = getCopy(state.language);
  confirmCancelBtn.textContent = copy.items.cancel;

  if (!state.pendingConfirm) {
    confirmBar.hidden = true;
    confirmBar.classList.remove("is-permanent");
    confirmPermanentBadge.hidden = true;
    document.documentElement.style.removeProperty("--toast-bottom");
    confirmAcceptBtn.disabled = false;
    confirmCancelBtn.disabled = false;
    confirmAcceptBtn.replaceChildren(icon(Trash2, 12), document.createTextNode(copy.items.moveToTrash));
    return;
  }

  const pending = state.pendingConfirm;
  const busy = isMutating();
  let actionLabel: string;
  if (pending.kind === "snapshot") {
    actionLabel = state.deletingSnapshots ? copy.snapshots.removing : copy.snapshots.confirmAction;
  } else if (pending.kind === "dev") {
    actionLabel = pending.getActionLabel(devPanel.isBusy());
  } else {
    actionLabel = state.trashing ? copy.items.moving : copy.items.moveToTrash;
  }

  confirmBar.classList.toggle("is-permanent", pending.isPermanent);
  if (pending.isPermanent) {
    confirmPermanentBadge.replaceChildren(
      icon(AlertTriangle, 11),
      document.createTextNode(copy.confirm.permanentWarning),
    );
    confirmPermanentBadge.hidden = false;
  } else {
    confirmPermanentBadge.hidden = true;
  }

  confirmMessage.textContent = pending.getLabel();
  confirmAcceptBtn.disabled = busy || isScanningAny();
  confirmCancelBtn.disabled = busy;
  confirmAcceptBtn.replaceChildren(
    icon(pending.isPermanent ? AlertTriangle : Trash2, 12),
    document.createTextNode(actionLabel),
  );
  confirmBar.hidden = false;
  const offset = confirmBar.offsetHeight > 0 ? confirmBar.offsetHeight + 20 : 92;
  document.documentElement.style.setProperty("--toast-bottom", `${offset}px`);
}

function renderActiveTab(): void {
  syncAtmosphere();
  updateChrome();
  updateConfirmBar();
  if (state.activeTab === "resumo") {
    renderResumo();
  } else if (state.activeTab === "itens") {
    renderItens();
  } else if (state.activeTab === "dev") {
    devPanel.render();
  } else {
    renderAjustes();
  }
}

function renderResumo(): void {
  const prevScroll = panelResumo.scrollTop;
  const copy = getCopy(state.language);
  panelResumo.replaceChildren();

  const diskSection = el("section", "aqua-panel");
  const diskHead = el("div", "surface-row");
  const diskTitleWrap = el("div", "heading-with-icon");
  diskTitleWrap.append(
    icon(HardDrive, 14),
    el("h2", "section-heading", state.diskHealth?.volumeName ?? copy.disk.localFallback),
  );
  diskHead.append(diskTitleWrap);

  if (state.diskHealth && state.diskHealth.totalBytes > 0) {
    const { availableBytes, totalBytes, usedPercent } = state.diskHealth;
    const healthLabel =
      usedPercent >= 90 ? copy.disk.critical : usedPercent >= 75 ? copy.disk.warn : copy.disk.healthy;
    const healthClass =
      usedPercent >= 90 ? "risk-alto" : usedPercent >= 75 ? "risk-medio" : "risk-baixo";
    diskHead.append(el("span", `risk-badge ${healthClass}`, `${healthLabel} · ${usedPercent}%`));

    const bar = el("div", "disk-bar");
    const reclaimBytes = Math.min(state.summary?.totalBytes ?? 0, state.diskHealth.usedBytes);
    const nonReclaimUsed = Math.max(0, state.diskHealth.usedBytes - reclaimBytes);
    const usedPct = (nonReclaimUsed / totalBytes) * 100;
    const reclaimPct = (reclaimBytes / totalBytes) * 100;

    const usedSeg = el("div", "disk-bar-used");
    usedSeg.style.width = `${usedPct.toFixed(2)}%`;
    const reclaimSeg = el("div", "disk-bar-reclaim");
    reclaimSeg.style.width = `${reclaimPct.toFixed(2)}%`;
    bar.append(usedSeg, reclaimSeg);

    const diskMeta = el(
      "p",
      "muted tabular",
      copy.disk.freeOf(
        formatBytes(availableBytes, state.language),
        formatBytes(totalBytes, state.language),
      ),
    );

    const legend = el("div", "legend-row");
    const usedLeg = el("span");
    usedLeg.append(el("span", "legend-dot is-used"), document.createTextNode(copy.disk.inUse));
    const reclaimLeg = el("span");
    reclaimLeg.append(
      el("span", "legend-dot is-reclaim"),
      document.createTextNode(
        state.summary
          ? `${copy.disk.mapped} (${formatBytes(reclaimBytes, state.language)})`
          : copy.disk.mapped,
      ),
    );
    legend.append(usedLeg, reclaimLeg);

    diskSection.append(diskHead, bar, diskMeta, legend);
  } else {
    diskHead.append(el("span", "muted", copy.disk.readingVolume));
    diskSection.append(diskHead);
  }

  const scanSection = el("section", "aqua-panel");
  const scanHead = el("div", "surface-row");
  const scanTitleWrap = el("div", "heading-with-icon");
  scanTitleWrap.append(icon(ScanSearch, 14), el("h2", "section-heading", copy.summary.smartCleanup));
  scanHead.append(scanTitleWrap);
  if (state.summary) {
    scanHead.append(
      el("span", "muted tabular", `${formatInt(state.summary.durationMs, state.language)} ms`),
    );
  }

  const figures = el("div", "telemetry-figures");
  const totalText = state.summary
    ? formatBytes(state.summary.totalBytes, state.language)
    : copy.summary.readyToScan;
  figures.append(el("span", state.summary ? "reclaim-figure" : "muted", totalText));
  if (state.summary) {
    figures.append(
      el(
        "span",
        "reclaim-safe",
        copy.summary.safeLabel(formatBytes(state.summary.safeBytes, state.language)),
      ),
    );
  }

  const scanDesc = el(
    "p",
    "muted",
    state.scanning
      ? state.scanProgressText || copy.summary.startingScan
      : state.summary
        ? copy.summary.foldersAnalyzed(formatInt(state.summary.itemCount, state.language))
        : copy.summary.idleHint,
  );
  scanDesc.id = "scan-progress-desc";

  const actionRow = el("div", "action-row");
  const scanBtn = el("button", "btn btn-primary");
  scanBtn.type = "button";
  scanBtn.disabled = isScanningAny() || isMutating();
  scanBtn.append(
    icon(ScanSearch, 14),
    document.createTextNode(
      state.scanning
        ? copy.summary.scanning
        : state.summary
          ? copy.summary.rescan
          : copy.summary.scanDisk,
    ),
  );
  scanBtn.addEventListener("click", () => {
    void runScan();
  });
  actionRow.append(scanBtn);

  const safeTargets = collectSafeTargets();
  if (safeTargets.paths.length > 0 && !state.scanning) {
    const cleanSafeBtn = el("button", "btn btn-secondary tabular");
    cleanSafeBtn.type = "button";
    cleanSafeBtn.disabled = isMutating() || isScanningAny();
    cleanSafeBtn.append(
      icon(Trash2, 13),
      document.createTextNode(copy.summary.cleanSafeCount(safeTargets.paths.length)),
    );
    cleanSafeBtn.addEventListener("click", () => {
      requestCleanSafeTargets();
    });
    actionRow.append(cleanSafeBtn);
  }

  scanSection.append(scanHead, figures, scanDesc, actionRow);
  panelResumo.append(diskSection, scanSection, buildSnapshotsSection());

  if (state.summary && state.summary.items.length > 0) {
    const villainsSection = el("section", "aqua-panel");
    const villainsHead = el("div", "surface-row");
    villainsHead.append(el("h2", "section-heading", copy.summary.largestCulprits));
    const seeAllBtn = el("button", "btn btn-ghost btn-xs", copy.summary.viewAll);
    seeAllBtn.type = "button";
    seeAllBtn.addEventListener("click", () => {
      state.riskFilter = "todos";
      setActiveTab("itens");
    });
    villainsHead.append(seeAllBtn);

    const list = el("div", "villains-list");
    for (const item of state.summary.items.slice(0, 4)) {
      const row = el("button", "villain-row");
      row.type = "button";
      const info = el("div", "villain-info");
      info.append(
        el("span", `risk-badge risk-${item.risk}`, copy.risk[item.risk]),
        el("span", "villain-name", item.name),
      );
      row.append(
        info,
        el("span", "item-size", formatBytes(item.sizeBytes, state.language)),
      );
      row.addEventListener("click", () => {
        state.highlightedItemId = item.id;
        if (item.subitems.length > 0) {
          state.expandedItemIds.add(item.id);
        }
        state.riskFilter = "todos";
        setActiveTab("itens");
      });
      list.append(row);
    }

    villainsSection.append(villainsHead, list);
    panelResumo.append(villainsSection);
  }
  panelResumo.scrollTop = prevScroll;
}

function buildSnapshotsSection(): HTMLElement {
  const copy = getCopy(state.language);
  const section = el("section", "aqua-panel snapshots-panel");

  const head = el("div", "surface-row");
  const titleWrap = el("div", "heading-with-icon");
  titleWrap.append(icon(History, 14), el("h2", "section-heading", copy.snapshots.title));

  if (state.snapshotsLoaded && state.snapshotsSupported) {
    const count = state.snapshots.length;
    const pillClass = count > 0 ? "snapshot-count-pill has-items tabular" : "snapshot-count-pill tabular";
    titleWrap.append(el("span", pillClass, formatInt(count, state.language)));
  }

  const headActions = el("div", "snapshot-head-actions");

  const refreshBtn = el("button", "btn btn-ghost btn-icon-xs");
  refreshBtn.type = "button";
  refreshBtn.disabled = state.loadingSnapshots || state.deletingSnapshots;
  refreshBtn.classList.toggle("is-spinning", state.loadingSnapshots);
  refreshBtn.setAttribute("aria-label", copy.snapshots.refreshAria);
  refreshBtn.title = copy.snapshots.refreshAria;
  refreshBtn.append(icon(RefreshCw, 11));
  refreshBtn.addEventListener("click", () => {
    void refreshSnapshots(false);
  });
  headActions.append(refreshBtn);

  if (state.snapshotsSupported && state.snapshots.length > 0) {
    const toggleBtn = el("button", "btn btn-ghost btn-xs");
    toggleBtn.type = "button";
    toggleBtn.setAttribute("aria-expanded", String(state.snapshotsExpanded));
    toggleBtn.append(
      icon(state.snapshotsExpanded ? ChevronDown : ChevronRight, 11),
      document.createTextNode(state.snapshotsExpanded ? copy.snapshots.hide : copy.snapshots.show),
    );
    toggleBtn.addEventListener("click", () => {
      state.snapshotsExpanded = !state.snapshotsExpanded;
      renderResumo();
    });
    headActions.append(toggleBtn);
  }

  head.append(titleWrap, headActions);
  section.append(head);

  if (!state.snapshotsLoaded && state.loadingSnapshots) {
    section.append(el("p", "muted", copy.snapshots.loading));
    return section;
  }

  if (!state.snapshotsSupported) {
    section.append(el("p", "muted", state.snapshotsNote ?? copy.snapshots.unsupported));
    return section;
  }

  if (state.snapshots.length === 0) {
    section.append(el("p", "muted", copy.snapshots.empty));
    return section;
  }

  if (!state.snapshotsExpanded) {
    section.append(el("p", "muted", copy.snapshots.summaryLine(state.snapshots.length)));
    return section;
  }

  section.append(el("p", "muted", copy.snapshots.hint));

  const list = el("div", "snapshots-list");
  for (const snap of state.snapshots) {
    const row = el("div", "snapshot-row");
    const info = el("div", "snapshot-info");
    const formattedDate = formatSnapshotTimestamp(snap.timestampIso, state.language);
    const age = formatSnapshotAge(snap.timestampIso, state.language);

    const dateEl = el("span", "snapshot-date tabular", formattedDate);
    dateEl.title = snap.id;
    info.append(dateEl);
    if (age) {
      info.append(el("span", "snapshot-meta", `${age} · ${snap.volume}`));
    }

    const delBtn = el("button", "btn btn-ghost btn-xs");
    delBtn.type = "button";
    delBtn.disabled = isMutating() || isScanningAny();
    delBtn.append(icon(Trash2, 11), document.createTextNode(copy.snapshots.remove));
    delBtn.addEventListener("click", () => {
      state.pendingConfirm = {
        kind: "snapshot",
        originTab: "resumo",
        isPermanent: true,
        dateTokens: [snap.dateToken],
        getLabel: () =>
          getCopy(state.language).snapshots.confirmSingle(
            formatSnapshotTimestamp(snap.timestampIso, state.language),
          ),
      };
      updateConfirmBar();
    });

    row.append(info, delBtn);
    list.append(row);
  }

  section.append(list);

  if (state.snapshots.length > 1) {
    const footerRow = el("div", "snapshot-footer");
    const clearAllBtn = el("button", "btn btn-secondary btn-xs");
    clearAllBtn.type = "button";
    clearAllBtn.disabled = isMutating() || isScanningAny();
    clearAllBtn.append(icon(Trash2, 11), document.createTextNode(copy.snapshots.clearAll));
    clearAllBtn.addEventListener("click", () => {
      const tokens = state.snapshots.map((snap) => snap.dateToken);
      state.pendingConfirm = {
        kind: "snapshot",
        originTab: "resumo",
        isPermanent: true,
        dateTokens: tokens,
        getLabel: () => getCopy(state.language).snapshots.confirmAll(tokens.length),
      };
      updateConfirmBar();
    });
    footerRow.append(clearAllBtn);
    section.append(footerRow);
  }

  return section;
}

function renderItens(): void {
  const prevScroll = panelItens.querySelector(".items-list")?.scrollTop ?? 0;
  const copy = getCopy(state.language);
  panelItens.replaceChildren();

  if (!state.summary) {
    const empty = el("div", "empty-state");
    empty.append(
      icon(FolderSearch, 24),
      el("h2", "section-heading", copy.items.emptyTitle),
      el("p", "muted", copy.items.emptyDesc),
    );
    const btn = el("button", "btn btn-primary");
    btn.type = "button";
    btn.disabled = isScanningAny() || isMutating();
    btn.append(
      icon(ScanSearch, 14),
      document.createTextNode(state.scanning ? copy.summary.scanning : copy.summary.scanDisk),
    );
    btn.addEventListener("click", () => {
      void runScan();
    });
    empty.append(btn);
    panelItens.append(empty);
    return;
  }

  const toolbar = el("div", "items-toolbar");
  const pills = el("div", "filter-pills");
  const filters: Array<{ id: RiskFilter; label: string }> = [
    { id: "todos", label: copy.items.filterAll },
    { id: "baixo", label: copy.risk.baixo },
    { id: "medio", label: copy.risk.medio },
    { id: "alto", label: copy.risk.alto },
  ];

  for (const filter of filters) {
    const pill = el("button", "filter-pill", filter.label);
    const active = state.riskFilter === filter.id;
    pill.type = "button";
    pill.classList.toggle("is-active", active);
    pill.setAttribute("aria-pressed", active ? "true" : "false");
    pill.addEventListener("click", () => {
      state.riskFilter = filter.id;
      renderItens();
    });
    pills.append(pill);
  }

  toolbar.append(pills);

  const safeTargets = collectSafeTargets();
  if (safeTargets.paths.length > 0) {
    const cleanAllBtn = el("button", "btn btn-secondary btn-xs");
    cleanAllBtn.type = "button";
    cleanAllBtn.disabled = isScanningAny() || isMutating();
    cleanAllBtn.append(icon(Trash2, 11), document.createTextNode(copy.items.cleanSafe));
    cleanAllBtn.addEventListener("click", () => {
      requestCleanSafeTargets();
    });
    toolbar.append(cleanAllBtn);
  }

  panelItens.append(toolbar);

  const visibleItems = state.summary.items.filter(
    (item) => state.riskFilter === "todos" || item.risk === state.riskFilter,
  );

  if (visibleItems.length === 0) {
    const empty = el("div", "empty-state");
    empty.append(
      el("h2", "section-heading", copy.items.filterEmptyTitle),
      el("p", "muted", copy.items.filterEmptyDesc),
    );
    panelItens.append(empty);
    return;
  }

  const maxItemBytes = Math.max(1, ...visibleItems.map((item) => item.sizeBytes));
  const list = el("div", "items-list");
  for (const item of visibleItems) {
    const row = el("article", "item-row");
    if (state.highlightedItemId === item.id) {
      row.classList.add("is-highlighted");
    }

    const top = el("div", "item-top");
    const titleGroup = el("div", "item-title-group");
    titleGroup.append(
      el("span", `risk-badge risk-${item.risk}`, copy.risk[item.risk]),
      el("span", "item-name", item.name),
    );
    if (item.aiClassified) {
      const chip = el("span", "ai-chip");
      chip.append(icon(Sparkles, 10), document.createTextNode(state.language === "en" ? "AI" : "IA"));
      titleGroup.append(chip);
    }
    top.append(
      titleGroup,
      el("span", "item-size", formatBytes(item.sizeBytes, state.language)),
    );

    const dustBar = createDustBar(item.sizeBytes, maxItemBytes, riskToDustVariant(item.risk));
    const reason = el("p", "item-reason", item.reason);
    row.append(top, dustBar, reason);

    if (item.topExtensions.length > 0) {
      const extRow = el("div", "ext-row");
      for (const ext of item.topExtensions.slice(0, 4)) {
        extRow.append(
          el("span", "ext-chip", `${ext.ext} · ${formatBytes(ext.sizeBytes, state.language)}`),
        );
      }
      row.append(extRow);
    }

    const footer = el("div", "item-footer");
    const cleanableNote =
      item.cleanableBytes > 0 && item.risk !== "baixo"
        ? copy.items.cleanableNote(formatBytes(item.cleanableBytes, state.language))
        : "";
    const meta = el(
      "span",
      "item-meta tabular",
      `${copy.items.filesCount(formatInt(item.fileCount, state.language))} · ${formatDays(item.daysSinceModified, state.language)}${cleanableNote}`,
    );
    meta.title = item.path;

    const actions = el("div", "item-actions");
    const isExpanded = state.expandedItemIds.has(item.id);
    if (item.subitems.length > 0) {
      const expandBtn = el("button", "btn btn-ghost btn-xs tabular");
      expandBtn.type = "button";
      expandBtn.setAttribute("aria-expanded", String(isExpanded));
      expandBtn.append(
        icon(isExpanded ? ChevronDown : ChevronRight, 11),
        document.createTextNode(
          isExpanded ? copy.items.hide : copy.items.subfoldersCount(item.subitems.length),
        ),
      );
      expandBtn.addEventListener("click", () => {
        if (state.expandedItemIds.has(item.id)) {
          state.expandedItemIds.delete(item.id);
        } else {
          state.expandedItemIds.add(item.id);
        }
        renderItens();
      });
      actions.append(expandBtn);
    }

    const revealBtn = el("button", "btn btn-ghost btn-icon-xs");
    revealBtn.type = "button";
    revealBtn.disabled = isMutating();
    revealBtn.setAttribute("aria-label", copy.items.revealInFinder);
    revealBtn.title = copy.items.revealInFinder;
    revealBtn.append(icon(FolderOpen, 12));
    revealBtn.addEventListener("click", () => {
      void revealInFinder(item.path);
    });
    actions.append(revealBtn);

    const ignoreBtn = el("button", "btn btn-ghost btn-icon-xs");
    ignoreBtn.type = "button";
    ignoreBtn.disabled = isScanningAny() || isMutating();
    ignoreBtn.setAttribute("aria-label", copy.items.ignoreAlways);
    ignoreBtn.title = copy.items.ignoreAlways;
    ignoreBtn.append(icon(EyeOff, 12));
    ignoreBtn.addEventListener("click", () => {
      void ignorePath(item.path, item.name);
    });
    actions.append(ignoreBtn);
    const trashBtn = el("button", "btn btn-ghost btn-xs");
    trashBtn.type = "button";
    trashBtn.disabled = isScanningAny() || isMutating();
    trashBtn.append(icon(Trash2, 11), document.createTextNode(copy.items.trash));
    trashBtn.addEventListener("click", () => {
      state.pendingConfirm = {
        kind: "trash",
        originTab: "itens",
        isPermanent: false,
        paths: [item.path],
        getLabel: () =>
          getCopy(state.language).items.confirmTrashItem(
            item.name,
            formatBytes(item.sizeBytes, state.language),
          ),
      };
      updateConfirmBar();
    });
    actions.append(trashBtn);

    footer.append(meta, actions);
    row.append(footer);

    if (isExpanded && item.subitems.length > 0) {
      const subPanel = el("div", "subitems-panel");
      for (const sub of item.subitems) {
        const subRow = el("div", "subitem-row");
        const subTop = el("div", "subitem-top");
        const subTitle = el("div", "item-title-group");
        subTitle.append(
          el("span", `risk-badge risk-${sub.risk}`, copy.risk[sub.risk]),
          el("span", "subitem-name", sub.name),
        );
        subTitle.title = sub.path;
        subTop.append(
          subTitle,
          el("span", "item-size", formatBytes(sub.sizeBytes, state.language)),
        );

        const subDustBar = createDustBar(
          sub.sizeBytes,
          Math.max(1, item.sizeBytes),
          riskToDustVariant(sub.risk),
          true,
          item.sizeBytes,
        );

        const subBottom = el("div", "subitem-bottom");
        subBottom.append(
          el(
            "span",
            "subitem-reason",
            `${sub.reason} (${copy.items.filesCount(formatInt(sub.fileCount, state.language))} · ${formatDays(sub.daysSinceModified, state.language)})`,
          ),
        );

        const subActions = el("div", "subitem-actions");

        const subRevealBtn = el("button", "btn btn-ghost btn-icon-xs");
        subRevealBtn.type = "button";
        subRevealBtn.disabled = isMutating();
        subRevealBtn.setAttribute("aria-label", copy.items.revealInFinder);
        subRevealBtn.title = copy.items.revealInFinder;
        subRevealBtn.append(icon(FolderOpen, 11));
        subRevealBtn.addEventListener("click", () => {
          void revealInFinder(sub.path);
        });
        subActions.append(subRevealBtn);

        const subTrashBtn = el("button", "btn btn-ghost btn-xs");
        subTrashBtn.type = "button";
        subTrashBtn.disabled = isScanningAny() || isMutating();
        subTrashBtn.append(icon(Trash2, 11), document.createTextNode(copy.items.trash));
        subTrashBtn.addEventListener("click", () => {
          state.pendingConfirm = {
            kind: "trash",
            originTab: "itens",
            isPermanent: false,
            paths: [sub.path],
            getLabel: () =>
              getCopy(state.language).items.confirmTrashItem(
                sub.name,
                formatBytes(sub.sizeBytes, state.language),
              ),
          };
          updateConfirmBar();
        });
        subActions.append(subTrashBtn);

        subBottom.append(subActions);

        subRow.append(subTop, subDustBar, subBottom);
        subPanel.append(subRow);
      }
      row.append(subPanel);
    }

    list.append(row);
  }

  panelItens.append(list);
  list.scrollTop = prevScroll;
}

function renderAjustes(): void {
  const prevScroll = panelAjustes.scrollTop;
  const copy = getCopy(state.language);
  panelAjustes.replaceChildren();
  const cfg = state.aiConfig;
  if (!cfg) {
    const loading = el("div", "empty-state");
    loading.append(el("p", "muted", copy.settings.loading));
    panelAjustes.append(loading);
    return;
  }

  if (!state.settingsDraft) {
    state.settingsDraft = sanitizeAiConfigForSave(cfg);
  }
  const draft: AiConfig = state.settingsDraft;

  const form = el("form", "settings-form");
  form.addEventListener("submit", (event) => {
    event.preventDefault();
    void saveSettings(draft);
  });

  const langSection = el("section", "aqua-panel");
  const langField = el("div", "field");
  const langLabel = el("label", "field-label");
  langLabel.htmlFor = "language-select";
  const langTitle = el("span", "heading-with-icon");
  langTitle.append(icon(Globe, 13), document.createTextNode(copy.settings.languageLabel));
  langLabel.append(langTitle);

  const langSelect = el("select", "select");
  langSelect.id = "language-select";
  langSelect.name = "language";
  const languages: Array<{ value: AppLanguage; label: string }> = [
    { value: "pt-br", label: "Português (Brasil)" },
    { value: "en", label: "English" },
    { value: "es", label: "Español" },
  ];
  for (const langOpt of languages) {
    const optEl = el("option", undefined, langOpt.label);
    optEl.value = langOpt.value;
    optEl.selected = draft.language === langOpt.value;
    langSelect.append(optEl);
  }
  langSelect.addEventListener("change", () => {
    const chosen = langSelect.value as AppLanguage;
    draft.language = chosen;
    state.language = chosen;
    void saveLanguagePreference(chosen);
  });
  langField.append(langLabel, langSelect);
  langSection.append(langField, el("p", "muted", copy.settings.languageHint));

  const minSection = el("section", "aqua-panel");
  const minField = el("div", "field");
  const minLabelRow = el("label", "field-label");
  minLabelRow.htmlFor = "min-mb-number";
  minLabelRow.append(el("span", undefined, copy.settings.minFolderSize), el("span", "muted", "MB"));

  const rangeRow = el("div", "range-row");
  const range = el("input", "range-input");
  range.type = "range";
  range.min = "1";
  range.max = "500";
  range.value = String(draft.minMb);
  range.setAttribute("aria-label", copy.settings.minSizeAria);

  const numberInput = el("input", "input number-input");
  numberInput.id = "min-mb-number";
  numberInput.name = "minMb";
  numberInput.type = "number";
  numberInput.min = "1";
  numberInput.max = "10240";
  numberInput.value = String(draft.minMb);

  range.addEventListener("input", () => {
    draft.minMb = Math.max(1, Number(range.value) || 1);
    numberInput.value = String(draft.minMb);
  });
  numberInput.addEventListener("input", () => {
    draft.minMb = Math.max(1, Number(numberInput.value) || 1);
    range.value = String(Math.min(500, draft.minMb));
  });

  rangeRow.append(range, numberInput);
  minField.append(minLabelRow, rangeRow);
  minSection.append(minField);

  const aiSection = el("section", "aqua-panel");
  const providerField = el("div", "field");
  const providerLabel = el("label", "field-label");
  providerLabel.htmlFor = "ai-provider-select";
  const providerTitle = el("span", "heading-with-icon");
  providerTitle.append(icon(Cpu, 13), document.createTextNode(copy.settings.aiEngineLabel));
  providerLabel.append(providerTitle);

  const providerSelect = el("select", "select");
  providerSelect.id = "ai-provider-select";
  providerSelect.name = "provider";
  const options: Array<{ value: AiProvider; text: string }> = [
    { value: "ollama", text: copy.settings.providerOllama },
    { value: "externalApi", text: copy.settings.providerExternal },
    { value: "disabled", text: copy.settings.providerDisabled },
  ];
  for (const opt of options) {
    const optionEl = el("option", undefined, opt.text);
    optionEl.value = opt.value;
    optionEl.selected = draft.provider === opt.value;
    providerSelect.append(optionEl);
  }
  providerField.append(providerLabel, providerSelect);
  aiSection.append(providerField);

  const dynamicFields = el("div", "settings-form");

  function renderProviderInputs(): void {
    dynamicFields.replaceChildren();
    if (draft.provider === "ollama") {
      const endpointField = createTextField({
        id: "ollama-endpoint",
        label: copy.settings.ollamaEndpoint,
        type: "url",
        value: draft.ollamaEndpoint,
        placeholder: "http://127.0.0.1:11434…",
        onInput: (val) => {
          draft.ollamaEndpoint = val;
        },
      });

      const modelField = el("div", "field");
      const modelLabel = el("label", "field-label", copy.settings.localModel);
      modelLabel.htmlFor = "ollama-model-select";

      const modelSelect = el("select", "select");
      modelSelect.id = "ollama-model-select";
      modelSelect.name = "ollamaModel";

      const candidateModels = new Set<string>(["auto"]);
      if (draft.ollamaModel && draft.ollamaModel !== "auto") {
        candidateModels.add(draft.ollamaModel);
      }
      for (const installed of state.installedOllamaModels) {
        candidateModels.add(installed);
      }

      for (const modelName of candidateModels) {
        const opt = el(
          "option",
          undefined,
          modelName === "auto" ? copy.settings.autoModel : modelName,
        );
        opt.value = modelName;
        opt.selected = draft.ollamaModel === modelName;
        modelSelect.append(opt);
      }
      modelSelect.addEventListener("change", () => {
        draft.ollamaModel = modelSelect.value;
      });
      modelField.append(modelLabel, modelSelect);

      dynamicFields.append(
        endpointField,
        modelField,
        el("p", "muted", copy.settings.ollamaHint),
      );
    } else if (draft.provider === "externalApi") {
      const endpointField = createTextField({
        id: "external-endpoint",
        label: copy.settings.externalEndpoint,
        type: "url",
        value: draft.externalEndpoint,
        placeholder: "https://api.openai.com/v1/chat/completions…",
        onInput: (val) => {
          draft.externalEndpoint = val;
        },
      });
      const modelField = createTextField({
        id: "external-model",
        label: copy.settings.model,
        type: "text",
        value: draft.externalModel,
        placeholder: "gpt-4o-mini…",
        onInput: (val) => {
          draft.externalModel = val;
        },
      });
      const keyField = createTextField({
        id: "external-key",
        label: copy.settings.apiKey,
        type: "password",
        value: draft.externalApiKey,
        placeholder: "sk-…",
        onInput: (val) => {
          draft.externalApiKey = val;
        },
      });
      dynamicFields.append(endpointField, modelField, keyField);
    } else {
      dynamicFields.append(el("p", "muted", copy.settings.disabledHint));
    }
  }

  const actions = el("div", "action-row");
  const detectBtn = el(
    "button",
    "btn btn-secondary",
    state.detectingModels ? copy.settings.detecting : copy.settings.detectModels,
  );
  detectBtn.type = "button";
  detectBtn.disabled = state.detectingModels || draft.provider !== "ollama";
  detectBtn.addEventListener("click", () => {
    void detectOllamaModels(draft);
  });

  providerSelect.addEventListener("change", () => {
    draft.provider = providerSelect.value as AiProvider;
    state.settingsError = null;
    detectBtn.disabled = state.detectingModels || draft.provider !== "ollama";
    renderProviderInputs();
  });

  renderProviderInputs();
  aiSection.append(dynamicFields);

  if (state.settingsError) {
    aiSection.append(el("p", "inline-error", state.settingsError));
  }

  const systemSection = el("section", "aqua-panel");
  const systemTitle = el("span", "heading-with-icon");
  systemTitle.append(icon(SlidersHorizontal, 13), el("h2", "section-heading", copy.settings.systemSectionLabel));
  systemSection.append(systemTitle);

  const autostartRow = el("label", "toggle-row");
  autostartRow.htmlFor = "autostart-toggle";
  const autostartInfo = el("span", "toggle-info");
  autostartInfo.append(
    el("span", "toggle-title", copy.settings.autostartLabel),
    el("span", "toggle-hint", copy.settings.autostartHint),
  );
  const autostartSwitch = el("input", "toggle-switch");
  autostartSwitch.id = "autostart-toggle";
  autostartSwitch.type = "checkbox";
  autostartSwitch.checked = state.autostartEnabled;
  autostartSwitch.disabled = state.updatingAutostart;
  autostartSwitch.addEventListener("change", () => {
    void toggleAutostart(autostartSwitch.checked);
  });
  autostartRow.append(autostartInfo, autostartSwitch);

  const alertRow = el("label", "toggle-row");
  alertRow.htmlFor = "low-disk-alert-toggle";
  const alertInfo = el("span", "toggle-info");
  alertInfo.append(
    el("span", "toggle-title", copy.settings.lowDiskAlertLabel),
    el("span", "toggle-hint", copy.settings.lowDiskAlertHint),
  );
  const alertSwitch = el("input", "toggle-switch");
  alertSwitch.id = "low-disk-alert-toggle";
  alertSwitch.type = "checkbox";
  alertSwitch.checked = draft.lowDiskAlertEnabled;
  alertSwitch.addEventListener("change", () => {
    draft.lowDiskAlertEnabled = alertSwitch.checked;
    void saveLowDiskAlertPreference(alertSwitch.checked);
  });
  alertRow.append(alertInfo, alertSwitch);
  systemSection.append(autostartRow, alertRow);

  const allowlistSection = el("section", "aqua-panel");
  const allowlistHead = el("div", "surface-row");
  const allowlistTitle = el("span", "heading-with-icon");
  allowlistTitle.append(icon(EyeOff, 13), el("h2", "section-heading", copy.settings.allowlistLabel));
  allowlistHead.append(
    allowlistTitle,
    el("span", "snapshot-count-pill tabular", formatInt(cfg.ignoredPaths.length, state.language)),
  );
  allowlistSection.append(allowlistHead);

  if (cfg.ignoredPaths.length === 0) {
    allowlistSection.append(el("p", "muted", copy.settings.allowlistEmpty));
  } else {
    allowlistSection.append(el("p", "muted", copy.settings.allowlistHint));
    const ignoredList = el("div", "ignored-list");
    for (const ignoredPath of cfg.ignoredPaths) {
      const row = el("div", "ignored-row");
      const info = el("div", "ignored-info");
      const folderName = ignoredPath.split("/").filter(Boolean).pop() ?? ignoredPath;
      const nameEl = el("span", "ignored-name", folderName);
      nameEl.title = ignoredPath;
      const pathEl = el("span", "ignored-path", ignoredPath);
      pathEl.title = ignoredPath;
      info.append(nameEl, pathEl);

      const removeBtn = el("button", "btn btn-ghost btn-xs", copy.settings.allowlistRestore);
      removeBtn.type = "button";
      removeBtn.addEventListener("click", () => {
        void unignorePath(ignoredPath);
      });

      row.append(info, removeBtn);
      ignoredList.append(row);
    }
    allowlistSection.append(ignoredList);
  }

  const saveBtn = el(
    "button",
    "btn btn-primary",
    state.savingSettings ? copy.settings.saving : copy.settings.saveSettings,
  );
  saveBtn.type = "submit";
  saveBtn.disabled = state.savingSettings;

  actions.append(detectBtn, saveBtn);
  form.append(langSection, systemSection, minSection, allowlistSection, aiSection, actions);
  panelAjustes.append(form);
  panelAjustes.scrollTop = prevScroll;
}

function createTextField(params: {
  id: string;
  label: string;
  type: "text" | "url" | "password";
  value: string;
  placeholder: string;
  onInput: (value: string) => void;
}): HTMLElement {
  const wrapper = el("div", "field");
  const label = el("label", "field-label", params.label);
  label.htmlFor = params.id;

  const input = el("input", "input");
  input.id = params.id;
  input.name = params.id;
  input.type = params.type;
  input.value = params.value;
  input.placeholder = params.placeholder;
  input.autocomplete = "off";
  input.spellcheck = false;
  input.addEventListener("input", () => {
    params.onInput(input.value);
  });

  wrapper.append(label, input);
  return wrapper;
}

function collectSafeTargets(): { paths: string[]; totalBytes: number } {
  if (!state.summary) {
    return { paths: [], totalBytes: 0 };
  }
  const paths: string[] = [];
  let totalBytes = 0;

  for (const item of state.summary.items) {
    const hasHighSub = item.subitems.some((sub) => sub.risk === "alto");
    if (item.risk === "baixo" && !hasHighSub) {
      paths.push(item.path);
      totalBytes += item.sizeBytes;
      continue;
    }

    for (const sub of item.subitems) {
      if (sub.safeToClean) {
        paths.push(sub.path);
        totalBytes += sub.sizeBytes;
      }
    }
  }

  return { paths, totalBytes };
}

function requestCleanSafeTargets(): void {
  const safeTargets = collectSafeTargets();
  if (safeTargets.paths.length === 0) {
    return;
  }
  state.pendingConfirm = {
    kind: "trash",
    originTab: state.activeTab,
    isPermanent: false,
    paths: safeTargets.paths,
    getLabel: () =>
      getCopy(state.language).items.confirmTrashSafe(
        safeTargets.paths.length,
        formatBytes(safeTargets.totalBytes, state.language),
      ),
  };
  updateConfirmBar();
}

async function revealInFinder(path: string): Promise<void> {
  try {
    await invoke("reveal_in_finder", { path });
  } catch (err) {
    showToast(getCopy(state.language).toasts.revealError(String(err)));
  }
}

async function refreshSnapshots(silent: boolean): Promise<void> {
  if (state.loadingSnapshots) {
    return;
  }
  state.loadingSnapshots = true;
  if (state.activeTab === "resumo") {
    renderResumo();
  }

  try {
    const result = await invoke<SnapshotListResult>("list_apfs_snapshots");
    state.snapshots = result.snapshots;
    state.snapshotsSupported = result.supported;
    state.snapshotsNote = result.note;
    state.snapshotsLoaded = true;
    if (result.snapshots.length === 0) {
      state.snapshotsExpanded = false;
    }
    invalidateStaleSnapshotConfirm();
  } catch (err) {
    state.snapshotsLoaded = true;
    if (!silent) {
      showToast(getCopy(state.language).toasts.snapshotsLoadError(String(err)));
    }
  } finally {
    state.loadingSnapshots = false;
    if (state.activeTab === "resumo") {
      renderResumo();
    }
  }
}

async function executeDeleteSnapshots(dateTokens: string[]): Promise<void> {
  if (isMutating() || isScanningAny() || dateTokens.length === 0) {
    state.pendingConfirm = null;
    updateConfirmBar();
    return;
  }

  state.deletingSnapshots = true;
  renderActiveTab();

  try {
    const result = await invoke<SnapshotDeleteResult>("delete_apfs_snapshots", { dateTokens });
    await Promise.allSettled([refreshSnapshots(true), refreshDiskHealth()]);

    const toasts = getCopy(state.language).toasts;
    if (result.cancelled && result.deletedTokens.length === 0) {
      showToast(toasts.snapshotsCancelled);
    } else if (result.failed.length > 0 && result.deletedTokens.length === 0) {
      showToast(toasts.snapshotsError(result.failed[0].reason));
    } else if (result.failed.length > 0) {
      showToast(toasts.snapshotsPartial(result.deletedTokens.length, result.failed.length));
    } else if (result.deletedTokens.length > 0) {
      showToast(toasts.snapshotsDeleted(result.deletedTokens.length));
    }
  } catch (err) {
    showToast(getCopy(state.language).toasts.snapshotsError(String(err)));
  } finally {
    state.deletingSnapshots = false;
    state.pendingConfirm = null;
    renderActiveTab();
  }
}

async function refreshDiskHealth(): Promise<void> {
  try {
    state.diskHealth = await invoke<DiskHealth>("get_disk_health");
  } catch (err) {
    showToast(getCopy(state.language).toasts.diskError(String(err)));
  }
}

async function runScan(): Promise<void> {
  if (isScanningAny() || isMutating()) {
    return;
  }
  const copy = getCopy(state.language);
  state.scanning = true;
  state.scanProgressText = copy.summary.startingScan;
  state.pendingConfirm = null;
  renderActiveTab();

  const channel = new Channel<ScanEvent>();
  channel.onmessage = (msg) => {
    if (msg.event === "progress") {
      state.scanProgressText = /^(?:IA|AI) \(/.test(msg.data.currentPath)
        ? msg.data.currentPath
        : getCopy(state.language).summary.scanningPath(msg.data.currentPath);
      if (state.activeTab === "resumo") {
        const descEl = document.getElementById("scan-progress-desc");
        if (descEl) {
          descEl.textContent = state.scanProgressText;
        }
      }
    }
  };

  try {
    const summary = await invoke<ScanSummary>("scan_system", {
      minMb: state.aiConfig?.minMb ?? null,
      onEvent: channel,
    });
    state.summary = summary;
    const validIds = new Set(summary.items.map((item) => item.id));
    for (const id of state.expandedItemIds) {
      if (!validIds.has(id)) {
        state.expandedItemIds.delete(id);
      }
    }
    if (state.highlightedItemId && !validIds.has(state.highlightedItemId)) {
      state.highlightedItemId = null;
    }
    const [diskResult, configResult, snapshotResult] = await Promise.allSettled([
      invoke<DiskHealth>("get_disk_health"),
      invoke<AiConfig>("get_ai_config"),
      invoke<SnapshotListResult>("list_apfs_snapshots"),
    ]);
    if (diskResult.status === "fulfilled") {
      state.diskHealth = diskResult.value;
    }
    if (configResult.status === "fulfilled") {
      state.aiConfig = configResult.value;
      state.language = configResult.value.language;
    }
    if (snapshotResult.status === "fulfilled") {
      state.snapshots = snapshotResult.value.snapshots;
      state.snapshotsSupported = snapshotResult.value.supported;
      state.snapshotsNote = snapshotResult.value.note;
      state.snapshotsLoaded = true;
    }
    showToast(
      getCopy(state.language).toasts.scanComplete(formatBytes(summary.totalBytes, state.language)),
    );
  } catch (err) {
    showToast(getCopy(state.language).toasts.scanFailed(String(err)));
  } finally {
    state.scanning = false;
    state.scanProgressText = "";
    renderActiveTab();
  }
}

function recomputeItemCleanableBytes(item: ScanItem): number {
  const hasHighSub = item.subitems.some((sub) => sub.risk === "alto");
  if (item.risk === "baixo" && !hasHighSub) {
    return item.sizeBytes;
  }
  return item.subitems
    .filter((sub) => sub.safeToClean)
    .reduce((sum, sub) => sum + sub.sizeBytes, 0);
}

function normalizeMacPath(rawPath: string): string {
  return rawPath.replace(/\/+$/, "").toLowerCase();
}

function sanitizeAiConfigForSave(cfg: AiConfig): AiConfig {
  const { devProjectRoots: _roots, devRootsInitialized: _init, ...clean } = cfg;
  return clean;
}

function invalidateStaleSnapshotConfirm(): void {
  if (state.pendingConfirm?.kind !== "snapshot" || isMutating()) {
    return;
  }
  const present = new Set(state.snapshots.map((snap) => snap.dateToken));
  if (state.pendingConfirm.dateTokens.some((token) => !present.has(token))) {
    state.pendingConfirm = null;
    updateConfirmBar();
  }
}

function isRemovedOrCovered(candidateNorm: string, removedNorms: string[]): boolean {
  for (const removed of removedNorms) {
    if (candidateNorm === removed || candidateNorm.startsWith(`${removed}/`)) {
      return true;
    }
  }
  return false;
}

function reconcileSummaryAfterTrash(
  trashedPaths: string[],
  cleanedItems?: CleanedPathItem[],
): void {
  if (!state.summary) {
    return;
  }
  const entryMap = new Map<string, number>();
  if (cleanedItems) {
    for (const item of cleanedItems) {
      const norm = normalizeMacPath(item.path);
      if (!norm) {
        continue;
      }
      const bytes = Number.isFinite(item.freedBytes) && item.freedBytes > 0 ? item.freedBytes : 0;
      entryMap.set(norm, (entryMap.get(norm) ?? 0) + bytes);
    }
  }
  for (const rawPath of trashedPaths) {
    const norm = normalizeMacPath(rawPath);
    if (norm && !entryMap.has(norm)) {
      entryMap.set(norm, 0);
    }
  }
  if (entryMap.size === 0) {
    return;
  }

  const removedNorms = Array.from(entryMap.keys());
  const removedEntries = Array.from(entryMap.entries()).map(([norm, freedBytes]) => ({
    norm,
    freedBytes,
  }));
  const updatedItems: ScanItem[] = [];

  for (const item of state.summary.items) {
    const itemNorm = normalizeMacPath(item.path);
    if (isRemovedOrCovered(itemNorm, removedNorms)) {
      state.expandedItemIds.delete(item.id);
      if (state.highlightedItemId === item.id) {
        state.highlightedItemId = null;
      }
      continue;
    }

    const remainingSubs: ScanItem["subitems"] = [];
    const matchedSubNorms: string[] = [];
    let removedBytesFromSubs = 0;
    let removedSubFiles = 0;

    for (const sub of item.subitems) {
      const subNorm = normalizeMacPath(sub.path);
      if (isRemovedOrCovered(subNorm, removedNorms)) {
        removedBytesFromSubs += sub.sizeBytes;
        removedSubFiles += sub.fileCount;
        matchedSubNorms.push(subNorm);
        continue;
      }

      let childFreed = 0;
      for (const entry of removedEntries) {
        if (entry.norm.startsWith(`${subNorm}/`) && entry.freedBytes > 0) {
          childFreed += entry.freedBytes;
        }
      }

      if (childFreed > 0) {
        matchedSubNorms.push(subNorm);
        const nextSubSize = Math.max(0, sub.sizeBytes - childFreed);
        removedBytesFromSubs += sub.sizeBytes - nextSubSize;
        if (nextSubSize === 0) {
          removedSubFiles += sub.fileCount;
          continue;
        }
        remainingSubs.push({
          ...sub,
          sizeBytes: nextSubSize,
        });
      } else {
        remainingSubs.push(sub);
      }
    }

    let unmatchedChildFreed = 0;
    for (const entry of removedEntries) {
      if (
        entry.norm.startsWith(`${itemNorm}/`) &&
        entry.freedBytes > 0 &&
        !isRemovedOrCovered(entry.norm, matchedSubNorms)
      ) {
        unmatchedChildFreed += entry.freedBytes;
      }
    }

    const totalRemovedBytes = removedBytesFromSubs + unmatchedChildFreed;
    const nextSize = Math.max(0, item.sizeBytes - totalRemovedBytes);
    if (nextSize === 0 && remainingSubs.length === 0 && totalRemovedBytes > 0) {
      state.expandedItemIds.delete(item.id);
      if (state.highlightedItemId === item.id) {
        state.highlightedItemId = null;
      }
      continue;
    }

    const nextItem: ScanItem = {
      ...item,
      sizeBytes: nextSize,
      fileCount: Math.max(0, item.fileCount - removedSubFiles),
      subitems: remainingSubs,
      cleanableBytes: 0,
    };
    nextItem.cleanableBytes = recomputeItemCleanableBytes(nextItem);
    updatedItems.push(nextItem);
  }

  updatedItems.sort((a, b) => b.sizeBytes - a.sizeBytes || a.name.localeCompare(b.name));

  state.summary = {
    ...state.summary,
    items: updatedItems,
    itemCount: updatedItems.length,
    totalBytes: updatedItems.reduce((sum, item) => sum + item.sizeBytes, 0),
    safeBytes: updatedItems.reduce((sum, item) => sum + item.cleanableBytes, 0),
  };
}

async function executeTrash(paths: string[]): Promise<void> {
  if (isMutating() || isScanningAny() || paths.length === 0) {
    state.pendingConfirm = null;
    updateConfirmBar();
    return;
  }

  state.trashing = true;
  renderActiveTab();

  try {
    const result = await invoke<TrashResult>("trash_items", { paths });
    reconcileSummaryAfterTrash(result.trashedPaths);

    await refreshDiskHealth();

    const toasts = getCopy(state.language).toasts;
    if (result.failedPaths.length > 0 && result.trashedPaths.length === 0) {
      showToast(toasts.trashNoneMoved(result.failedPaths[0].reason));
    } else if (result.failedPaths.length > 0) {
      showToast(toasts.trashPartial(result.trashedPaths.length, result.failedPaths.length));
    } else {
      showToast(toasts.trashMoved(formatBytes(result.freedBytes, state.language)));
    }
  } catch (err) {
    showToast(getCopy(state.language).toasts.trashError(String(err)));
  } finally {
    state.trashing = false;
    state.pendingConfirm = null;
    renderActiveTab();
  }
}

async function checkUpdates(): Promise<void> {
  if (state.checkingUpdate || state.installingUpdate) {
    return;
  }
  state.checkingUpdate = true;
  renderActiveTab();

  try {
    const result = await invoke<UpdateCheckResult>("check_for_updates");
    state.updateResult = result;
    const toasts = getCopy(state.language).toasts;
    if (result.updateAvailable && result.latestVersion) {
      showToast(toasts.updateAvailable(result.latestVersion, result.currentVersion));
    } else if (result.latestVersion) {
      showToast(toasts.updateLatest(result.currentVersion));
    } else {
      showToast(result.message);
    }
  } catch (err) {
    showToast(getCopy(state.language).toasts.updateError(String(err)));
  } finally {
    state.checkingUpdate = false;
    renderActiveTab();
  }
}

async function installUpdate(): Promise<void> {
  if (state.installingUpdate || state.checkingUpdate) {
    return;
  }
  const copy = getCopy(state.language);
  state.installingUpdate = true;
  state.updateProgressText = copy.header.installingUpdate;
  updateChrome();
  showToast(copy.toasts.updateInstalling);

  const channel = new Channel<UpdateInstallEvent>();
  channel.onmessage = (msg) => {
    const activeCopy = getCopy(state.language);
    if (msg.event === "downloading") {
      if (msg.data.totalBytes && msg.data.totalBytes > 0) {
        const pct = Math.min(100, Math.round((msg.data.downloadedBytes / msg.data.totalBytes) * 100));
        state.updateProgressText = activeCopy.header.downloadingPct(pct);
      } else {
        state.updateProgressText = activeCopy.header.installingUpdate;
      }
      updateChrome();
    } else if (msg.event === "installing") {
      state.updateProgressText = activeCopy.header.installingUpdate;
      updateChrome();
    } else if (msg.event === "restarting") {
      showToast(activeCopy.toasts.updateRestarting);
    }
  };

  try {
    await invoke("install_update", { onEvent: channel });
  } catch (err) {
    showToast(getCopy(state.language).toasts.updateError(String(err)));
  } finally {
    state.installingUpdate = false;
    state.updateProgressText = "";
    updateChrome();
  }
}

async function saveLanguagePreference(language: AppLanguage): Promise<void> {
  if (!state.aiConfig) {
    renderActiveTab();
    return;
  }
  try {
    const saved = await invoke<AiConfig>("set_ai_config", {
      config: sanitizeAiConfigForSave({ ...state.aiConfig, language }),
    });
    state.aiConfig = saved;
    state.language = saved.language;
    if (state.settingsDraft) {
      state.settingsDraft.language = saved.language;
    }
    renderActiveTab();
  } catch {
    renderActiveTab();
  }
}

async function saveLowDiskAlertPreference(enabled: boolean): Promise<void> {
  if (!state.aiConfig) {
    return;
  }
  try {
    const saved = await invoke<AiConfig>("set_ai_config", {
      config: sanitizeAiConfigForSave({ ...state.aiConfig, lowDiskAlertEnabled: enabled }),
    });
    state.aiConfig = saved;
    if (state.settingsDraft) {
      state.settingsDraft.lowDiskAlertEnabled = saved.lowDiskAlertEnabled;
    }
  } catch (err) {
    if (state.settingsDraft) {
      state.settingsDraft.lowDiskAlertEnabled = state.aiConfig.lowDiskAlertEnabled;
    }
    showToast(getCopy(state.language).toasts.updateError(String(err)));
  } finally {
    renderAjustes();
  }
}

async function toggleAutostart(enabled: boolean): Promise<void> {
  if (state.updatingAutostart) {
    return;
  }
  state.updatingAutostart = true;
  renderAjustes();

  try {
    state.autostartEnabled = await invoke<boolean>("set_autostart_enabled", { enabled });
  } catch (err) {
    showToast(getCopy(state.language).toasts.autostartError(String(err)));
  } finally {
    state.updatingAutostart = false;
    renderAjustes();
  }
}

async function ignorePath(path: string, displayName: string): Promise<void> {
  try {
    const saved = await invoke<AiConfig>("add_ignored_path", { path });
    state.aiConfig = saved;
    if (state.settingsDraft) {
      state.settingsDraft.ignoredPaths = [...saved.ignoredPaths];
    }
    if (state.summary) {
      const remaining = state.summary.items.filter((item) => item.path !== path);
      for (const item of state.summary.items) {
        if (item.path === path) {
          state.expandedItemIds.delete(item.id);
          if (state.highlightedItemId === item.id) {
            state.highlightedItemId = null;
          }
        }
      }
      state.summary = {
        ...state.summary,
        items: remaining,
        itemCount: remaining.length,
        totalBytes: remaining.reduce((sum, item) => sum + item.sizeBytes, 0),
        safeBytes: remaining.reduce((sum, item) => sum + item.cleanableBytes, 0),
      };
    }
    if (state.pendingConfirm?.kind === "trash" && !isMutating()) {
      const ignoredNorm = normalizeMacPath(path);
      const referencesIgnored = state.pendingConfirm.paths.some((p) => {
        const pNorm = normalizeMacPath(p);
        return (
          pNorm === ignoredNorm ||
          pNorm.startsWith(`${ignoredNorm}/`) ||
          ignoredNorm.startsWith(`${pNorm}/`)
        );
      });
      if (referencesIgnored) {
        state.pendingConfirm = null;
        updateConfirmBar();
      }
    }
    showToast(getCopy(state.language).toasts.pathIgnored(displayName));
    renderActiveTab();
  } catch (err) {
    showToast(getCopy(state.language).toasts.ignoreError(String(err)));
  }
}

async function unignorePath(path: string): Promise<void> {
  try {
    const saved = await invoke<AiConfig>("remove_ignored_path", { path });
    state.aiConfig = saved;
    if (state.settingsDraft) {
      state.settingsDraft.ignoredPaths = [...saved.ignoredPaths];
    }
    const label = path.split("/").filter(Boolean).pop() ?? path;
    showToast(getCopy(state.language).toasts.pathUnignored(label));
    renderAjustes();
  } catch (err) {
    showToast(getCopy(state.language).toasts.ignoreError(String(err)));
  }
}

async function saveSettings(draft: AiConfig): Promise<void> {
  const cleanDraft = sanitizeAiConfigForSave(draft);
  state.savingSettings = true;
  state.settingsError = null;
  state.settingsDraft = { ...cleanDraft };
  renderAjustes();

  try {
    const saved = await invoke<AiConfig>("set_ai_config", { config: cleanDraft });
    state.aiConfig = saved;
    state.settingsDraft = sanitizeAiConfigForSave(saved);
    state.language = saved.language;
    showToast(getCopy(state.language).toasts.settingsSaved);
  } catch (err) {
    state.settingsError = String(err);
  } finally {
    state.savingSettings = false;
    renderActiveTab();
  }
}

async function detectOllamaModels(draft: AiConfig): Promise<void> {
  const cleanDraft = sanitizeAiConfigForSave(draft);
  state.detectingModels = true;
  state.settingsError = null;
  state.settingsDraft = { ...cleanDraft };
  renderAjustes();

  try {
    const saved = await invoke<AiConfig>("set_ai_config", { config: cleanDraft });
    state.aiConfig = saved;
    state.settingsDraft = sanitizeAiConfigForSave(saved);
    const models = await invoke<string[]>("list_ollama_models");
    state.installedOllamaModels = models;
    if (models.length > 0 && (!saved.ollamaModel || !models.includes(saved.ollamaModel))) {
      const updated = await invoke<AiConfig>("set_ai_config", {
        config: sanitizeAiConfigForSave({ ...saved, ollamaModel: models[0] }),
      });
      state.aiConfig = updated;
      state.settingsDraft = sanitizeAiConfigForSave(updated);
    }
    const copy = getCopy(state.language);
    showToast(
      models.length > 0 ? copy.toasts.modelsDetected(models.join(", ")) : copy.toasts.noModelsFound,
    );
  } catch (err) {
    state.settingsError = String(err);
  } finally {
    state.detectingModels = false;
    renderActiveTab();
  }
}

function wireEvents(): void {
  for (const btn of tabButtons) {
    btn.addEventListener("click", () => {
      const target = btn.dataset.tab as TabId | undefined;
      if (target) {
        setActiveTab(target);
      }
    });

    btn.addEventListener("keydown", (event) => {
      if (event.key !== "ArrowRight" && event.key !== "ArrowLeft") {
        return;
      }
      event.preventDefault();
      const idx = tabButtons.indexOf(btn);
      const delta = event.key === "ArrowRight" ? 1 : -1;
      const next = tabButtons[(idx + delta + tabButtons.length) % tabButtons.length];
      next.focus();
      const target = next.dataset.tab as TabId | undefined;
      if (target) {
        setActiveTab(target);
      }
    });
  }

  updateHeaderBtn.addEventListener("click", () => {
    if (state.installingUpdate || state.checkingUpdate) {
      return;
    }
    if (state.updateResult?.updateAvailable) {
      void installUpdate();
    } else {
      void checkUpdates();
    }
  });

  rescanIconBtn.addEventListener("click", () => {
    void runScan();
  });

  confirmCancelBtn.addEventListener("click", () => {
    state.pendingConfirm = null;
    updateConfirmBar();
  });

  confirmAcceptBtn.addEventListener("click", () => {
    const pending = state.pendingConfirm;
    if (!pending) {
      return;
    }
    if (pending.kind === "trash") {
      void executeTrash(pending.paths);
    } else if (pending.kind === "snapshot") {
      void executeDeleteSnapshots(pending.dateTokens);
    } else {
      void (async () => {
        try {
          await pending.onConfirm();
        } finally {
          if (state.pendingConfirm === pending) {
            state.pendingConfirm = null;
          }
          updateChrome();
          updateConfirmBar();
        }
      })();
    }
  });

  window.addEventListener("keydown", (event) => {
    if (event.key === "Escape" && state.pendingConfirm && !isMutating()) {
      state.pendingConfirm = null;
      updateConfirmBar();
    }
  });
}

async function init(): Promise<void> {
  wireEvents();
  state.loadingSnapshots = true;
  renderActiveTab();

  const [versionResult, diskResult, configResult, snapshotResult, autostartResult] =
    await Promise.allSettled([
      invoke<string>("get_app_version"),
      invoke<DiskHealth>("get_disk_health"),
      invoke<AiConfig>("get_ai_config"),
      invoke<SnapshotListResult>("list_apfs_snapshots"),
      invoke<boolean>("get_autostart_enabled"),
    ]);

  if (versionResult.status === "fulfilled") {
    state.appVersion = versionResult.value;
  }
  if (diskResult.status === "fulfilled") {
    state.diskHealth = diskResult.value;
  }
  if (configResult.status === "fulfilled") {
    state.aiConfig = configResult.value;
    state.language = configResult.value.language;
  }
  if (snapshotResult.status === "fulfilled") {
    state.snapshots = snapshotResult.value.snapshots;
    state.snapshotsSupported = snapshotResult.value.supported;
    state.snapshotsNote = snapshotResult.value.note;
  }
  if (autostartResult.status === "fulfilled") {
    state.autostartEnabled = autostartResult.value;
  }
  state.snapshotsLoaded = true;
  state.loadingSnapshots = false;

  renderActiveTab();
}

void init();
