# SGR Fund Contract Introduction

The contract is the **financial heart of SGR**. It manages an investment fund backed by rental properties and Brazilian Treasury bonds (via Etherfuse) as reserve. Think of it as a smart vault with automatic rules written in code on the Stellar blockchain.

---

## The participation unit: the MUTAV token

Investors in the fund receive **MUTAV tokens** in return. MUTAV represents your share of the fund — the more the fund earns, the more each MUTAV is worth. It works like a real estate investment fund share, but digital and traceable on the blockchain.

---

## Who controls what

The contract has two control profiles with different permissions:

- **Owner (cold wallet):** The "controlling partner". Handles important, irreversible decisions — changing who the responsible parties are, registering defaults, configuring new contracts. Uses a wallet stored in a secure location, accessed infrequently.

- **Operator (hot wallet):** The "day-to-day manager". Executes routine operations — receiving payments, processing withdrawals, recording yields. Uses a more accessible wallet, but with limited powers.

Separating the two prevents a compromised operator wallet from granting full control of the fund.

---

## Features

### 1. Initial setup

When the contract is deployed on the blockchain, it needs to be configured once: who is the owner, who is the operator, which currency is accepted (USDC), where fund money goes, and a set of financial parameters such as:

- How much can be withdrawn per week (redemption limit)
- The monthly management fee
- The fee charged on redemption
- The protocol's share of each incoming payment

These parameters can be updated after deployment by the owner (admin), within the same maximum limits enforced at initialization.

---

### 2. Approved partner check

Each fund maintains its own whitelist of approved partner agencies. Before accepting a payment, the contract checks whether the agency is on this fund's whitelist — if not, the payment is rejected.

The owner manages the whitelist via `set_approved_partner`. This is also how reallocation works when an agency's risk score changes tier: the owner removes the agency from the current fund and adds it to the fund that matches the new tier. Agencies already being covered by the old fund continue normally until their contracts expire.

---

### 3. Receiving payment from a real estate agency

Each month, real estate agencies pay MUTAV a guarantee fee — the pooled amount collected from their tenants as part of the rental guarantee service. When this payment lands in MUTAV's bank account and the team completes the on-ramp, the operator records it in the contract. The amount is automatically split:

- A small portion goes to the protocol (the company maintaining the system)
- The remainder goes to the fund wallet, which will be converted into Treasury bonds via Etherfuse

The fund's total assets under management (AUM) increase by the portion retained for the fund.

**How the numbers change:**

| | Before | After |
|---|---|---|
| Payment received | — | 100,000 USDC |
| Protocol share (20%) | — | − 20,000 USDC |
| AUM | 1,000,000 USDC | 1,080,000 USDC |
| Supply (tokens) | 1,000,000 MUTAV | 1,000,000 MUTAV |
| NAV (AUM ÷ Supply) | 1.00 USDC | **1.08 USDC** |

The supply does not change — no new tokens are created. Only the AUM rises, so each existing MUTAV becomes worth more.

---

### 4. Investor deposits funds

An investor sends USDC and receives MUTAV tokens in return. The amount of MUTAV received depends on the current fund value — if the fund has already appreciated, each MUTAV is worth more, so the investor receives fewer tokens for the same USDC (but each token is worth more). The deposited USDC goes directly to the fund wallet, which converts it into Treasury bonds. The contract does not hold USDC itself.

**How the numbers change:**

> Formula: tokens received = USDC deposited × current supply ÷ current AUM

| | Before | After |
|---|---|---|
| Investor deposit | — | 100,000 USDC |
| AUM | 1,080,000 USDC | 1,180,000 USDC |
| Supply (tokens) | 1,000,000 MUTAV | 1,092,592 MUTAV |
| NAV (AUM ÷ Supply) | 1.08 USDC | **1.08 USDC** |

The investor received 92,592 MUTAV (fewer than the USDC deposited) because each MUTAV was already worth 1.08. The NAV does not change — the deposit is proportional to the current assets, so it neither dilutes nor appreciates the other investors.

---

### 5. Requesting a redemption

An investor who wants to exit the fund requests redemption of their MUTAV tokens. Those tokens are **locked** immediately (removed from their available balance) and placed in a queue. The exit price **is not calculated now** — it will be calculated when the operator processes the queue, ensuring everyone exits at the fair value on the day of execution.

