/** Small presentation helpers shared across panels. */

const UNITS: [limit: number, seconds: number, name: Intl.RelativeTimeFormatUnit][] = [
  [60, 1, "second"],
  [3600, 60, "minute"],
  [86_400, 3600, "hour"],
  [2_592_000, 86_400, "day"],
  [31_536_000, 2_592_000, "month"],
  [Number.POSITIVE_INFINITY, 31_536_000, "year"],
];

const relative = new Intl.RelativeTimeFormat(undefined, { numeric: "auto" });

/** "3 hours ago" from a Unix timestamp in seconds. */
export function timeAgo(timestamp: number): string {
  const elapsed = Math.max(0, Date.now() / 1000 - timestamp);
  const unit = UNITS.find(([limit]) => elapsed < limit) ?? UNITS[UNITS.length - 1];
  return relative.format(-Math.floor(elapsed / unit[1]), unit[2]);
}

const absolute = new Intl.DateTimeFormat(undefined, {
  dateStyle: "medium",
  timeStyle: "short",
});

export const fullDate = (timestamp: number): string =>
  absolute.format(new Date(timestamp * 1000));

/** Last path segment, used to show a filename ahead of its directory. */
export function splitPath(path: string): { dir: string; file: string } {
  const cut = path.lastIndexOf("/");
  return cut === -1
    ? { dir: "", file: path }
    : { dir: path.slice(0, cut + 1), file: path.slice(cut + 1) };
}

export const initials = (name: string): string =>
  name
    .split(/\s+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((part) => part[0]?.toUpperCase() ?? "")
    .join("") || "?";
