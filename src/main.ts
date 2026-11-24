import { Channel, invoke } from "@tauri-apps/api/core";
import {
  ArrowUpCircle,
  ChevronDown,
  ChevronRight,
  Cpu,
  FolderSearch,
  Globe,
  HardDrive,
  RefreshCw,
  ScanSearch,
  SlidersHorizontal,
  Sparkles,
  Trash2,
  createElement,
  type IconNode,
} from "lucide";
import { formatBytes, formatDays, formatInt, getCopy } from "./i18n";
import type {
  AiConfig,
  AiProvider,
  AppLanguage,
  DiskHealth,
  RiskLevel,
  ScanEvent,
  ScanItem,
  ScanSummary,
  TrashResult,
  UpdateCheckResult,
} from "./types";

type TabId = "resumo" | "itens" | "ajustes";
type RiskFilter = "todos" | RiskLevel;

interface PendingTrashAction {
  paths: string[];
  getLabel: () => string;
}

interface AppViewState {
  activeTab: TabId;
  appVersion: string;
  language: AppLanguage;
  updateResult: UpdateCheckResult | null;
  checkingUpdate: boolean;
  diskHealth: DiskHealth | null;
  summary: ScanSummary | null;
  aiConfig: AiConfig | null;
  settingsDraft: AiConfig | null;
  installedOllamaModels: string[];
  expandedItemIds: Set<string>;
  scanning: boolean;
  scanProgressText: string;
  trashing: boolean;
  riskFilter: RiskFilter;
  highlightedItemId: string | null;
  pendingConfirm: PendingTrashAction | null;
  settingsError: string | null;
  detectingModels: boolean;
  savingSettings: boolean;
}