**How the numbers change at the time of the request:**

No change in AUM or NAV yet. The tokens simply leave the investor's available balance and are locked.

**How the numbers change when the operator processes the queue:**

> Formula: USDC to receive = MUTAV redeemed × current AUM ÷ current supply

| | Before | After |
|---|---|---|
| MUTAV redeemed | — | 100,000 MUTAV |
| AUM | 1,080,000 USDC | 972,000 USDC |
| Supply (tokens) | 1,000,000 MUTAV | 900,000 MUTAV |
| NAV (AUM ÷ Supply) | 1.08 USDC | **1.08 USDC** |

The investor will receive 108,000 USDC (100,000 × 1.08). AUM and supply decrease proportionally, so the NAV for other investors remains the same.

---

### 6. Cancelling a redemption request

If the investor changes their mind before the operator processes the queue, they can cancel. The locked tokens are returned to their balance instantly, without affecting the queue for other investors.

---

### 7. Processing the redemption queue

The operator runs this function periodically (typically once a week). The contract:

1. Checks how much of the assets can exit that week (e.g., a 2.5% limit)
2. Serves investors in order of arrival until the limit is reached
3. Calculates the USDC value of each request using the day's NAV
4. Burns the corresponding MUTAV tokens
5. Records how much USDC needs to be retrieved from Etherfuse to pay each one
6. Sets a deadline for payment to occur

Investors who don't fit within the week's limit remain in the queue for the next round.

---

### 8. Paying the investor

After the operator retrieves USDC from Etherfuse and deposits it into the contract, they trigger this function for each investor in the payment queue. The contract deducts a small redemption fee and sends the remainder directly to the investor's wallet. If the deadline passes without the operator paying, the investor has an alternative protection path (see below).

---

### 9. Redemption after deadline expiry (investor protection)

If the operator has not paid within the configured deadline, the investor can trigger this safety mechanism on their own. Their MUTAV tokens are **restored** as if the redemption had never occurred, and the fund's assets are corrected back. This protects the investor from being stranded without payment or their tokens in case the backend fails.

---

### 10. Recording Treasury yield

The operator periodically records the yield received via Etherfuse (Treasury bond interest). This increases the fund's assets and consequently the NAV — meaning each MUTAV becomes worth more. There is a per-call limit (e.g., a maximum of 5% of current assets at once) to prevent manipulation.

**How the numbers change:**

| | Before | After |
|---|---|---|
| Yield recorded | — | + 10,000 USDC |
| AUM | 1,000,000 USDC | 1,010,000 USDC |
| Supply (tokens) | 1,000,000 MUTAV | 1,000,000 MUTAV |
| NAV (AUM ÷ Supply) | 1.00 USDC | **1.01 USDC** |

The supply does not change — no new tokens are created. The yield alone increases the AUM and appreciates each existing MUTAV.

---

### 11. Recording a tenant fee received

Works the same as yield recording, but specifically for income derived from the rental guarantee contracts themselves — fees earned as part of the guarantee service, distinct from Treasury bond yield. Also has a per-call limit to prevent manipulation.

---

### 12. Charging the monthly management fee

Once a month (with a minimum 30-day interval), the operator charges the fund's management fee. This reduces the assets — the AUM shrinks slightly, causing the NAV to dip. The actual payment happens off-chain via Etherfuse; the contract only records the accounting deduction.

**How the numbers change:**

> Formula: fee = AUM × management_fee_bps ÷ 10,000 (e.g., 100 bps = 1%)

| | Before | After |
|---|---|---|
| Management fee charged (1%) | — | − 10,000 USDC |
| AUM | 1,000,000 USDC | 990,000 USDC |
| Supply (tokens) | 1,000,000 MUTAV | 1,000,000 MUTAV |
| NAV (AUM ÷ Supply) | 1.00 USDC | **0.99 USDC** |

The supply does not change. Only the AUM falls, so each MUTAV is worth slightly less — the management cost is distributed proportionally among all investors.

---

### 13. Recording an off-chain payment

When the fund makes an operational payment off-chain (such as reimbursements or administrative expenses that are not claim payouts), the operator records it here for audit purposes: the AUM decreases, and the destination address is stored on the blockchain. This function is distinct from covering a default — that one is owner-only and specifically for approved claims. This one is for other off-chain outflows that the operator is responsible for.

