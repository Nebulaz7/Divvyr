# Divvyr — Phased Contract Implementation Plan
**Program:** `strip_vault`  
**Execution Strategy:** Step-by-Step Modular Build with Checkpoints

---

## Roadmap at a Glance

We will build the smart contract in **5 focused, sequential parts**. Each part has a dedicated scope, specific files to write, and an automated verification checkpoint before moving to the next:

```
Part 1: Scaffolding, State Layout & Multiplier Math (Phase 1A)
  └── Checkpoint: Cargo unit tests pass for Scaled UI math & account layouts

Part 2: Vault Initialization & Master Deed Minting (Phase 1B - Part 1)
  └── Checkpoint: Token-2022 deposit & Metaplex Core Master Deed mint CPI verified

Part 3: Position Stripping & Yield NFT with Burn Delegate (Phase 1B - Part 2)
  └── Checkpoint: Yield NFT minted with Vault PDA BurnDelegate plugin & bps allocation enforced

Part 4: The 3-Step Dividend Engine (Harvest -> Simulated Swap -> Distribute) (Phase 1C)
  └── Checkpoint: Full dividend simulation cycle (effective multiplier check -> delta harvest -> reserve swap -> holder verification)

Part 5: Buyout & Principal Redemption (Phase 1D & 1E)
  └── Checkpoint: SOL buyout triggers delegated burn of Yield NFT; principal redemption unwinds vault
```

---

## Detailed Breakdown of Each Part

---

### Part 1: Scaffolding, State Layout & Multiplier Math
**Goal:** Establish the Anchor project foundation, data structures, error codes, and the core mathematical engine for non-dilutive dividend harvesting.

- **Files to Create:**
  1. `Anchor.toml` & `Cargo.toml` (workspace level)
  2. `programs/strip_vault/Cargo.toml`:
     - Dependencies: `anchor-lang = "0.30"`, `anchor-spl` (with `token_2022`), `mpl-core`, `spl-token-2022`, `solana-program`
  3. `programs/strip_vault/src/lib.rs`: Program scaffold and instruction routing
  4. `programs/strip_vault/src/errors.rs`: `StripVaultError` enum
  5. `programs/strip_vault/src/state/mod.rs`
  6. `programs/strip_vault/src/state/vault.rs`: `Vault` struct, `PayoutAssetType` enum, upgrade padding (`_reserved: [u8; 64]`)
  7. `programs/strip_vault/src/state/yield_record.rs`: `YieldRecord` struct, `ExpiryCondition` enum, padding (`_reserved: [u8; 32]`)
  8. `programs/strip_vault/src/utils/math.rs`: Fixed-point calculation for harvesting raw shares from multiplier deltas:
     $$\Delta_{\text{tokens}} = \frac{\text{raw\_balance} \times (M_{\text{new}} - M_{\text{old}})}{M_{\text{new}}}$$
- **Verification Checkpoint:**
  - Run unit tests: `cargo test-sbf` or `cargo test --lib`
  - Assert the worked proof: 100 raw shares @ $1.00 \rightarrow 1.05$ multiplier produces exactly $4.761904$ harvest tokens and preserves 100.00 UI shares in the vault.

---

### Part 2: Vault Initialization & Master Deed Minting
**Goal:** Enable users to lock their Token-2022 stock in the vault and receive their Master Deed NFT (Principal Claim).

- **Files to Create:**
  1. `programs/strip_vault/src/utils/metaplex_core.rs`: CPI helper for `mpl_core::instructions::CreateV1` / `CreateV2`
  2. `programs/strip_vault/src/instructions/creator_initialize_vault.rs`:
     - Validate Token-2022 stock mint and unpacked `ScaledUiAmountConfig`
     - Transfer raw stock from creator to `stock_vault_ata` via Token-2022 CPI
     - Mint Master Deed NFT via Metaplex Core CPI directly to creator
     - Initialize `Vault` state account
  3. `programs/strip_vault/src/instructions/mod.rs`: Export instruction context
- **Verification Checkpoint:**
  - Test vault creation: verify raw tokens transfer to the vault PDA, and creator's wallet receives the Master Deed NFT asset ID.

