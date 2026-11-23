export const errorText = (e: unknown) => (e instanceof Error ? e.message : String(e));

export function formatAge(days: number | null): string {
  if (days === null) return "";
  if (days < 31) return "this month";
  if (days < 365) return `${Math.round(days / 30)} mo ago`;
  return `${(days / 365.25).toFixed(1)} yr ago`;
}

export function formatSize(bytes: number | null | undefined): string {
  if (!bytes) return "";
  const units = ["KB", "MB", "GB", "TB"];
  let value = bytes / 1024;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit++;
  }
  return `${value >= 10 || unit === 0 ? Math.max(1, Math.round(value)) : value.toFixed(1)} ${units[unit]}`;
}

export function formatDate(unixSeconds: number | null): string {
  return unixSeconds === null ? "" : new Date(unixSeconds * 1000).toISOString().slice(0, 10);
}
