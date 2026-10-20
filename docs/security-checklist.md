# Security Review Checklist and Hardening Playbook

This document provides a structured checklist for performing internal and
external security reviews of Veritoken contracts and frontend. Each item is
specific to the current architecture. Use it before any mainnet deployment,
after significant contract changes, and as a briefing document for external
auditors.

For operational incidents (active exploits, key compromise), see
[docs/incident-response.md](incident-response.md).  
For deployment hardening, see [docs/mainnet-deployment.md](mainnet-deployment.md).

---

## How to use this checklist

Work through each section in order. For each item:

- **Pass** — the control is present and correct.
- **Finding** — the control is absent or incorrect. Record it with a severity
  (Critical / High / Medium / Low) and refer to the hardening playbook at the
  end of this document for remediation steps.
- **N/A** — the item does not apply to the contract or component under review.

Mark findings with the contract name and function so they can be tracked to a
fix.

---

## 1. Authorization

### Admin-only functions

- [ ] Every function that modifies global state (rules, admin address, verifier
  list, pause/unpause) calls `env.require_auth(&admin)` before any state
  change.
- [ ] The admin address is read from persistent storage, not passed as a
  parameter that a caller could forge.
- [ ] `propose_admin` / `accept_admin` two-step handover pattern is used —
  a single `set_admin` call cannot silently transfer control.
- [ ] After `accept_admin`, the old admin address loses privileges immediately
  with no grace window that could be exploited.

### Verifier-only functions

- [ ] `approve`, `revoke`, `reject` in `kyc-registry` check that the caller
  is in the verifier list before modifying any KYC record.
- [ ] `add_verifier` and `remove_verifier` are restricted to the admin — a
  verifier cannot grant verifier status to another address.
- [ ] `revoke_all_by_verifier` is restricted to the admin — a compromised
  verifier cannot self-revoke its trail of approvals.

### No privilege escalation paths

- [ ] No public function accepts an `admin` or `verifier` address as a
  parameter that is then used for authorization without first checking stored
  state.
- [ ] Constructor (`__constructor`) sets the admin atomically at deploy time
  with no post-deploy initialization window.
- [ ] The `AlreadyInitialized` guard prevents a second initialization call from
  replacing the admin after deployment.

### Admin recovery (RWA tokens)

- [ ] Recovery configuration and voting follow the documented method order and
  authorization rules. For the recovery test fixture, threshold rules, cooldown
  advancement, and the principal positive/negative cases, see
  [rwa-recovery-test-guide.md](rwa-recovery-test-guide.md).

---

## 2. Storage Safety

### TTL management

- [ ] Every public entry-point calls `env.storage().instance().extend_ttl()`
  at or near the top of the function body to keep the contract instance alive.
- [ ] Every persistent key is TTL-bumped in the same operation that writes it
  so freshly-written data is never at immediate risk of expiry.
- [ ] Read-heavy persistent keys (e.g. `Balance`, `KycStatus`) are also bumped
  on read, not only on write. (Known gap: Issues 8, 9, 15, 16, 17 in
  [storage-patterns.md](storage-patterns.md) — verify these are resolved or
  accepted as risk.)
- [ ] `THRESHOLD` and `BUMP` constants follow the pattern
  `BUMP = N * DAY_IN_LEDGERS` and `THRESHOLD = BUMP - DAY_IN_LEDGERS` to avoid
  redundant writes.

### Key design

- [ ] All storage keys use the typed `DataKey` enum — no raw string keys that
  could collide or be guessed.
- [ ] No two distinct logical entries share the same `DataKey` variant with
  different semantics.
- [ ] Temporary storage is used only for short-lived data (allowances,
  ephemeral flags) and is never relied upon for authoritative state.

### No reads of potentially archived keys without a prior extend

- [ ] Every code path that reads a persistent key either (a) bumps the TTL
  immediately before reading, or (b) is only called in contexts where the TTL
  is guaranteed to be fresh (e.g. directly after a write in the same
  invocation).

---

## 3. Transfer Flow

### Pre-transfer compliance checks

- [ ] Every `transfer` and `transfer_from` implementation calls
  `KycRegistry::is_approved` for **both** sender and receiver before any
  balance mutation.
