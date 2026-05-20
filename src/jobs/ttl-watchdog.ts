import * as fs from "fs";
import * as path from "path";
import { nativeToScVal, scValToNative } from "@stellar/stellar-sdk";
import { resolveNetwork, type NetworkConfig } from "../core/network.ts";
import { loadFundContractId, loadOperatorKeypair } from "../core/wallet.ts";
import { extendBalanceTtl } from "../providers/soroban/fund.ts";
import { sorobanClient } from "../providers/soroban/client.ts";

// Renew any investor whose last extension was 25+ days ago (TTL lease is ~30 days).
const EXTEND_THRESHOLD_MS = 25 * 24 * 60 * 60 * 1000;
// Re-check every hour — well within the 5-day safety margin.
const TICK_MS = 60 * 60 * 1000;
// Soroban RPC providers expose at most ~24h of event history (~17 280 ledgers at 5 s each).
// On first run, scan that window. Older investors must be seeded manually in the state file.
const DEFAULT_LOOKBACK_LEDGERS = 17_280;
const STATE_FILE = path.join(process.cwd(), "data", "ttl-watchdog.json");

interface WatchdogState {
  ledgerCursor: number;
  investors: string[];
  lastExtended: Record<string, number>;
}

function loadState(): WatchdogState {
  try {
    return JSON.parse(fs.readFileSync(STATE_FILE, "utf8")) as WatchdogState;
  } catch {
    return { ledgerCursor: 0, investors: [], lastExtended: {} };
  }
}

function saveState(state: WatchdogState): void {
  fs.mkdirSync(path.dirname(STATE_FILE), { recursive: true });
  fs.writeFileSync(STATE_FILE, JSON.stringify(state, null, 2));
}

// XDR-encode the deposit symbol so the RPC can filter events server-side.
const DEPOSIT_TOPIC_XDR = nativeToScVal("deposit", { type: "symbol" }).toXDR("base64");

async function discoverInvestors(
  net: NetworkConfig,
  contractId: string,
  fromLedger: number,
): Promise<{ investors: string[]; latestLedger: number }> {
  const server = sorobanClient(net);
  const discovered = new Set<string>();
  let cursor: string | undefined;
  let latestLedger = fromLedger;

  while (true) {
    const request = cursor
      ? { cursor, filters: [{ type: "contract" as const, contractIds: [contractId], topics: [[DEPOSIT_TOPIC_XDR, "*"]] }], limit: 200 }
      : { startLedger: fromLedger, filters: [{ type: "contract" as const, contractIds: [contractId], topics: [[DEPOSIT_TOPIC_XDR, "*"]] }], limit: 200 };

    const response = await server.getEvents(request);
    latestLedger = response.latestLedger;

    for (const event of response.events) {
      const investorTopic = event.topic[1];
      if (event.topic.length >= 2 && investorTopic) {
        discovered.add(scValToNative(investorTopic) as string);
      }
    }

    if (response.events.length < 200) break;
    cursor = response.events.at(-1)!.pagingToken;
  }

  return { investors: [...discovered], latestLedger };
}

let stopping = false;
process.on("SIGTERM", () => {
  console.log("[ttl-watchdog] SIGTERM recebido — aguardando ciclo atual terminar");
  stopping = true;
});

async function run() {
  const net = resolveNetwork();
  const operator = loadOperatorKeypair();
  const contractId = loadFundContractId();

  console.log(`[ttl-watchdog] daemon iniciado — rede: ${net.name}, contrato: ${contractId}`);

  while (!stopping) {
    const state = loadState();

    if (state.ledgerCursor === 0) {
      const server = sorobanClient(net);
      const { sequence } = await server.getLatestLedger();
      state.ledgerCursor = Math.max(1, sequence - DEFAULT_LOOKBACK_LEDGERS);
      console.log(`[ttl-watchdog] primeiro boot — escaneando a partir do ledger ${state.ledgerCursor}`);
    }

    // Discover investors who deposited since the last cursor.
    const { investors: found, latestLedger } = await discoverInvestors(net, contractId, state.ledgerCursor);

    const investorSet = new Set(state.investors);
    let newCount = 0;
    for (const addr of found) {
      if (!investorSet.has(addr)) { investorSet.add(addr); newCount++; }
    }
    state.investors = [...investorSet];
    state.ledgerCursor = latestLedger;
    if (newCount > 0) console.log(`[ttl-watchdog] ${newCount} novo(s) investidor(es) descoberto(s) — total: ${state.investors.length}`);
    saveState(state);

    // Extend TTL for any investor approaching the 30-day expiry.
    const now = Date.now();
    for (const investor of state.investors) {
      if (stopping) break;
      const lastExt = state.lastExtended[investor] ?? 0;
      if (now - lastExt < EXTEND_THRESHOLD_MS) continue;

      try {
        await extendBalanceTtl(net, operator, contractId, investor);
        state.lastExtended[investor] = now;
        saveState(state);
        console.log(`[ttl-watchdog] TTL renovado: ${investor}`);
      } catch (err) {
        console.error(`[ttl-watchdog] falha ao renovar TTL de ${investor}:`, err);
      }
    }

    if (!stopping) await Bun.sleep(TICK_MS);
  }

  console.log("[ttl-watchdog] encerrado");
}

run();
