# Divvyr

_Divvyr — Stocklana Hackathon (Solana Foundation), deadline Sep 18, 2026_

## One-liner

Receive your tokenized stock dividends in any crypto you want, and turn that income stream into a giftable, tradeable NFT for anyone you choose.

## Problem

Tokenized stocks on Solana (xStocks, etc.) pay dividends by rebasing your token balance via a multiplier, you never actually see cash, and there's no clean way to do anything with that income except let it sit as more locked equity. There's also no simple way to share equity income with someone else without giving up your actual shares. If you want to support a family member, gift a friend some passive income, or just want your dividends as spendable stablecoins instead of illiquid rebased exposure, there's currently no product for that.

## The idea, two pillars

### 1. Multi-asset dividend harvesting

- Watch the xStock's Token2022 Scaled UI multiplier for corporate-action deltas (dividends, splits, reverse splits)
- On a dividend delta, calculate the token amount it represents and swap that slice through Jupiter into whatever crypto the user wants to receive, SOL, USDC, or other supported assets, not locked to one payout currency
- Underlying stock position and price exposure are untouched, only the yield slice ever gets converted
- Splits and reverse splits are detected and handled distinctly from dividends (not treated as a payout event)

### 2. Shareable, tradeable dividend-rights NFTs

- A holder can "strip" their xStock position into a vault, and mint a **Yield NFT** representing a % share of future dividends from that position
- The Yield NFT is a bearer instrument: whoever holds it _at the moment a dividend fires_ receives that share, automatically, swapped to their chosen currency and sent to their wallet
- Gift it directly to a friend or family member's wallet, or list it on a marketplace, income follows the NFT, no claim step needed
- Each Yield NFT has:
  - a **share %** of the dividend stream
  - an **expiry**, set as either time-bound ("pays out for 6 months") or count-bound ("captures the next 3 payouts")
  - a **pre-committed SOL buyout price**, set by the original owner at strip time, letting them reclaim full rights early by compensating the current holder a known, transparent amount
- The original owner also holds a **Principal NFT/claim**, representing the underlying stock plus any unallocated yield, tradeable separately from the Yield NFT(s) if they want liquidity without needing to buy back yield rights
- At expiry, a Yield NFT's share automatically reverts to the owner's unallocated pool, no action required from anyone

## Why this is hard to copy in a week

Most teams working with xStocks on Solana will land on dividend harvesting as their first idea, since the multiplier mechanism basically hands you the concept. Multi-asset payout via Jupiter is a solid baseline, but the Yield NFT layer is the actual differentiator: it's not a second feature bolted on, it reuses the same harvesting engine and just changes who the payout resolves to at swap time. That makes it cheap to build relative to how distinct it makes the submission.

## Why Solana

- Token2022's Scaled UI Amount Config is the exact multiplier hook this depends on, purpose-built for this use case
- Jupiter gives a single, reliable swap layer to any payout asset without building custom liquidity routing per token pair
- Sub-second finality and low fees make "dividend lands in your wallet within seconds of the multiplier update" a real, demoable claim, not a marketing line

## Open design questions (for the team to weigh in on)

- Multiple Yield NFTs per vault: cap total allocated % (e.g., can't exceed 100% across all outstanding NFTs)?
- Early buyout: paid fully by owner in SOL, or partially from unclaimed accrued yield?
- Do we support re-stripping (owner strips again after a full unwind) in this build, or treat that as future work?
- Default payout asset if user hasn't chosen one, USDC as the safe default?

## Not in scope for hackathon build

- Full principal-token marketplace/pricing engine
- Cross-chain payout routing
- Real xStocks (KYC-gated) — build and demo against a mock Token2022 multiplier-enabled test token