**How the numbers change:**

| | Before | After |
|---|---|---|
| Off-chain payment recorded | — | − 50,000 USDC |
| AUM | 1,000,000 USDC | 950,000 USDC |
| Supply (tokens) | 1,000,000 MUTAV | 1,000,000 MUTAV |
| NAV (AUM ÷ Supply) | 1.00 USDC | **0.95 USDC** |

Identical behavior to the management fee: only the AUM falls, the supply stays the same, and the NAV reflects the actual outflow from the fund.

---

### 14. Covering a default

When a tenant defaults and the claim is analyzed and approved by MUTAV, the **owner** (not the operator) triggers this function to register the coverage. The assets are reduced by the approved amount, and the destination address is recorded for traceability. The fund transfers the value to MUTAV's classic wallet, which then forwards it to the real estate agency via bank transfer.

**How the numbers change:**

| | Before | After |
|---|---|---|
| Default coverage | — | − 30,000 USDC |
| AUM | 1,000,000 USDC | 970,000 USDC |
| Supply (tokens) | 1,000,000 MUTAV | 1,000,000 MUTAV |
| NAV (AUM ÷ Supply) | 1.00 USDC | **0.97 USDC** |

A default is a real loss for the fund: AUM drops and every investor's NAV drops with it. That is why this function requires the owner (cold wallet) — it is a high-impact decision.

---

### 15. Emergency pause

The owner can pause the contract at any time — for example, if a critical vulnerability is discovered or while an audit is in progress. When paused, all fund operations are blocked: no new deposits, no redemption requests, no yield recording, no fee charges.

However, two functions remain available even while paused: cancelling a redemption request and reclaiming an expired redemption. This guarantees that investors can always recover their funds regardless of the contract's state — they are never locked out.

To resume normal operations, the owner unpauses the contract with the same function.

---

### 16. Contract ownership transfer

To prevent a typo from transferring control of the fund to a non-existent address (locking everything permanently), changing ownership works in **two steps**:

1. The current owner nominates a new address
2. The new address must **confirm** that it accepts control

Only after confirmation does control transfer. If the nominated address is wrong, the owner simply nominates another.

---

### 17. Replacing the operator

The owner can replace the operator at any time (for example, if the hot wallet is compromised). The new address immediately assumes operational permissions.

---

### 18. MUTAV token operations

MUTAV follows Stellar's token standard (SEP-0041), meaning it works like any other token on the network:

- **Transfer:** Send MUTAV from one wallet to another
- **Approve spending:** Authorize another wallet to spend your tokens (for use in other contracts or platforms)
- **Burn:** Destroy tokens directly — assets are reduced proportionally so the NAV of other investors does not change
- **Query balance and decimals:** Standard read functions

---

### 19. Public queries

The contract exposes various pieces of information that anyone can query at no cost:

| Query | What it returns |
|---|---|
| NAV | Current value of each MUTAV token in USDC |
| AUM | Total fund assets under management |
| Total supply | How many MUTAV tokens exist |
| Address balance | How many MUTAV a specific wallet holds |
| Pending request | How many MUTAV an investor has awaiting processing |
| Ready for redemption | How much USDC an investor has to receive |
| Redemption deadline | The timestamp by which the operator must pay a processed redemption |
| Queue length | How many investors are currently waiting in the redemption queue |
| Available this week | How much can still be redeemed in the current cycle |
| Parameters | Fees, limits, and payment window as configured |
| Paused | Whether the contract is currently paused |
| Approved partner | Whether a specific agency is on this fund's whitelist |

---

### 20. On-chain data maintenance

On Stellar, data stored on-chain expires if not renewed periodically. The contract has functions for this:

- The operator renews the fund's global data approximately every 25 days
- Anyone can renew a specific investor's balance record — useful for investors who go a long time without moving their wallet
- Anyone can renew a specific investor's pending or ready redemption entries — useful when a redemption is sitting in queue for an extended period

---

## The full flow in one sentence

A real estate agency pays the monthly guarantee fee → the fund receives and invests in Treasury bonds → MUTAV tokens appreciate → the investor requests redemption → the fund processes it that week, calculates the fair value, converts the Treasury bonds back to USDC → and pays the investor, deducting a small fee.
