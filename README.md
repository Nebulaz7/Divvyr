# Divvyr — 5-Day Build Plan
**Stocklana Hackathon (Solana Foundation) — submissions close Friday Sep 18, 4:00pm ET**
Team: Nebulaz (smart contracts + frontend), James (backend)
Assumption: ~14 hrs/day each, Sep 13 → Sep 17 build, Sep 18 morning = buffer + submission

---

## 1. Tech stack (researched, not guessed)

| Layer | Choice | Why |
|---|---|---|
| Program framework | **Anchor** (latest stable) | Handles account validation/serialization boilerplate, standard for hackathon judging familiarity |
| Token standard | **Token2022 with Scaled UI Amount extension** | This IS how xStocks represent dividends/splits — a multiplier applied to raw balance, not a real transfer. Docs: solana.com/docs/tokens/extensions/scaled-ui-amount |
| NFT standard | **Metaplex Core** | Cheaper and simpler than legacy Token Metadata (single account per NFT instead of multiple), faster to mint programmatically via CPI, good fit for a time-boxed build |
| Swap | **Jupiter**, via Swap API + `maxAccounts` cap, called as a **separate on-chain instruction**, not bundled inside the harvest instruction | See critical correction in section 2 below — this is the single biggest technical risk in the whole project if not handled right |
| Backend/keeper | **Node.js/TypeScript**, Helius webhooks or RPC polling | Matches James's existing backend/API strength, Helius gives account-change webhooks so you're not brute-force polling |
| Devnet test token | Custom SPL token minted with `--program-id TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb create-token --ui-amount-multiplier 1.0` | Real xStocks are KYC-gated and not freely testable — mock your own multiplier-enabled token that behaves identically |
| Frontend | Next.js + wallet adapter + Tailwind (your existing stack) | No change needed from what you already know |
| Indexer/API | Lightweight Postgres or SQLite + a sync service reading your program's accounts | Keeps frontend fast without hammering RPC directly |

---

## 2. Critical accuracy correction: Jupiter swap can't just be "CPI'd inside harvest"

This is worth reading carefully before anyone writes code, because it will break your demo at the worst moment if missed.

Jupiter's own docs flag that **CPI-based swaps are prone to failing at runtime** because Solana caps transaction size at 1,232 bytes, and Jupiter's best routes often span multiple DEXes, needing more accounts than fit in one transaction alongside your own program's logic. Their own guidance: cap `maxAccounts` in the Swap API quote request to keep the route small enough to fit inside your own instruction, accepting slightly worse price execution in exchange for reliability.

**Correct pattern for Divvyr (3 discrete on-chain steps, not 1 atomic step):**

1. **`harvest_dividend`** — reads the multiplier delta on the watched mint, moves the delta-equivalent raw xStock tokens from the vault into a temporary "pending distribution" token account owned by the vault PDA. No swap happens here.
2. **`execute_swap`** — your program CPIs into Jupiter's swap program (via the `jupiter-cpi` crate) using the vault PDA as signer, swapping the pending-distribution balance into the user's chosen payout asset. Request the quote with a **capped `maxAccounts`** so it reliably fits in one transaction. This step is technically the riskiest part of the whole build — budget your best engineering time here, and have your teammate test it early, not on day 4.
3. **`distribute_payout`** — reads current owner of every non-expired Yield NFT tied to the vault, splits the swapped-in amount by percentage, transfers directly to each holder's wallet, and routes any remainder/expired shares back to the vault owner.

Three separate instructions, likely triggered back-to-back by your backend keeper, not one mega-transaction. This is both more reliable AND easier to demo step-by-step to judges ("here's the harvest, here's the swap, here's the payout landing in my friend's wallet").

**Second accuracy note**: the Scaled UI Amount extension currently has a known idiosyncrasy — if you schedule a new multiplier update before a previous scheduled update has taken effect, the earlier one gets silently overridden rather than applied first. If your test token needs two multiplier changes close together (e.g. simulating two dividends), you need to explicitly re-set the prior multiplier at its original timestamp before setting the new one. Build your test script around this from day 1 so you don't lose hours debugging "my second test dividend never applied."

