import { resolveNetwork } from "../core/network.ts";
import { loadFundContractId, loadOperatorKeypair } from "../core/wallet.ts";
import { addYield, queryAum, queryMaxAumIncreaseBps } from "../providers/soroban/fund.ts";

// TODO(Etherfuse): replace CLI arg with Etherfuse API call that returns
// accumulated yield since the last registered ledger.

const USAGE = "Uso: bun run job:yield-sync <amount_usdc_6dec>\nExemplo: bun run job:yield-sync 1500000  (= 1.50 USDC)";
const BPS_DENOMINATOR = 10_000n;

async function run() {
  const rawAmount = process.argv[2];
  if (!rawAmount) throw new Error(USAGE);

  let yieldAmount: bigint;
  try {
    yieldAmount = BigInt(rawAmount);
  } catch {
    throw new Error(`Valor inválido: "${rawAmount}". Use inteiro em 6 decimais.\n${USAGE}`);
  }
  if (yieldAmount <= 0n) {
    console.log("[yield-sync] valor zero ou negativo — nada a fazer");
    return;
  }

  const net = resolveNetwork();
  const operator = loadOperatorKeypair();
  const contractId = loadFundContractId();

  console.log(`[yield-sync] iniciado — rede: ${net.name}, yield: ${yieldAmount}`);

  const [aum, maxBps] = await Promise.all([
    queryAum(net, operator, contractId),
    queryMaxAumIncreaseBps(net, operator, contractId),
  ]);

  const maxPerCall = aum * BigInt(maxBps) / BPS_DENOMINATOR;

  if (maxPerCall === 0n) {
    console.log("[yield-sync] AUM ou cap é zero — nada a fazer");
    return;
  }

  console.log(`[yield-sync] AUM: ${aum}, cap por chamada: ${maxPerCall} (${maxBps} bps)`);

  let remaining = yieldAmount;
  let calls = 0;

  while (remaining > 0n) {
    const batch = remaining < maxPerCall ? remaining : maxPerCall;
    await addYield(net, operator, contractId, batch);
    remaining -= batch;
    calls++;
    console.log(`[yield-sync] add_yield confirmado — lote ${calls}: ${batch} (restante: ${remaining})`);
  }

  console.log(`[yield-sync] concluído — ${yieldAmount} USDC registrado em ${calls} chamada(s)`);
}

run();
