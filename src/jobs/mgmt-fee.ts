import { Asset } from "@stellar/stellar-sdk";
import { resolveNetwork } from "../core/network.ts";
import { loadFundContractId, loadOperatorKeypair } from "../core/wallet.ts";
import { chargeMgmtFee, queryAum, queryMgmtFeeBps } from "../providers/soroban/fund.ts";
import { sendClassicPayment } from "../providers/horizon/classic.ts";

const BPS_DENOMINATOR = 10_000n;

function loadClassicConfig() {
  const wallet = process.env.ETHERFUSE_CLASSIC_WALLET;
  const pixKey = process.env.MGMT_FEE_PIX_KEY;
  const assetCode = process.env.TESOURO_ASSET_CODE;
  const assetIssuer = process.env.TESOURO_ISSUER;

  if (!wallet) throw new Error("ETHERFUSE_CLASSIC_WALLET não definido no .env.local");
  if (!pixKey) throw new Error("MGMT_FEE_PIX_KEY não definido no .env.local");
  if (!assetCode) throw new Error("TESOURO_ASSET_CODE não definido no .env.local");
  if (!assetIssuer) throw new Error("TESOURO_ISSUER não definido no .env.local");

  return { wallet, pixKey, assetCode, assetIssuer };
}

// Horizon's payment operation requires a 7-decimal string (e.g. "1.5000000").
// The contract stores amounts as USDC with 6 decimals — pad frac to 6 digits
// then append one zero to reach 7.
function toStellarAmount(amount6dec: bigint): string {
  const whole = amount6dec / 1_000_000n;
  const frac = (amount6dec % 1_000_000n).toString().padStart(6, "0");
  return `${whole}.${frac}0`;
}

async function run() {
  const net = resolveNetwork();
  const operator = loadOperatorKeypair();
  const contractId = loadFundContractId();

  console.log(`[mgmt-fee] iniciado — rede: ${net.name}`);

  const [aum, feeBps] = await Promise.all([
    queryAum(net, operator, contractId),
    queryMgmtFeeBps(net, operator, contractId),
  ]);

  const fee = aum * BigInt(feeBps) / BPS_DENOMINATOR;

  if (fee === 0n) {
    console.log("[mgmt-fee] taxa calculada é zero — nada a fazer");
    return;
  }

  console.log(`[mgmt-fee] AUM: ${aum}, taxa: ${fee} (${feeBps} bps)`);

  try {
    await chargeMgmtFee(net, operator, contractId);
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    if (msg.includes("already charged this period")) {
      console.log("[mgmt-fee] taxa já cobrada neste período — nada a fazer");
      return;
    }
    throw err;
  }

  console.log("[mgmt-fee] charge_mgmt_fee confirmado");

  // TODO(Etherfuse): confirmar endereço da carteira de liquidação e formato do MEMO PIX.
  const { wallet, pixKey, assetCode, assetIssuer } = loadClassicConfig();
  const asset = new Asset(assetCode, assetIssuer);
  const stellarAmount = toStellarAmount(fee);
  console.log(`[mgmt-fee] enviando pagamento Clássico: ${stellarAmount} ${asset.code} → ${wallet} (MEMO: ${pixKey})`);

  await sendClassicPayment(net, operator, wallet, asset, stellarAmount, pixKey);

  console.log(`[mgmt-fee] concluído — ${stellarAmount} ${asset.code} enviado via Stellar Clássico`);
}

run();
