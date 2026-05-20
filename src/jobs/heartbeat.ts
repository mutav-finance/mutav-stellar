import { resolveNetwork } from "../core/network.ts";
import { loadFundContractId, loadOperatorKeypair } from "../core/wallet.ts";
import { extendTtl } from "../providers/soroban/fund.ts";

// TTL lease is ~30 days. Renew every 25 days to keep a 5-day safety margin.
const INTERVAL_MS = 25 * 24 * 60 * 60 * 1000;
// On failure, retry after 1 hour instead of waiting the full interval.
const RETRY_MS = 60 * 60 * 1000;

async function extendOnce(): Promise<void> {
  const net = resolveNetwork();
  const operator = loadOperatorKeypair();
  const contractId = loadFundContractId();

  console.log(`[heartbeat] extend_ttl — rede: ${net.name}, contrato: ${contractId}`);
  await extendTtl(net, operator, contractId);
  console.log("[heartbeat] TTL renovado com sucesso");
}

async function run() {
  console.log("[heartbeat] daemon iniciado — renovação a cada 25 dias");

  while (true) {
    try {
      await extendOnce();
      await Bun.sleep(INTERVAL_MS);
    } catch (err) {
      console.error("╔══════════════════════════════════════════════════════╗");
      console.error("║  ALERTA CRÍTICO: extend_ttl falhou                   ║");
      console.error("║  Se não corrigido, o estado do contrato irá expirar. ║");
      console.error("╚══════════════════════════════════════════════════════╝");
      console.error("[heartbeat] erro:", err);
      console.error(`[heartbeat] próxima tentativa em 1 hora`);
      await Bun.sleep(RETRY_MS);
    }
  }
}

run();
