import { afterEach, describe, expect, test } from "bun:test";

import { printJSON, printKV, printTable } from "../output.ts";

let captured: string[] = [];
const orig = console.log;

function capture(fn: () => void): string {
  captured = [];
  console.log = (...args: unknown[]) => {
    captured.push(args.map(String).join(" "));
  };
  try {
    fn();
  } finally {
    console.log = orig;
  }
  return captured.join("\n");
}

afterEach(() => {
  console.log = orig;
});

describe("printJSON", () => {
  test("serializes bigint as string", () => {
    const out = capture(() => printJSON({ supply: 1_234n }));
    expect(out).toContain('"supply": "1234"');
  });

  test("serializes Uint8Array as hex", () => {
    const out = capture(() => printJSON({ hash: new Uint8Array([0xab, 0xcd]) }));
    expect(out).toContain('"hash": "abcd"');
  });
});

describe("printKV", () => {
  test("aligns the key column", () => {
    const out = capture(() =>
      printKV([
        ["k1", "v1"],
        ["longerKey", "v2"],
      ]),
    );
    const lines = out.split("\n");
    expect(lines[0]).toMatch(/k1\s+v1/);
    expect(lines[1]).toMatch(/longerKey\s+v2/);
    // both value columns line up
    expect(lines[0]!.indexOf("v1")).toBe(lines[1]!.indexOf("v2"));
  });

  test("renders bigint values", () => {
    const out = capture(() => printKV([["aum", 12_345n]]));
    expect(out).toContain("12345");
  });

  test("noop on empty input", () => {
    const out = capture(() => printKV([]));
    expect(out).toBe("");
  });
});

describe("printTable", () => {
  test("prints `(empty)` for an empty array", () => {
    const out = capture(() => printTable([]));
    expect(out).toBe("(empty)");
  });

  test("renders rows with header + separator", () => {
    const out = capture(() =>
      printTable([
        { name: "alice", amt: 100n },
        { name: "bob", amt: 200n },
      ]),
    );
    const lines = out.split("\n");
    expect(lines[0]).toMatch(/name\s+amt/);
    expect(lines[1]).toMatch(/─+\s+─+/);
    expect(lines[2]).toMatch(/alice\s+100/);
    expect(lines[3]).toMatch(/bob\s+200/);
  });

  test("respects explicit column order", () => {
    const out = capture(() =>
      printTable([{ name: "alice", amt: 100n }], ["amt", "name"]),
    );
    const lines = out.split("\n");
    expect(lines[0]).toMatch(/amt\s+name/);
    expect(lines[2]).toMatch(/100\s+alice/);
  });
});
