import { Horizon } from "@stellar/stellar-sdk";
import { resolveNetwork } from "../core/network.ts";
import { loadFundContractId, loadOperatorKeypair } from "../core/wallet.ts";
import { parseStellarUsdc } from "../core/units.ts";
import { processRedemptions, fulfillRedemption } from "../providers/soroban/fund.ts";
import { transferToken } from "../providers/soroban/token.ts";

const BALANCE_POLL_INTERVAL_MS = 5 * 60 * 1000;
const BALANCE_POLL_TIMEOUT_MS  = 24 * 60 * 60 * 1000;

async function waitForUsdcBalance(
  horizon: Horizon.Server,
  operatorPublicKey: string,
  usdcIssuer: string,
  needed: bigint,
): Promise<void> {
  const deadline = Date.now() + BALANCE_POLL_TIMEOUT_MS;

  while (Date.now() < deadline) {
    const account = await horizon.loadAccount(operatorPublicKey);
    const usdcEntry = account.balances.find(
      (b) =>
        b.asset_type !== "native" &&
        (b as Horizon.HorizonApi.BalanceLineAsset).asset_code === "USDC" &&
        (b as Horizon.HorizonApi.BalanceLineAsset).asset_issuer === usdcIssuer,
    ) as Horizon.HorizonApi.BalanceLineAsset | undefined;

    const balance = usdcEntry ? parseStellarUsdc(usdcEntry.balance) : 0n;

    if (balance >= needed) {
      console.log(`[off-ramp] saldo USDC suficiente: ${balance} >= ${needed}`);
      return;
    }

    console.log(`[off-ramp] aguardando USDC... saldo atual: ${balance}, necessário: ${needed}`);
    await Bun.sleep(BALANCE_POLL_INTERVAL_MS);
  }

  throw new Error(`[off-ramp] timeout aguardando USDC na carteira do operador após 24h`);
}

async function run() {
  const net = resolveNetwork();
  const operator = loadOperatorKeypair();
  const contractId = loadFundContractId();

  const usdcIssuer = process.env.USDC_ISSUER;
  if (!usdcIssuer) throw new Error("USDC_ISSUER não definido no .env.local");

  const usdcContractId = process.env.USDC_CONTRACT_ID;
  if (!usdcContractId) throw new Error("USDC_CONTRACT_ID não definido no .env.local");

  console.log(`[off-ramp] iniciado — rede: ${net.name}, operador: ${operator.publicKey()}`);

  console.log("[off-ramp] chamando process_redemptions...");
  const { totalUsdc, investors } = await processRedemptions(net, operator, contractId);

  if (totalUsdc === 0n || investors.length === 0) {
    console.log("[off-ramp] nenhum resgate pendente — nada a fazer");
    return;
  }

  console.log(`[off-ramp] ${investors.length} investidor(es) prontos — total USDC: ${totalUsdc}`);

  // TODO(Etherfuse): call liquidation API to convert ${totalUsdc} USDC from TESOURO before polling
  console.log(`[off-ramp] aguardando USDC na carteira do operador (${operator.publicKey()})...`);

  const horizon = new Horizon.Server(net.horizonUrl);
  await waitForUsdcBalance(horizon, operator.publicKey(), usdcIssuer, totalUsdc);

  console.log(`[off-ramp] depositando ${totalUsdc} USDC no contrato...`);
  await transferToken(net, operator, usdcContractId, contractId, totalUsdc);
  console.log("[off-ramp] USDC depositado no contrato");

  let fulfilled = 0;
  for (const investor of investors) {
    try {
      await fulfillRedemption(net, operator, contractId, investor);
      fulfilled++;
      console.log(`[off-ramp] fulfill_redemption confirmado — investidor: ${investor}`);
    } catch (err) {
      console.error(`[off-ramp] falha ao fulfillRedemption para ${investor}:`, err);
    }
  }

  console.log(`[off-ramp] ciclo concluído — ${fulfilled}/${investors.length} investidor(es) atendidos`);
}

run();
