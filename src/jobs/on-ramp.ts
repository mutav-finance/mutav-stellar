import { Horizon } from "@stellar/stellar-sdk";
import { resolveNetwork } from "../core/network.ts";
import { loadFundContractId, loadOperatorKeypair } from "../core/wallet.ts";
import { receivePayment } from "../providers/soroban/fund.ts";

const POLL_INTERVAL_MS = 30_000;
const USDC_ASSET_CODE = "USDC";
const CURSOR_FILE = ".on-ramp-cursor";

// IDs de transações já processadas na sessão atual — segunda linha de defesa
// contra duplo processamento (a primeira é o cursor persistido em disco).
const processed = new Set<string>();

interface UsdcCredit {
  txHash: string;
  imobiliaria: string;
  amountUsdc: bigint;
}

// Stellar representa valores com 7 casas decimais (ex: "100.0000000").
// O contrato espera USDC com 6 casas decimais (ex: 100_000_000n).
function stellarAmountToContractUnits(amount: string): bigint {
  const [whole, frac = ""] = amount.split(".");
  const fracPadded = frac.padEnd(6, "0").slice(0, 6);
  return BigInt(whole + fracPadded);
}

function loadCursor(): string {
  try {
    return Bun.file(CURSOR_FILE).text() as unknown as string;
  } catch {
    return "now";
  }
}

async function saveCursor(cursor: string): Promise<void> {
  await Bun.write(CURSOR_FILE, cursor);
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
    if (!record.asset_code || !record.asset_issuer) continue;
    if (record.asset_code !== USDC_ASSET_CODE || record.asset_issuer !== usdcIssuer) continue;

    const txHash = record.transaction_hash;
    if (processed.has(txHash)) continue;

    credits.push({
      txHash,
      imobiliaria: record.from,
      amountUsdc: stellarAmountToContractUnits(record.amount),
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
  let cursor = loadCursor();

  console.log(`[on-ramp] iniciado — rede: ${net.name}, operador: ${operator.publicKey()}`);
  console.log(`[on-ramp] cursor: ${cursor}`);

  while (true) {
    try {
      const { credits, nextCursor } = await fetchNewCredits(
        horizon,
        operator.publicKey(),
        usdcIssuer,
        cursor
      );

      cursor = nextCursor;
      await saveCursor(cursor);

      for (const credit of credits) {
        console.log(
          `[on-ramp] crédito detectado — imobiliária: ${credit.imobiliaria}, valor: ${credit.amountUsdc} — tx: ${credit.txHash}`
        );

        try {
          await receivePayment(
            net,
            operator,
            contractId,
            credit.imobiliaria,
            credit.amountUsdc,
            credit.txHash
          );
          processed.add(credit.txHash);
          console.log(`[on-ramp] receive_payment confirmado — tx: ${credit.txHash}`);
        } catch (err) {
          // Não adiciona ao processed — será retentado no próximo ciclo via cursor
          console.error(`[on-ramp] falha ao processar tx ${credit.txHash}:`, err);
        }
      }
    } catch (err) {
      console.error("[on-ramp] erro no ciclo de polling:", err);
    }

    await Bun.sleep(POLL_INTERVAL_MS);
  }
}

run();