---

## 3. Architecture at a glance

```
[Mock xStock mint, Scaled UI Amount enabled]
              │  multiplier update (simulated dividend)
              ▼
[James: Keeper service] ──detects delta──▶ calls harvest_dividend
              │
              ▼
[Anchor Program: StripVault]
   ├─ harvest_dividend  → moves delta to pending-distribution account
   ├─ execute_swap      → CPI to Jupiter (maxAccounts capped) → payout asset
   ├─ distribute_payout → reads Yield NFT owners → splits → transfers
   ├─ strip_position    → locks xStock, mints Yield NFT(s) + Principal NFT (Metaplex Core CPI)
   ├─ buyout            → owner pays pre-set SOL amount → reclaims yield share early
   └─ check_expiry       → time or count based → reverts share to owner
              │
              ▼
[James: Indexer/API] ──reads program accounts──▶ [Nebulaz: Frontend]
                                                    strip / gift / view dashboard / buyout
```

---

## 4. Day-by-day plan

### Day 1 (Sep 13) — Foundations, in parallel

**You (contracts + frontend) — 14h**
- Set up Anchor project, repo structure, devnet wallet + airdrop devnet SOL
- Mint the mock Token2022 Scaled UI Amount test token via CLI, confirm you can update its multiplier and read the resulting UI amount correctly
- Write and test the multiplier double-update workaround from section 2 as a standalone script, before it's inside any program logic
- Scaffold the `StripVault` program: account structures only (vault state, Yield NFT metadata fields, Principal claim fields) — no business logic yet
- Scaffold Next.js frontend shell, wallet adapter connected, empty dashboard route

**James (backend) — 14h**
- Set up Node/TS backend project, connect to devnet RPC and Helius (or plain polling as fallback)
- Build a standalone script that watches the mock token's multiplier account and logs a delta the moment you manually update it — prove the detection loop works before anything else depends on it
- Design the indexer schema: vaults, yield NFTs (share %, expiry type/value, buyout price, current owner), payout history
- Stub the API endpoints frontend will need (get user vaults, get NFT details, get payout history) with mock data so frontend work isn't blocked later

**Sync point (end of day):** confirm multiplier-watch detection works reliably, and both of you agree on the exact account/data shapes so nobody builds against a guess.

---

### Day 2 (Sep 14) — Core program logic

