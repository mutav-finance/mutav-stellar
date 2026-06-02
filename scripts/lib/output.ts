// Output formatters for every CLI: machine-readable JSON, human-readable
// key/value blocks, and tabular listings. Handles bigint + bytes natively.

export function printJSON(data: unknown): void {
  console.log(JSON.stringify(data, jsonReplacer, 2));
}

function jsonReplacer(_key: string, value: unknown): unknown {
  if (typeof value === "bigint") return value.toString();
  if (value instanceof Uint8Array) {
    return Buffer.from(value).toString("hex");
  }
  return value;
}

export function printKV(pairs: ReadonlyArray<readonly [string, unknown]>): void {
  if (pairs.length === 0) return;
  const width = Math.max(...pairs.map(([k]) => k.length));
  for (const [k, v] of pairs) {
    console.log(`  ${k.padEnd(width)}  ${formatVal(v)}`);
  }
}

export function printTable(
  rows: ReadonlyArray<Record<string, unknown>>,
  columns?: ReadonlyArray<string>,
): void {
  if (rows.length === 0) {
    console.log("(empty)");
    return;
  }
  const cols = columns ?? Object.keys(rows[0]!);
  const widths = cols.map((c) =>
    Math.max(c.length, ...rows.map((r) => formatVal(r[c]).length)),
  );
  const sep = "  ";
  console.log(cols.map((c, i) => c.padEnd(widths[i]!)).join(sep));
  console.log(cols.map((_, i) => "─".repeat(widths[i]!)).join(sep));
  for (const r of rows) {
    console.log(cols.map((c, i) => formatVal(r[c]).padEnd(widths[i]!)).join(sep));
  }
}

function formatVal(v: unknown): string {
  if (v === null || v === undefined) return "";
  if (typeof v === "bigint") return v.toString();
  if (typeof v === "string") return v;
  if (typeof v === "number" || typeof v === "boolean") return String(v);
  if (v instanceof Uint8Array) return Buffer.from(v).toString("hex");
  return JSON.stringify(v);
}