---

### Part 3: Position Stripping & Yield NFT Minting with Burn Delegate
**Goal:** Enable the Master Deed holder to carve out future dividend percentages and mint bearer Yield NFTs with embedded burn delegation.

- **Files to Create:**
  1. `programs/strip_vault/src/instructions/owner_strip_position.rs`:
     - Authenticate that `ctx.accounts.signer.key() == master_deed_asset.owner`
     - Verify `vault.total_yield_allocated_bps + share_bps <= 10_000`
     - Metaplex Core CPI to mint Yield NFT to recipient with the **`BurnDelegate` plugin** attached (`authority = vault_pda`)
     - Initialize `YieldRecord` PDA
     - Update `vault.total_yield_allocated_bps`
  2. `programs/strip_vault/src/events.rs`: `YieldStrippedEvent`
- **Verification Checkpoint:**
  - Test stripping: verify Yield NFT is minted to recipient with the BurnDelegate plugin active, `YieldRecord` is initialized, and attempting to allocate >100% total yield reverts with `ExceedsMaxYieldAllocation`.

---

### Part 4: The 3-Step Dividend Engine (Harvest $\rightarrow$ Swap $\rightarrow$ Distribute)
**Goal:** Build the 3-step keeper pipeline that turns multiplier bumps into liquid USDC or SOL payouts for Yield NFT holders.

- **Files to Create:**
  1. `programs/strip_vault/src/instructions/keeper_harvest_dividend.rs`:
     - Inspect `ScaledUiAmountConfig`
     - Validate `Clock::get()?.unix_timestamp >= config.new_multiplier_effective_timestamp`
     - Compute delta raw tokens, transfer from `stock_vault_ata` to pending distribution ATA
     - Update `vault.last_multiplier` and `vault.pending_harvest_amount`
  2. `programs/strip_vault/src/instructions/keeper_execute_swap.rs`:
     - Deterministic simulated swap engine: converts `pending_harvest_amount` of mock stock into equivalent USDC/SOL in the vault payout account
     - Resets `vault.pending_harvest_amount = 0`
  3. `programs/strip_vault/src/instructions/keeper_distribute_payout.rs`:
     - Iterate through remaining accounts
     - Strict recipient validation: `require_keys_eq!(holder_wallet.key(), metaplex_asset.owner, StripVaultError::InvalidRecipientWallet)`
     - Passive expiry check (timestamp or payout count)
     - Proportional payout transfer to active Yield NFT holders; remainder/expired to Master Deed holder
- **Verification Checkpoint:**
  - Test 3-step sequence: trigger multiplier bump $\rightarrow$ harvest moves delta $\rightarrow$ simulated swap funds payout account $\rightarrow$ distribute splits USDC/SOL to holder and remainder to Master Deed holder.

---

### Part 5: Buyout & Principal Redemption
**Goal:** Implement the buyout mechanism (SOL payment + delegated burn) and full vault unwind.

- **Files to Create:**
  1. `programs/strip_vault/src/instructions/owner_buyout_yield.rs`:
     - Verify caller holds Master Deed NFT
     - Transfer `buyout_price_lamports` from caller to current Yield NFT holder
     - Vault PDA executes Metaplex Core `BurnV1` CPI as authorized `BurnDelegate`
     - Mark `YieldRecord.is_active = false`, subtract `share_bps` from `vault.total_yield_allocated_bps`
  2. `programs/strip_vault/src/instructions/owner_redeem_principal.rs`:
     - Require `vault.total_yield_allocated_bps == 0`
     - Burn Master Deed NFT via Metaplex Core CPI
     - Transfer remaining stock tokens back to owner and close vault accounts
- **Verification Checkpoint:**
  - Full lifecycle test: buyout pays SOL to holder and permanently burns the Yield NFT; principal redemption unlocks all original stock back to the owner.

---

## Current Step: Ready for Part 1

We will start with **Part 1: Scaffolding, State Layout & Multiplier Math**.  
Once you confirm, we will scaffold the Anchor project structure, define `Vault`, `YieldRecord`, error codes, and implement the multiplier math with unit tests!
