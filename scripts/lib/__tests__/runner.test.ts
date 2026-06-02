import { afterEach, describe, expect, test } from "bun:test";

import { runScript } from "../runner.ts";

const origExit = process.exitCode;
afterEach(() => {
  process.exitCode = origExit;
});

describe("runScript", () => {
  test("invokes fn and resolves on success", async () => {
    let called = false;
    await runScript("noop", async () => {
      called = true;
    });
    expect(called).toBe(true);
    expect(process.exitCode).not.toBe(1);
  });

  test("sets exitCode=1 and does not re-throw on error", async () => {
    await runScript("bang", async () => {
      throw new Error("kaboom");
    });
    expect(process.exitCode).toBe(1);
  });

  test("handles non-Error throws", async () => {
    await runScript("string-throw", async () => {
      // eslint-disable-next-line @typescript-eslint/no-throw-literal
      throw "just-a-string";
    });
    expect(process.exitCode).toBe(1);
  });
});