**You — 14h**
- Implement `strip_position`: lock xStock into vault PDA, mint Principal NFT + Yield NFT(s) via Metaplex Core CPI, encode share %, expiry (time/count), buyout price into metadata
- Implement `harvest_dividend`: detect delta, move equivalent raw tokens to pending-distribution account
- Write unit tests for both against your mock token on devnet
- Start on `check_expiry` logic (time-based first, it's simpler)

**James — 14h**
- Turn the standalone watcher script into a real keeper service: on detected delta, call `harvest_dividend` automatically
- Build the indexer sync job: read all vault + NFT accounts on an interval, write to your database
- Implement the "get user's vaults/NFTs" API endpoint against real (not mock) indexed data

**Sync point:** you hand James the actual account layouts from your now-implemented `strip_position`/`harvest_dividend` so his indexer reads real fields, not placeholders.

---

### Day 3 (Sep 15) — The risky part: swap + distribution

**You — 14h**
- Implement `execute_swap`: CPI to Jupiter via `jupiter-cpi` crate, capped `maxAccounts`, vault PDA as signer. **Start this first thing in the morning** — it's the highest-risk piece in the whole build, per section 2, and you want failure cycles happening now, not on day 4.
- Implement `distribute_payout`: enumerate non-expired Yield NFTs for a vault, read current owner of each, split swapped amount by %, transfer, route remainder to vault owner
- Implement count-based expiry (on top of time-based from day 2)
- If `execute_swap` proves unreliable even with capped accounts by midday, fall back plan: swap via Jupiter's HTTP API off-chain, executed as its own signed transaction by a backend-held authority the vault delegates to, rather than in-program CPI. Flag this decision to James immediately if you take the fallback, since it changes what he's building around.

**James — 14h**
- Extend keeper service to trigger `execute_swap` and `distribute_payout` right after a successful harvest, so a dividend event flows end-to-end without manual intervention
- Add payout history tracking to the indexer (which NFT holder got paid what, when)
- Start building the buyout flow's backend support: endpoint to fetch a Yield NFT's current buyout price for the frontend to display

**Sync point:** run one full end-to-end dividend event together (multiplier update → harvest → swap → payout lands in a test wallet) before the day ends. If this doesn't work today, day 4 gets reprioritized around fixing it before anything else.

---

### Day 4 (Sep 16) — Frontend, buyout, gifting, polish

**You — 14h**
- Implement `buyout`: owner pays pre-set SOL amount, reclaims Yield NFT rights, vault share reallocated
- Build frontend flows: strip a position, view your Principal/Yield NFTs, gift/send a Yield NFT to a wallet, trigger buyout, dashboard showing payout history (pulling from James's API)
- Polish the "gift" UX specifically — this is your headline differentiator, it needs to feel effortless, not like a raw NFT transfer

**James — 14h**
- Finish buyout backend support (price calc if formula-based, not just flat)
- Harden the keeper service: error handling, retry logic, logging, so it survives a live demo without silently failing
- Load-test the indexer against several vaults with multiple NFTs each, confirm the dashboard queries stay fast

**Sync point:** full walkthrough of every user-facing flow together, end to end, on the actual frontend, not just via scripts/CLI.

---

### Day 5 (Sep 17) — Hardening, demo, pitch

**You — 14h**
- Bug fixes from day 4's walkthrough
- Record the demo video: strip a position, gift the Yield NFT, trigger a simulated dividend, show payout landing in the recipient's wallet in their chosen asset, show a buyout, show expiry reverting a share
- Write the submission README (problem, two-pillar pitch, why Solana, architecture diagram, link to demo video)
- Deploy final program build to devnet (or mainnet only if Stocklana explicitly requires it — confirm this early, don't assume)

**James — 14h**
- Final indexer/API hardening, make sure nothing breaks mid-demo-recording
- Help stress-test the demo script by acting as the "recipient" in a second wallet during the recording
- Write the technical section of the README covering the keeper/indexer architecture

**Sync point:** watch the recorded demo together before calling it done, fix anything that reads confusingly to someone seeing it cold.

---

### Sep 18 (submission day, before 4:00pm ET)

- Final review of the submission form: include GitHub repo link, demo video, and/or live devnet link (per Stocklana's "at least one link: GitHub, live demo, or video" rule)
- Invite teammates from the submit form
- Submit with margin before the deadline, don't cut it to the final minutes, remember edits are allowed until submissions close so submitting early and refining after is safer than rushing at 3:55pm

---

## 5. What to explicitly NOT build (keep scope honest)

- Real xStock integration (KYC-gated, not testable in this window) — mock token throughout, state clearly in your pitch that swapping to real xStocks is a config change, not a rearchitecture
- Full principal-token marketplace/pricing engine
- Stealth addresses / privacy layer — if there's spare time on day 5 only, mention Token2022 Confidential Transfers as roadmap, don't attempt to implement
- Re-stripping after a full unwind
- Cross-chain anything

## 6. Biggest risks, ranked

1. **Jupiter CPI reliability** (section 2) — mitigated by tackling it first on day 3, with a known fallback path
2. **Multiplier double-update quirk** — mitigated by testing it standalone on day 1 before it's buried inside program logic
3. **Running out of demo time on day 5** — mitigated by having a working end-to-end flow by end of day 3, so days 4-5 are UX and polish, not core functionality
4. **Metaplex Core CPI unfamiliarity** — if this eats more time than expected on day 2, fall back to simpler Token Metadata NFTs, slightly more expensive per mint but better-documented