const state: AppViewState = {
  activeTab: "resumo",
  appVersion: "0.1.5",
  language: "pt-br",
  updateResult: null,
  checkingUpdate: false,
  diskHealth: null,
  summary: null,
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
const panelAjustes = mustEl<HTMLElement>("#panel-ajustes");
const brandIconSlot = mustEl<HTMLElement>("#brand-icon-slot");
const versionTag = mustEl<HTMLElement>("#version-tag");
const updateHeaderBtn = mustEl<HTMLButtonElement>("#update-header-btn");
const rescanIconBtn = mustEl<HTMLButtonElement>("#rescan-icon-btn");
const tabListEl = mustEl<HTMLElement>("#tab-list");
const tabResumoBtn = mustEl<HTMLButtonElement>("#tab-resumo");
const tabItensBtn = mustEl<HTMLButtonElement>("#tab-itens");
const tabAjustesBtn = mustEl<HTMLButtonElement>("#tab-ajustes");
const confirmBar = mustEl<HTMLElement>("#confirm-bar");
const confirmMessage = mustEl<HTMLElement>("#confirm-message");
const confirmCancelBtn = mustEl<HTMLButtonElement>("#confirm-cancel-btn");
const confirmAcceptBtn = mustEl<HTMLButtonElement>("#confirm-accept-btn");
const toastEl = mustEl<HTMLElement>("#toast");
const tabButtons = [tabResumoBtn, tabItensBtn, tabAjustesBtn];

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

function setActiveTab(tab: TabId): void {
  state.activeTab = tab;
  for (const btn of tabButtons) {
    const isMatch = btn.dataset.tab === tab;
    btn.classList.toggle("is-active", isMatch);
    btn.setAttribute("aria-selected", String(isMatch));
  }
  panelResumo.hidden = tab !== "resumo";
  panelItens.hidden = tab !== "itens";
  panelAjustes.hidden = tab !== "ajustes";
  if (tab === "ajustes" && !state.trashing) {
    state.pendingConfirm = null;
  }
  renderActiveTab();
}

function updateChrome(): void {
  const copy = getCopy(state.language);
  document.documentElement.lang = copy.htmlLang;
  tabListEl.setAttribute("aria-label", copy.tabs.navLabel);

  if (brandIconSlot.childElementCount === 0) {
    brandIconSlot.append(icon(Sparkles, 14));
  }

  versionTag.textContent = `v${state.appVersion}`;
  rescanIconBtn.disabled = state.scanning || state.trashing;
  rescanIconBtn.classList.toggle("is-spinning", state.scanning);
  rescanIconBtn.setAttribute("aria-label", copy.header.rescanAria);
  rescanIconBtn.title = copy.summary.scanDisk;
  if (rescanIconBtn.childElementCount === 0) {
    rescanIconBtn.append(icon(RefreshCw, 14));
  }

  updateHeaderBtn.disabled = state.checkingUpdate;
  updateHeaderBtn.setAttribute("aria-label", copy.header.checkUpdateTitle);
  updateHeaderBtn.replaceChildren(icon(ArrowUpCircle, 12));
  if (state.checkingUpdate) {
    updateHeaderBtn.append(document.createTextNode(copy.header.checkingUpdate));
    updateHeaderBtn.classList.remove("has-update");
    updateHeaderBtn.title = copy.header.checkingUpdate;
  } else if (state.updateResult?.updateAvailable && state.updateResult.latestVersion) {
    updateHeaderBtn.append(document.createTextNode(`v${state.updateResult.latestVersion}`));
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

  tabResumoBtn.replaceChildren(icon(HardDrive, 13), el("span", undefined, copy.tabs.resumo));

  const count = state.summary?.items.length ?? 0;
  const countBadge = el("span", "tab-count", formatInt(count, state.language));
  countBadge.hidden = count === 0;
  tabItensBtn.replaceChildren(
    icon(FolderSearch, 13),
    el("span", undefined, copy.tabs.itens),
    countBadge,
  );

  tabAjustesBtn.replaceChildren(
    icon(SlidersHorizontal, 13),
    el("span", undefined, copy.tabs.ajustes),
  );
}

function updateConfirmBar(): void {
  const copy = getCopy(state.language);
  confirmCancelBtn.textContent = copy.items.cancel;

  if (!state.pendingConfirm) {
    confirmBar.hidden = true;
    confirmAcceptBtn.disabled = false;
    confirmCancelBtn.disabled = false;
    confirmAcceptBtn.replaceChildren(icon(Trash2, 12), document.createTextNode(copy.items.moveToTrash));
    return;
  }
  confirmMessage.textContent = state.pendingConfirm.getLabel();
  confirmAcceptBtn.disabled = state.trashing || state.scanning;
  confirmCancelBtn.disabled = state.trashing;
  confirmAcceptBtn.replaceChildren(
    icon(Trash2, 12),
    document.createTextNode(state.trashing ? copy.items.moving : copy.items.moveToTrash),
  );
  confirmBar.hidden = false;
}

function renderActiveTab(): void {
  syncAtmosphere();
  updateChrome();
  updateConfirmBar();
  if (state.activeTab === "resumo") {
    renderResumo();
  } else if (state.activeTab === "itens") {
    renderItens();
  } else {
    renderAjustes();
  }
}

function renderResumo(): void {
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

  const actionRow = el("div", "action-row");
  const scanBtn = el("button", "btn btn-primary");
  scanBtn.type = "button";
  scanBtn.disabled = state.scanning || state.trashing;
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
    cleanSafeBtn.disabled = state.trashing;
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
  panelResumo.append(diskSection, scanSection);

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
}

function renderItens(): void {
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
    btn.disabled = state.scanning;
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
    pill.type = "button";
    pill.classList.toggle("is-active", state.riskFilter === filter.id);
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
    cleanAllBtn.disabled = state.scanning || state.trashing;
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

    const reason = el("p", "item-reason", item.reason);
    row.append(top, reason);

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

    const trashBtn = el("button", "btn btn-ghost btn-xs");
    trashBtn.type = "button";
    trashBtn.disabled = state.scanning || state.trashing;
    trashBtn.append(icon(Trash2, 11), document.createTextNode(copy.items.trash));
    trashBtn.addEventListener("click", () => {
      state.pendingConfirm = {
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

        const subBottom = el("div", "subitem-bottom");
        subBottom.append(
          el(
            "span",
            "subitem-reason",
            `${sub.reason} (${copy.items.filesCount(formatInt(sub.fileCount, state.language))} · ${formatDays(sub.daysSinceModified, state.language)})`,
          ),
        );

        const subTrashBtn = el("button", "btn btn-ghost btn-xs");
        subTrashBtn.type = "button";
        subTrashBtn.disabled = state.scanning || state.trashing;
        subTrashBtn.append(icon(Trash2, 11), document.createTextNode(copy.items.trash));
        subTrashBtn.addEventListener("click", () => {
          state.pendingConfirm = {
            paths: [sub.path],
            getLabel: () =>
              getCopy(state.language).items.confirmTrashItem(
                sub.name,
                formatBytes(sub.sizeBytes, state.language),
              ),
          };
          updateConfirmBar();
        });
        subBottom.append(subTrashBtn);

        subRow.append(subTop, subBottom);
        subPanel.append(subRow);
      }
      row.append(subPanel);
    }

    list.append(row);
  }

  panelItens.append(list);
}

function renderAjustes(): void {
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
    state.settingsDraft = { ...cfg };
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

  const saveBtn = el(
    "button",
    "btn btn-primary",
    state.savingSettings ? copy.settings.saving : copy.settings.saveSettings,
  );
  saveBtn.type = "submit";
  saveBtn.disabled = state.savingSettings;

  actions.append(detectBtn, saveBtn);
  form.append(langSection, minSection, aiSection, actions);
  panelAjustes.append(form);
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
    paths: safeTargets.paths,
    getLabel: () =>
      getCopy(state.language).items.confirmTrashSafe(
        safeTargets.paths.length,
        formatBytes(safeTargets.totalBytes, state.language),
      ),
  };
  updateConfirmBar();
}

async function refreshDiskHealth(): Promise<void> {
  try {
    state.diskHealth = await invoke<DiskHealth>("get_disk_health");
  } catch (err) {
    showToast(getCopy(state.language).toasts.diskError(String(err)));
  }
}

async function runScan(): Promise<void> {
  if (state.scanning || state.trashing) {
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
        renderResumo();
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
    const [diskResult, configResult] = await Promise.allSettled([
      invoke<DiskHealth>("get_disk_health"),
      invoke<AiConfig>("get_ai_config"),
    ]);
    if (diskResult.status === "fulfilled") {
      state.diskHealth = diskResult.value;
    }
    if (configResult.status === "fulfilled") {
      state.aiConfig = configResult.value;
      state.language = configResult.value.language;
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

async function executeTrash(paths: string[]): Promise<void> {
  if (state.trashing || state.scanning || paths.length === 0) {
    state.pendingConfirm = null;
    updateConfirmBar();
    return;
  }

  state.trashing = true;
  renderActiveTab();

  try {
    const result = await invoke<TrashResult>("trash_items", { paths });
    if (state.summary && result.trashedPaths.length > 0) {
      const removed = new Set(result.trashedPaths);
      const updatedItems: ScanItem[] = [];

      for (const item of state.summary.items) {
        if (removed.has(item.path)) {
          state.expandedItemIds.delete(item.id);
          if (state.highlightedItemId === item.id) {
            state.highlightedItemId = null;
          }
          continue;
        }
        const remainingSubs = item.subitems.filter((sub) => !removed.has(sub.path));
        const removedSubBytes = item.subitems
          .filter((sub) => removed.has(sub.path))
          .reduce((sum, sub) => sum + sub.sizeBytes, 0);
        const removedSubFiles = item.subitems
          .filter((sub) => removed.has(sub.path))
          .reduce((sum, sub) => sum + sub.fileCount, 0);

        const nextSize = Math.max(0, item.sizeBytes - removedSubBytes);
        if (nextSize === 0 && remainingSubs.length === 0 && removedSubBytes > 0) {
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

      updatedItems.sort(
        (a, b) => b.sizeBytes - a.sizeBytes || a.name.localeCompare(b.name),
      );

      state.summary = {
        ...state.summary,
        items: updatedItems,
        itemCount: updatedItems.length,
        totalBytes: updatedItems.reduce((sum, item) => sum + item.sizeBytes, 0),
        safeBytes: updatedItems.reduce((sum, item) => sum + item.cleanableBytes, 0),
      };
    }

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
  if (state.checkingUpdate) {
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

async function saveLanguagePreference(language: AppLanguage): Promise<void> {
  if (!state.aiConfig) {
    renderActiveTab();
    return;
  }
  try {
    const saved = await invoke<AiConfig>("set_ai_config", {
      config: { ...state.aiConfig, language },
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

async function saveSettings(draft: AiConfig): Promise<void> {
  state.savingSettings = true;
  state.settingsError = null;
  state.settingsDraft = { ...draft };
  renderAjustes();

  try {
    const saved = await invoke<AiConfig>("set_ai_config", { config: draft });
    state.aiConfig = saved;
    state.settingsDraft = { ...saved };
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
  state.detectingModels = true;
  state.settingsError = null;
  state.settingsDraft = { ...draft };
  renderAjustes();

  try {
    const saved = await invoke<AiConfig>("set_ai_config", { config: draft });
    state.aiConfig = saved;
    state.settingsDraft = { ...saved };
    const models = await invoke<string[]>("list_ollama_models");
    state.installedOllamaModels = models;
    if (models.length > 0 && (!saved.ollamaModel || !models.includes(saved.ollamaModel))) {
      const updated = await invoke<AiConfig>("set_ai_config", {
        config: { ...saved, ollamaModel: models[0] },
      });
      state.aiConfig = updated;
      state.settingsDraft = { ...updated };
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
    if (state.updateResult?.updateAvailable && state.updateResult.releaseUrl) {
      void invoke("open_release_url", { url: state.updateResult.releaseUrl }).catch((err) => {
        showToast(getCopy(state.language).toasts.updateError(String(err)));
      });
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
    if (state.pendingConfirm) {
      void executeTrash(state.pendingConfirm.paths);
    }
  });

  window.addEventListener("keydown", (event) => {
    if (event.key === "Escape" && state.pendingConfirm && !state.trashing) {
      state.pendingConfirm = null;
      updateConfirmBar();
    }
  });
}

async function init(): Promise<void> {
  wireEvents();
  renderActiveTab();

  const [versionResult, diskResult, configResult] = await Promise.allSettled([
    invoke<string>("get_app_version"),
    invoke<DiskHealth>("get_disk_health"),
    invoke<AiConfig>("get_ai_config"),
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

  renderActiveTab();
}

void init();
