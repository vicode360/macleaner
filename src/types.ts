export type RiskLevel = "baixo" | "medio" | "alto";

export type AiProvider = "ollama" | "externalApi" | "disabled";

export type AppLanguage = "pt-br" | "en" | "es";

export interface DiskHealth {
  volumeName: string;
  totalBytes: number;
  availableBytes: number;
  usedBytes: number;
  usedPercent: number;
}

export interface ScanSubItem {
  name: string;
  path: string;
  sizeBytes: number;
  fileCount: number;
  daysSinceModified: number;
  isDir: boolean;
  risk: RiskLevel;
  reason: string;
  safeToClean: boolean;
}

export interface ExtensionStat {
  ext: string;
  sizeBytes: number;
  fileCount: number;
}

export interface ScanItem {
  id: string;
  name: string;
  path: string;
  category: string;
  sizeBytes: number;
  fileCount: number;
  daysSinceModified: number;
  risk: RiskLevel;
  reason: string;
  ruleId: string | null;
  aiClassified: boolean;
  selected: boolean;
  subitems: ScanSubItem[];
  cleanableBytes: number;
  topExtensions: ExtensionStat[];
}

export interface ScanSummary {
  items: ScanItem[];
  totalBytes: number;
  safeBytes: number;
  itemCount: number;
  skippedPermissionCount: number;
  durationMs: number;
  aiAvailable: boolean;
}

export interface AiConfig {
  provider: AiProvider;
  language: AppLanguage;
  minMb: number;
  ollamaEndpoint: string;
  ollamaModel: string;
  externalEndpoint: string;
  externalModel: string;
  externalApiKey: string;
  timeoutMs: number;
  ignoredPaths: string[];
  lowDiskAlertEnabled: boolean;
  devProjectRoots?: string[];
  devRootsInitialized?: boolean;
}

export interface AiStatus {
  available: boolean;
  provider: AiProvider;
  endpoint: string;
  model: string;
  reason: string | null;
}
export interface UpdateCheckResult {
  currentVersion: string;
  latestVersion: string | null;
  updateAvailable: boolean;
  message: string;
}

export type UpdateInstallEvent =
  | { event: "checking" }
  | { event: "downloading"; data: { downloadedBytes: number; totalBytes: number | null } }
  | { event: "installing" }
  | { event: "restarting" };

export interface TrashFailure {
  path: string;
  reason: string;
}

export interface TrashResult {
  trashedPaths: string[];
  failedPaths: TrashFailure[];
  freedBytes: number;
}

export interface ApfsSnapshot {
  id: string;
  dateToken: string;
  timestampIso: string;
  volume: string;
}

export interface SnapshotListResult {
  snapshots: ApfsSnapshot[];
  supported: boolean;
  note: string | null;
}

export interface SnapshotDeleteFailure {
  dateToken: string;
  reason: string;
}

export interface SnapshotDeleteResult {
  deletedTokens: string[];
  failed: SnapshotDeleteFailure[];
  cancelled: boolean;
}

export type ScanEvent =
  | { event: "started"; data: { totalTargets: number } }
  | { event: "progress"; data: { currentPath: string; scannedBytes: number } }
  | { event: "finished"; data: { summary: ScanSummary } };

export interface DevArtifact {
  path: string;
  name: string;
  sizeBytes: number;
}

export interface DevProject {
  id: string;
  name: string;
  path: string;
  kind: string;
  totalArtifactBytes: number;
  inactiveDays: number;
  artifacts: DevArtifact[];
}

export interface DevProjectsReport {
  roots: string[];
  projects: DevProject[];
  totalReclaimableBytes: number;
  scanDurationMs: number;
}

export interface DevTrashResult {
  trashedPaths: string[];
  freedBytes: number;
  failed: TrashFailure[];
}

export interface XcodeAreaItem {
  id: string;
  category: string;
  name: string;
  path: string;
  sizeBytes: number;
  inactiveDays: number;
}

export interface UnavailableSimulator {
  udid: string;
  name: string;
  runtime: string;
  state: string;
  sizeBytes: number;
}

export interface SimulatorRuntime {
  identifier: string;
  name: string;
  build: string;
  sizeBytes: number;
  state: string;
}

export interface XcodeReport {
  installed: boolean;
  areas: XcodeAreaItem[];
  unavailableSimulators: UnavailableSimulator[];
  unavailableTotalBytes: number;
  runtimes: SimulatorRuntime[];
  totalReclaimableBytes: number;
}

export interface XcodeActionResult {
  freedBytes: number;
  deletedCount: number;
  failed: TrashFailure[];
}

export type DockerPruneScope = "build-cache" | "unused-images" | "stopped-containers";

export interface DockerResourceRow {
  kind: string;
  totalCount: number;
  activeCount: number;
  sizeBytes: number;
  reclaimableBytes: number;
  reclaimablePercent: number;
  pruneScope: DockerPruneScope | null;
  readOnly: boolean;
}

export interface DockerReport {
  installed: boolean;
  daemonRunning: boolean;
  engineName: string;
  orbstackDetected: boolean;
  diskFootprintBytes: number;
  totalReclaimableBytes: number;
  resources: DockerResourceRow[];
  error: string | null;
}

export interface DockerPruneResult {
  scope: DockerPruneScope;
  reclaimedBytes: number;
  rawSummary: string;
}

export interface HomebrewReport {
  installed: boolean;
  outdatedCount: number;
  reclaimableBytes: number;
  autoremovePackages: string[];
  previewLines: string[];
}

export interface ToolchainCacheItem {
  id: string;
  name: string;
  ecosystem: string;
  path: string;
  sizeBytes: number;
}

export interface ToolchainsReport {
  homebrew: HomebrewReport;
  caches: ToolchainCacheItem[];
  totalReclaimableBytes: number;
}

export type BrewCleanAction = "cleanup" | "autoremove";

export interface BrewCleanResult {
  action: BrewCleanAction;
  freedBytes: number;
  summary: string;
}
