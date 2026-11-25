import type { RiskLevel } from "./types";

export type DustBarVariant = "low" | "med" | "high" | "accent";

export function riskToDustVariant(risk: RiskLevel): DustBarVariant {
  if (risk === "baixo") {
    return "low";
  }
  if (risk === "medio") {
    return "med";
  }
  return "high";
}

export function createDustBar(
  bytes: number,
  maxBytes: number,
  variant: DustBarVariant,
  showPercent = false,
  parentBytes?: number,
): HTMLElement {
  const row = document.createElement("div");
  row.className = "dust-bar-row";

  const wrap = document.createElement("div");
  wrap.className = "dust-bar-wrap";
  wrap.setAttribute("aria-hidden", "true");
  const safeBytes = Number.isFinite(bytes) && bytes > 0 ? bytes : 0;
  const denom = Number.isFinite(maxBytes) && maxBytes > 0 ? maxBytes : 1;
  const rawPct = safeBytes > 0 ? (safeBytes / denom) * 100 : 0;
  const clampedPct = safeBytes > 0 ? Math.max(2, Math.min(100, rawPct)) : 0;

  const fill = document.createElement("div");
  fill.className = `dust-bar-fill is-${variant}`;
  fill.style.setProperty("--bar-pct", `${clampedPct.toFixed(1)}%`);

  wrap.append(fill);
  row.append(wrap);

  if (showPercent) {
    const pctBase =
      parentBytes !== undefined && Number.isFinite(parentBytes) && parentBytes > 0
        ? parentBytes
        : denom;
    const share = safeBytes > 0 ? (safeBytes / pctBase) * 100 : 0;
    const label =
      safeBytes <= 0 ? "0%" : share < 1 ? "<1%" : `${Math.round(Math.min(100, share))}%`;
    const pctEl = document.createElement("span");
    pctEl.className = "dust-bar-pct tabular";
    pctEl.textContent = label;
    row.append(pctEl);
  }

  return row;
}
