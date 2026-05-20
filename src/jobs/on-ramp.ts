import { Horizon } from "@stellar/stellar-sdk";
import { resolveNetwork } from "../core/network.ts";
import { loadFundContractId, loadOperatorKeypair } from "../core/wallet.ts";
import { receivePayment } from "../providers/soroban/fund.ts";

const POLL_INTERVAL_MS = 30_000;
const USDC_ASSET_CODE = "USDC";

// IDs de transações já processadas — evita chamar receive_payment duas vezes
const processed = new Set<string>();

interface UsdcCredit {
  txHash: string;
  imobiliaria: string;
  amountUsdc: bigint;
}

function usdcStroopsToUnits(stroops: string): bigint {
  // Stellar usa 7 casas decimais. O contrato espera valor em unidades inteiras de USDC (6 casas).
  // Ex: "100.0000000" → 100_000_000n (6 decimais)
  const [whole, frac = ""] = stroops.split(".");
  const fracPadded = frac.padEnd(6, "0").slice(0, 6);
  return BigInt(whole + fracPadded);
}

async function fetchNewCredits(
  horizon: Horizon.Server,
  operatorPublicKey: string,
  usdcIssuer: string,
  cursor: string
): Promise<{ credits: UsdcCredit[]; nextCursor: string }> {
  const payments = await horizon
    .payments()
    .forAccount(operatorPublicKey)
    .cursor(cursor)
    .order("asc")
    .limit(50)
    .call();

  const credits: UsdcCredit[] = [];
  let nextCursor = cursor;

  for (const record of payments.records) {
    nextCursor = record.paging_token;

    if (record.type !== "payment") continue;
    if (record.to !== operatorPublicKey) continue;
    if (
      record.asset_type === "native" ||
      record.asset_code !== USDC_ASSET_CODE ||
      record.asset_issuer !== usdcIssuer
    )
      continue;

    const txHash = record.transaction_hash;
    if (processed.has(txHash)) continue;

    credits.push({
      txHash,
      imobiliaria: record.from,
      amountUsdc: usdcStroopsToUnits(record.amount),
    });
  }

  return { credits, nextCursor };
}

async function run() {
  const net = resolveNetwork();
  const operator = loadOperatorKeypair();
  const contractId = loadFundContractId();

  const usdcIssuer = process.env.USDC_ISSUER;
  if (!usdcIssuer) throw new Error("USDC_ISSUER não definido no .env.local");

  const horizon = new Horizon.Server(net.horizonUrl);
  let cursor = "now";

  console.log(`[on-ramp] iniciado — rede: ${net.name}, operador: ${operator.publicKey()}`);

  while (true) {
    try {
      const { credits, nextCursor } = await fetchNewCredits(
        horizon,
        operator.publicKey(),
        usdcIssuer,
        cursor
      );

      cursor = nextCursor;

      for (const credit of credits) {
        console.log(
          `[on-ramp] crédito detectado — imobiliária: ${credit.imobiliaria}, valor: ${credit.amountUsdc} — tx: ${credit.txHash}`
        );

        await receivePayment(
          net,
          operator,
          contractId,
          credit.imobiliaria,
          credit.amountUsdc
        );

        processed.add(credit.txHash);
        console.log(`[on-ramp] receive_payment confirmado — tx: ${credit.txHash}`);
      }
    } catch (err) {
      console.error("[on-ramp] erro no ciclo de polling:", err);
    }

    await Bun.sleep(POLL_INTERVAL_MS);
  }
}

run();
