// Uniform entrypoint wrapper for every Phase A CLI.
//
//   await runScript("op-receive-payment", async () => { ... });
//
// Catches and renders errors, sets process.exitCode, and prints elapsed time
// on success. Stack traces are shown when DEBUG is set.

export async function runScript(
  name: string,
  fn: () => Promise<void>,
): Promise<void> {
  const t0 = performance.now();
  try {
    await fn();
    const dt = ((performance.now() - t0) / 1000).toFixed(2);
    console.log(`✓ ${name} ok (${dt}s)`);
  } catch (err: unknown) {
    const message = err instanceof Error ? err.message : String(err);
    console.error(`✗ ${name} failed: ${message}`);
    if (err instanceof Error && err.stack && process.env.DEBUG) {
      console.error(err.stack);
    }
    process.exitCode = 1;
  }
}
