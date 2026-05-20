import { resolveNetwork } from "../core/network.ts";
import { loadFundContractId, loadOperatorKeypair } from "../core/wallet.ts";
import { extendTtl } from "../providers/soroban/fund.ts";

// TTL lease is ~30 days. Renew every 25 days to keep a 5-day safety margin.
const INTERVAL_MS = 25 * 24 * 60 * 60 * 1000;
// On failure, retry after 1 hour instead of waiting the full interval.
const RETRY_MS = 60 * 60 * 1000;
// JS runtimes use a 32-bit timer internally (max ~24.8 days). Sleep in 1-hour
// chunks so we never pass a value that would overflow and fire immediately.
const TICK_MS = 60 * 60 * 1000;

let stopping = false;
process.on("SIGTERM", () => {
  console.log("[heartbeat] SIGTERM recebido — aguardando ciclo atual terminar");
  stopping = true;
});

async function run() {
  const net = resolveNetwork();
  const operator = loadOperatorKeypair();
  const contractId = loadFundContractId();

  console.log(`[heartbeat] daemon iniciado — rede: ${net.name}, renovação a cada 25 dias`);

  let nextAt = Date.now();

  while (!stopping) {
    const now = Date.now();

    if (now >= nextAt) {
      try {
        console.log(`[heartbeat] extend_ttl — contrato: ${contractId}`);
        await extendTtl(net, operator, contractId);
        console.log("[heartbeat] TTL renovado com sucesso");
        nextAt = Date.now() + INTERVAL_MS;
      } catch (err) {
        console.error("╔══════════════════════════════════════════════════════╗");
        console.error("║  ALERTA CRÍTICO: extend_ttl falhou                   ║");
        console.error("║  Se não corrigido, o estado do contrato irá expirar. ║");
        console.error("╚══════════════════════════════════════════════════════╝");
        console.error("[heartbeat] erro:", err);
        nextAt = Date.now() + RETRY_MS;
        console.error(`[heartbeat] próxima tentativa em 1 hora`);
      }
    }

    const wait = Math.min(nextAt - Date.now(), TICK_MS);
    if (wait > 0 && !stopping) await Bun.sleep(wait);
  }

  console.log("[heartbeat] encerrado");
}

run();