- [ ] Every `transfer` and `transfer_from` calls
  `ComplianceEngine::can_transfer(from, to, amount)` before any balance
  mutation.
- [ ] The order is: KYC check → compliance check → balance debit → balance
  credit. No balance change occurs if either check fails.
- [ ] Neither check can be bypassed by passing `amount = 0` — zero-amount
  transfers should either be rejected outright or still pass through the full
  compliance pipeline.

### Error specificity

- [ ] Transfer failures return specific error codes (`KycNotApproved`,
  `TransferBlocked`, `AccountFrozen`) rather than a generic error, so callers
  can distinguish the cause without reading contract internals.
- [ ] Error codes do not leak internal state (e.g. the exact reason a compliance
  rule was triggered) beyond what is necessary for the caller to act.

### No bypass paths

- [ ] There is no admin-only `force_transfer` or `bypass_kyc_transfer` function
  that skips compliance checks.
- [ ] `mint` and `issue` functions that credit balances without a `transfer`
  still verify the recipient's KYC status before minting.
- [ ] The `redeem` function on invoice-token checks that the invoice is settled
  before allowing redemption — unsettled redemption is blocked.

---

## 4. Input Validation

### Numeric inputs

- [ ] All amount parameters are validated non-negative before any arithmetic.
  Negative amounts should return `NegativeAmount` immediately.
- [ ] Token amounts use `i128` with explicit overflow checks (`overflow-checks =
  true` is set in `[profile.release]` in the workspace `Cargo.toml`).
- [ ] `max_transfer_amount = 0` is correctly treated as "unlimited" (not as a
  limit of zero that blocks all transfers).

### String and address inputs

- [ ] Jurisdiction strings are validated non-empty before being stored.
- [ ] IPFS hash fields (invoice, property, carbon credit) are validated as
  non-empty strings on mainnet — placeholder values are not accepted.
- [ ] Addresses are validated as proper Stellar account IDs; malformed addresses
  cause an early error, not a silent storage of garbage data.

### Expiry timestamps

- [ ] KYC expiry values that are non-zero are validated to be in the future
  relative to the current ledger timestamp.
- [ ] Expiry `0` is explicitly treated as "no expiry" and not as "expired
  immediately".

### Compliance rule bounds

- [ ] `min_holding_period` is validated against the 365-day cap
  (`MinHoldingPeriodExceeds365Days` error).
- [ ] `max_holders = 0` is treated as "unlimited", not as "block all new
  holders".
- [ ] Risk scores are validated in [0, 100] (`InvalidRiskScore` error).

---

## 5. Upgrade and Deployment

### Constructor pattern

- [ ] All asset tokens use `__constructor` so admin, KYC registry, and
  compliance engine are set atomically at deploy time. There is no two-step
  deploy-then-initialize pattern that could be front-run.
- [ ] The `AlreadyInitialized` check is the first thing executed in any
  initializer path — before any state is read or written.

### Admin key hygiene

- [ ] The mainnet admin key is a hardware wallet (Ledger) or HSM — never a hot
  key stored on a server or in a `.env` file.
- [ ] The admin key mnemonic / seed phrase is stored offline in a fireproof
  safe and has never been stored digitally.
- [ ] Multi-signature is configured on the admin account (recommended: 2-of-3)
  to eliminate single points of failure.

### WASM integrity

- [ ] WASM binaries are optimized with `stellar contract optimize` before
  deployment to reduce fees and surface area.
- [ ] The WASM hash of the deployed contract is recorded and matches the hash
  of the artifact built from the tagged commit.
- [ ] `cargo build --release --target wasm32-unknown-unknown` and
  `cargo test --features testutils` both pass cleanly on the commit being
  deployed.

### Network configuration

- [ ] `--network mainnet` (not testnet) is used in all production deploy
  commands.
- [ ] Contract IDs are verified post-deployment using `stellar contract fetch`
  before configuring the frontend.
- [ ] Placeholder `--meta` values are replaced with real production data before
  deploying — no `"PLACEHOLDER"` strings appear in any mainnet contract.

---

## 6. Frontend Security

### Secrets and credentials

- [ ] No private keys, mnemonics, or secret values are present in
  `frontend/src/` or committed to version control.
