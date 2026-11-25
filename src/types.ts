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
  releaseUrl: string | null;
  message: string;
}

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
