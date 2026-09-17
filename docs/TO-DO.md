# Divvyr — Project To-Do & Task Tracker
**Target Deadline: September 18, 2026, 4:00 PM ET (Stocklana Hackathon)**

---

## 📌 Critical Pre-Demo & Metadata Tasks

- [ ] **Configure Project Metadata URIs & Assets:**
  - Create standard JSON metadata files for both:
    1. **Master Deed NFT** (`master-deed.json`): Name, symbol, image preview, locked stock attributes.
    2. **Yield NFT** (`yield-ticket.json`): Name, symbol, image preview, share %, expiry condition, buyout price.
  - Decide on hosting provider (Arweave via Irys, IPFS via Pinata/NFT.Storage, or raw GitHub Pages in repo).
  - Pass the decided URIs into frontend/script initialization calls.

---

## 🛠️ Smart Contracts (`contracts/programs/strip_vault`)

- [x] **Part 1: Scaffolding, State Layout & Math Foundation**
  - [x] Anchor setup and Token-2022 + `mpl-core` dependencies
  - [x] `Vault` state with fixed offsets and `_reserved: [u8; 64]`
  - [x] `YieldRecord` state with `_reserved: [u8; 32]`
  - [x] Scaled UI dividend harvest math + 7 passing unit tests
- [x] **Part 2: Vault Initialization & Master Deed Minting**
  - [x] Token-2022 Scaled UI Amount on-chain TLV unpacker
  - [x] Dynamic `name` and `uri` arguments (no hardcoded URLs)
  - [x] `creator_initialize_vault` with Token-2022 transfer and Metaplex Core CPI
- [x] **Part 3: Position Stripping & Yield NFT with Burn Delegate**
  - [x] Implement `owner_strip_position`
  - [x] Validate Master Deed ownership on-chain
  - [x] Attach Metaplex Core `PermanentBurnDelegate` plugin to Yield NFT (held by Vault PDA)
  - [x] Initialize `YieldRecord` PDA and enforce <= 100% total yield cap
- [x] **Part 4: The 3-Step Dividend Engine**
  - [x] `keeper_harvest_dividend` (effective timestamp validation and delta calculation)
  - [x] `keeper_execute_swap` (deterministic simulated swap engine)
  - [x] `keeper_distribute_payout` (passive expiry + strict recipient validation)
- [x] **Part 5: Buyout & Principal Unwind**
  - [x] `owner_buyout_yield` (SOL payment to holder + delegated burn of Yield NFT)
  - [x] `owner_redeem_principal` (burn Master Deed + unlock underlying stock)

---

## ⚡ Backend & Keeper Service (James — `backend/`)

- [ ] Devnet mock Token-2022 mint setup with `--ui-amount-multiplier 1.0`
- [ ] Multiplier watcher service (detects multiplier bumps via Helius webhook or devnet polling)
- [ ] Automated 3-step crank:
  1. Call `keeper_harvest_dividend`
  2. Call `keeper_execute_swap`
  3. Call `keeper_distribute_payout`
- [ ] Lightweight Indexer (PostgreSQL or SQLite) syncing vaults, yield NFTs, and payout history
- [ ] REST API endpoints for frontend consumption

---

## 💻 Frontend Application (Nebulaz — `frontend/`)

- [ ] Next.js shell with Tailwind CSS
- [ ] Modern `@solana/kit` and `@solana/react` wallet connection (Wallet Standard)
- [ ] User Dashboard:
  - My Master Deeds (locked stock, active yield %, unclaimed yield, buyout actions)
  - My Yield NFTs (received dividend streams, next expected payout, gift/send action)
- [ ] "Strip Position" modal (deposit xStock, configure %, expiry type, SOL buyout price)
- [ ] "Gift / Send Yield Ticket" UX flow
- [ ] "One-Click Buyout" interaction
- [ ] "Simulate Corporate Action / Dividend" admin button (triggers live multiplier bump on devnet for the demo)

---

## 🚀 Hackathon Submission & Demo

- [ ] Deploy finalized `strip_vault` program to Devnet
- [ ] Test full end-to-end user journey live on Devnet
- [ ] Record 3-minute video walkthrough (strip position $\rightarrow$ gift yield $\rightarrow$ trigger dividend $\rightarrow$ payout arrives $\rightarrow$ buyout)
- [ ] Finalize submission text and README overview
- [ ] Submit before deadline (Sep 18, 4:00 PM ET)
