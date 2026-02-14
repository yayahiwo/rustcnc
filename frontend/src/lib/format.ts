/** Format a coordinate value with fixed decimal places */
export function formatCoord(value: number, decimals = 3): string {
  return value.toFixed(decimals);
}

/** Format feed rate (mm/min) */
export function formatFeed(value: number): string {
  return Math.round(value).toString();
}

/** Format spindle RPM */
export function formatRPM(value: number): string {
  return Math.round(value).toString();
}

/** Format override percentage */
export function formatOverride(value: number): string {
  return `${value}%`;
}

/** Format file size in human-readable form */
export function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/** Format duration in seconds to human readable */
export function formatDuration(totalSecs: number): string {
  const h = Math.floor(totalSecs / 3600);
  const m = Math.floor((totalSecs % 3600) / 60);
  const s = Math.floor(totalSecs % 60);
  if (h > 0) return `${h}h ${pad(m)}m ${pad(s)}s`;
  if (m > 0) return `${m}m ${pad(s)}s`;
  return `${s}s`;
}

function pad(n: number): string {
  return n.toString().padStart(2, '0');
}

/** Format percentage */
export function formatPercent(value: number): string {
  return `${value.toFixed(1)}%`;
}