- [ ] `frontend/.env` is in `.gitignore` and is never committed.
- [ ] Contract IDs come from `VITE_*` environment variables, not hardcoded.

### Transaction signing

- [ ] All transactions are signed by Freighter — the frontend never holds or
  uses a private key to sign on behalf of the user.
- [ ] The frontend never submits a transaction without first simulating it and
  displaying the resource cost to the user.
- [ ] Error messages shown to the user are derived from the contract's typed
  error codes — raw XDR or internal stack traces are not surfaced.

### Input handling

- [ ] Address inputs use `AddressInput` with the validation hook
  (`useAddressValidation`) before any contract call is constructed.
- [ ] Amount inputs are validated as positive numbers before being converted to
  stroops.
- [ ] Forms that trigger irreversible on-chain actions (settle, retire, redeem)
  use the `ConfirmDialog` component to require explicit confirmation.

---

## 7. Hardening Playbook

For each finding identified above, apply the matching remediation below.

---

### Finding: Admin function missing `require_auth`

**Severity:** Critical  
**Remediation:** Add `env.require_auth(&admin_addr);` as the first line of the
function body, where `admin_addr` is read from `env.storage().instance().get()`
— not from the function parameters. Re-run `cargo test --features testutils` and
add a test that asserts the function panics when called by a non-admin address.

---

### Finding: No `AlreadyInitialized` guard

**Severity:** Critical  
**Remediation:** Read the stored admin key at the start of the constructor. If
it is already set, call `panic_with_error!(env, ErrorType::AlreadyInitialized)`.
This prevents a second caller from re-initializing the contract with a different
admin after deployment.

---

### Finding: Persistent key not bumped on read

**Severity:** High  
**Remediation:** Add `env.storage().persistent().extend_ttl(&key, THRESHOLD, BUMP)`
immediately before or after reading the key. For balance reads that occur on
every transfer, this is especially important. See
[storage-patterns.md](storage-patterns.md) for the THRESHOLD/BUMP values used in
each contract.

---

### Finding: KYC check missing for recipient on `transfer`

**Severity:** High  
**Remediation:** Ensure the transfer implementation calls `is_approved` for
**both** `from` and `to` addresses, not only the sender. Add a test that mints
tokens to an approved address and then attempts to transfer to an unapproved
address — the transfer must fail with `KycNotApproved`.

---

### Finding: Zero-amount transfer bypasses compliance

**Severity:** Medium  
**Remediation:** Add an early-return guard at the top of the transfer function:
```rust
if amount <= 0 {
    panic_with_error!(env, TokenError::NegativeAmount);
}
```
Alternatively, ensure `can_transfer` rejects zero-amount calls explicitly.

---

### Finding: Placeholder metadata on mainnet

**Severity:** High  
**Remediation:** Before deploying to mainnet, replace every `"PLACEHOLDER"` and
empty-string field in the `--meta` JSON with real production values. Add a
pre-deployment check that scans the deployment command for the string
`"PLACEHOLDER"` and fails if found. See the warning in
[docs/mainnet-deployment.md](mainnet-deployment.md).

---

### Finding: Admin key is a hot key

**Severity:** Critical  
**Remediation:** Generate a new admin keypair on a hardware wallet (Ledger) or
HSM. Follow the admin key rotation procedure in
[docs/incident-response.md](incident-response.md) to transfer admin rights from
the current hot key to the new hardware-backed key before any production traffic.
Delete or quarantine the hot key immediately after rotation.

---

### Finding: `frontend/.env` committed to version control

**Severity:** High  
**Remediation:**
1. Remove `.env` from version control: `git rm --cached frontend/.env`
2. Rotate all secrets that were exposed (contract IDs are not secret, but any
   private keys that were in the file must be considered compromised).
3. Confirm `frontend/.env` is listed in `.gitignore`.

---

### Finding: Hardcoded RPC URL or contract ID in frontend source

**Severity:** Medium  
**Remediation:** Move all network-specific values to `VITE_*` environment
variables in `frontend/.env`. The source code should reference only
`import.meta.env.VITE_*` — never a hardcoded `C...` contract ID or
`https://soroban...` URL.
