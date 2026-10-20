# Contract API Reference

This document is the canonical reference for every public entry point, data structure, and error code in the Veritoken contract suite. Use it alongside the source code in `contracts/` when deploying, integrating, or extending the kit.

> **Navigation**
> - [KYC Registry](#kyc-registry)
> - [Compliance Engine](#compliance-engine)
> - [RWA Token](#rwa-token)
> - [Invoice Token](#invoice-token)
> - [Property Token](#property-token)
> - [Carbon Credit Token](#carbon-credit-token)
> - [Common Workflows](#common-workflows)

---

## KYC Registry

**Crate:** `contracts/kyc-registry`  
**Purpose:** On-chain registry of investor KYC approvals. Every token transfer calls `is_approved` on both parties before any balance changes.

### Roles and authorization

Admin membership and verifier membership are independent. An admin must also be added as a verifier to make ordinary subject-level decisions. A verifier cannot manage admin or verifier membership unless separately granted the admin role.

| Capability | Existing methods | Caller requirement and authorization | Subject signature required? |
|---|---|---|---|
| Manage admins | `propose_admin`, `add_admin`, `remove_admin` | `caller.require_auth()` and `require_admin` membership check | No |
| Accept proposed admin membership | `accept_admin` | Stored pending admin authorizes via `pending.require_auth()` | No |
| Manage verifiers | `add_verifier`, `remove_verifier` | `caller.require_auth()` and `require_admin` | No |
| Approve, renew (including clearing expiry), reject, revoke, or update a subject's tier | `approve`, `approve_batch`, `reject`, `revoke`, `revoke_batch`, `update_tier` | `verifier.require_auth()` and `require_verifier` membership check | No |
| Revoke approvals associated with a verifier | `revoke_all_by_verifier` | `caller.require_auth()` and `require_admin` | No |
| Read a full subject record | `get_full_record(requester, subject)` | `requester.require_auth()`; requester must be the subject or an admin | Yes when requester is the subject; no for an admin requester |

### Data structures

#### `KycRecord`
```rust
pub struct KycRecord {
    pub status:       KycStatus,  // Pending | Approved | Rejected | Revoked
    pub verifier:     Address,    // verifier who last acted on this record
    pub tier:         u32,        // 0=Basic, 1=Accredited, 2=Institutional
    pub expiry:       u64,        // Unix timestamp; 0 = no expiry
    pub jurisdiction: String,     // ISO-3166-1 alpha-2 (e.g. "US", "GB")
}
```

#### `KycStatus`
| Value | Meaning |
|---|---|
| `Pending` | Record exists but not yet approved |
| `Approved` | Active approval; transfers allowed |
| `Rejected` | Rejected by a verifier |
| `Revoked` | Previously approved, now revoked |

#### `KycTransition`
Immutable audit record written on every status change.
```rust
pub struct KycTransition {
    pub seq:           u32,               // 0-based, scoped to subject
    pub model_version: u32,               // always 1
    pub kind:          KycTransitionKind, // Approve|Reject|Revoke|TierUpdate
    pub verifier:      Address,
    pub timestamp:     u64,
    pub tier:          u32,
    pub expiry:        u64,
    pub jurisdiction:  String,
}
```

#### `KycFullRecord`
Full data export for a subject (GDPR / CCPA subject-access requests).
```rust
pub struct KycFullRecord {
    pub record:      KycRecord,
    pub log_entries: Vec<VerifierLogEntry>,
    pub registry:    Address,
}
```

### Errors
| Code | Name | When |
|---|---|---|
| 1 | `AlreadyInitialized` | `initialize` called after first deploy |
| 2 | `NotVerifier` | Caller is not in the verifier list |
| 3 | `NotApproved` | Subject lacks an active approval |
| 4 | `NoRecord` | No KYC record exists for subject |
| 5 | `InvalidJurisdiction` | Jurisdiction string is not a valid ISO-3166-1 alpha-2 code |
| 6 | `NotAdmin` | Caller is not in the admin list |
| 7 | `EmptyAdminList` | Removing last admin would leave list empty |
| 8 | `NotAuthorized` | Caller is neither the subject nor an admin |

### Methods

#### Initialization
```
initialize(env, admin: Address)
```
Sets the first admin. Panics with `AlreadyInitialized` if called again.

---

#### Admin management
```
propose_admin(env, caller: Address, new_admin: Address)
accept_admin(env)
add_admin(env, caller: Address, new_admin: Address)
remove_admin(env, caller: Address, admin_to_remove: Address)
get_admins(env) → Vec<Address>
```
Two-step handover: `propose_admin` then `accept_admin` (called by the proposed address). `add_admin` is an immediate single-step addition. `remove_admin` panics if it would empty the list.

---

#### Verifier management
```
add_verifier(env, caller: Address, verifier: Address)
remove_verifier(env, caller: Address, verifier: Address)
verifier_count(env) → u32
verifier_list_pub(env) → Vec<Address>
get_verifiers(env, start: u32, limit: u32) → Vec<Address>   // limit capped at 20
```
Only `add_verifier` and `remove_verifier` are admin-only; the verifier listing and count queries require no authorization. Ordinary KYC write methods require verifier membership.

---

#### KYC operations
```
approve(env, verifier: Address, subject: Address, tier: u32, expiry: u64, jurisdiction: String)
approve_batch(env, verifier: Address, subjects: Vec<(Address, u32, u64, String)>)  // max 20
reject(env, verifier: Address, subject: Address)
revoke(env, verifier: Address, subject: Address)
revoke_batch(env, verifier: Address, subjects: Vec<Address>)  // max 20
update_tier(env, verifier: Address, subject: Address, new_tier: u32)
revoke_all_by_verifier(env, caller: Address, verifier: Address)  // admin-only, cap 50/call
```
`approve` / `approve_batch` require the subject to have an ISO-3166-1 alpha-2 jurisdiction. `update_tier` only works on currently Approved subjects.

---

#### Queries
```
is_approved(env, addr: Address) → bool
get_record(env, addr: Address) → KycRecord
get_tier(env, addr: Address) → u32
get_subjects_by_verifier(env, verifier: Address, start: u32, limit: u32) → Vec<Address>
get_lifecycle_history(env, subject: Address, start: u32, limit: u32) → Vec<KycTransition>
lifecycle_count(env, subject: Address) → u32
get_expiring_soon(env, within_seconds: u64, start: u32, limit: u32) → Vec<ExpiringRecord>
get_verifier_log(env, start: u32, limit: u32) → Vec<VerifierLogEntry>
get_full_record(env, requester: Address, subject: Address) → KycFullRecord
```

---

## Compliance Engine

**Crate:** `contracts/compliance-engine`  
**Purpose:** Configurable transfer rule enforcer. Every token transfer calls `can_transfer` before any balance changes.

### Data structures

#### `ComplianceRules`
```rust
pub struct ComplianceRules {
    pub max_transfer_amount:    i128,  // 0 = unlimited (in stroops)
    pub min_holding_period:     u64,   // seconds; 0 = none
    pub max_holders:            u32,   // 0 = unlimited
    pub require_same_jurisdiction: bool,
    pub paused:                 bool,
    pub allowlist_mode:         bool,  // true = only allowlisted addresses may transfer
    pub max_holding_period:     u64,   // seconds; 0 = unlimited (forced-exit window)
}
```

#### `TierPolicy`
Per-tier-pair transfer policy. Set `from_tier` or `to_tier` to `u32::MAX` (0xFFFFFFFF) as a wildcard.
```rust
pub struct TierPolicy {
    pub blocked:             bool,
    pub max_transfer_amount: i128,  // 0 = inherit global
    pub min_from_tier:       u32,
    pub min_to_tier:         u32,
}
```

#### `TierPolicyKey`
```rust
pub struct TierPolicyKey {
    pub from_tier: u32,
    pub to_tier:   u32,
}
```

#### `RiskConfig`
```rust
pub struct RiskConfig {
    pub max_score:     u32,  // 0 = risk scoring disabled; max 100
    pub default_score: u32,  // score for unknown jurisdictions; range 0–100
}
```

### Errors
| Code | Name | When |
|---|---|---|
| 1 | `AlreadyInitialized` | `initialize` called twice |
| 2 | `MinHoldingPeriodExceeds365Days` | `min_holding_period` > 31,536,000 s |
| 3 | `NegativeMaxTransferAmount` | `max_transfer_amount` < 0 |
| 4 | `MaxHoldersBelowCurrentCount` | New `max_holders` < current holder count |
| 5 | `NoRulesPending` | `activate_rules` called with no proposal |
| 6 | `TooEarlyToActivate` | Time-lock delay not yet elapsed |
| 7 | `InvalidRiskScore` | Score outside `[0, 100]` |
| 8 | `InvalidRiskConfig` | `max_score` outside `[0, 100]` |

### Methods

#### Initialization
```
initialize(env, admin: Address, kyc_registry: Address, rule_change_delay: u64)
```
`rule_change_delay` is seconds between `propose_rules` and `activate_rules`. Use `0` to disable the time-lock.

---

#### Admin management
```
propose_admin(env, new_admin: Address)
accept_admin(env)
```

---

#### Rule management
```
propose_rules(env, new_rules: ComplianceRules)
activate_rules(env)                         // callable by anyone after the delay
set_rules(env, rules: ComplianceRules)      // immediate override; admin-only; emits warning event
get_rules(env) → ComplianceRules
```
Normal flow: `propose_rules` → wait `rule_change_delay` seconds → `activate_rules`. `set_rules` bypasses the time-lock for emergencies.

---

#### Pause / unpause
```
pause(env)    // sets rules.paused = true; admin-only
unpause(env)  // sets rules.paused = false; admin-only
```

---

#### Blocklist
```
add_to_blocklist(env, addr: Address)
remove_from_blocklist(env, addr: Address)
is_blocklisted(env, addr: Address) → bool
get_blocklist(env, start: u32, limit: u32) → Vec<Address>
blocklist_count(env) → u32
```

---

#### Allowlist
```
add_to_allowlist(env, addr: Address)
remove_from_allowlist(env, addr: Address)
is_allowlisted(env, addr: Address) → bool
```
Only used when `rules.allowlist_mode = true`.

---

#### Jurisdiction controls
```
add_blocked_jurisdiction(env, jurisdiction: String)
remove_blocked_jurisdiction(env, jurisdiction: String)
get_blocked_jurisdictions(env) → Vec<String>
```

---

#### Tier-based policy
```
set_tier_policy(env, key: TierPolicyKey, policy: TierPolicy)
get_tier_policy(env, key: TierPolicyKey) → Option<TierPolicy>
remove_tier_policy(env, key: TierPolicyKey)
```
Use `u32::MAX` as a wildcard tier. Exact matches take precedence over wildcards.

---

#### Jurisdiction risk scoring
```
set_jurisdiction_risk(env, jurisdiction: String, score: u32)  // score 0–100
get_jurisdiction_risk(env, jurisdiction: String) → u32
set_risk_config(env, config: RiskConfig)
get_risk_config(env) → RiskConfig
```
Set `max_score = 0` to disable risk scoring entirely.

---

#### Transfer validation
```
can_transfer(env, from: Address, to: Address, amount: i128) → bool
evaluate_transfer(env, from: Address, to: Address, amount: i128) → TransferDecision
```
`can_transfer` is backward-compatible and skips KYC checks. Asset tokens call `evaluate_transfer` so that KYC-related denials are mapped to a distinct error code. Both functions delegate to the same internal evaluator.

##### Denial reason precedence

`evaluate_transfer` (and therefore every delegated asset-token transfer) returns **the first failing check** in the order listed below. When multiple rules are violated simultaneously only the first-reached reason is returned. This ordering is a **guaranteed implementation contract** — SDK callers and UI consumers may depend on it:

1. `CompliancePaused` — engine-wide pause is active
2. `FromBlocklisted` — sender is on the blocklist
3. `ToBlocklisted` — recipient is on the blocklist
4. `FromNotAllowlisted` / `ToNotAllowlisted` — allowlist mode active and address absent (from checked before to)
5. KYC checks on `from` (`Missing → Expired → Revoked → Rejected → Pending`), then the same sequence on `to`
6. Jurisdiction checks (`FromJurisdictionBlocked`, `ToJurisdictionBlocked`, `SameJurisdictionRequired`)
7. Tier policy (`TierPolicyBlocked`, `TierFromBelowMin`, `TierToBelowMin`, `TierAmountExceeded`)
8. Risk scoring (`RiskScoreTooHigh`)
9. Amount cap (`AmountExceeded`)
10. Sender holding period (`HoldingPeriodNotMet`)
11. Recipient holding period (`RecipientHoldingPeriodExceeded`)
12. Holder cap (`MaxHoldersReached`)

> **Note:** This table reflects the current source order in `evaluate_transfer_inner`. If future rule additions change this ordering a version bump and a doc update will accompany the change.

---

#### Holder tracking
```
holder_count(env) → u32
register_holder(env, addr: Address)   // called by asset tokens on first receive
```

---

## RWA Token

**Crate:** `contracts/rwa-token`  
**Purpose:** Base SEP-41 token extended with RWA compliance hooks. Fork this contract for any new asset type.

### Data structures

#### `ComplianceMetadata`
```rust
pub struct ComplianceMetadata {
    pub legal_entity:    Option<String>,
    pub governing_law:   Option<String>,
    pub isin:            Option<String>,
    pub prospectus_hash: Option<String>,
}
```

#### `RecipientEntry`
Used by batch transfer methods.
```rust
pub struct RecipientEntry {
    pub to:     Address,
    pub amount: i128,
}
```

#### `RecoveryConfig`
```rust
pub struct RecoveryConfig {
    pub members:   Vec<Address>,
    pub threshold: u32,  // number of approvals required; 1 ≤ threshold ≤ len(members)
}
```

#### `RecoveryProposal`
```rust
pub struct RecoveryProposal {
    pub proposed_admin: Address,
    pub approvals:      u32,
    pub approved_by:    Vec<Address>,
}
```

### Errors
| Code | Name | When |
|---|---|---|
| 1 | `AlreadyInitialized` | Constructor called twice |
| 2 | `KycNotApproved` | Sender or receiver lacks active KYC |
| 3 | `TransferBlocked` | Compliance engine returned false |
| 4 | `InsufficientBalance` | Sender balance too low |
| 5 | `AllowanceExpired` | SEP-41 allowance is past its expiration ledger |
| 6 | `InsufficientAllowance` | Allowance too small for requested amount |
| 7 | `AccountFrozen` | Account is individually frozen |
| 8 | `NegativeAmount` | Transfer amount ≤ 0 |
| 9 | `BatchTooLarge` | Batch recipient list exceeds 10 entries |
| 10 | `RecoveryNotConfigured` | `initiate_recovery` called before `set_recovery_config` |
| 11 | `NotRecoveryMember` | Caller is not in the recovery members list |
| 12 | `RecoveryAlreadyActive` | A recovery proposal is in progress |
| 13 | `AlreadyApproved` | Caller already approved the active proposal |
| 14 | `NoActiveRecovery` | `approve_recovery` called with no active proposal |
| 15 | `InvalidRecoveryConfig` | threshold < 1 or threshold > len(members) |
| 16 | `ExceedsMaxSupply` | Mint would exceed the configured max supply cap |

### Methods

#### SEP-41 standard interface
```
name(env) → String
symbol(env) → String
decimals(env) → u32
total_supply(env) → i128
balance(env, id: Address) → i128
allowance(env, from: Address, spender: Address) → i128
approve(env, from: Address, spender: Address, amount: i128, expiration_ledger: u32)
transfer(env, from: Address, to: Address, amount: i128)
transfer_from(env, spender: Address, from: Address, to: Address, amount: i128)
burn(env, from: Address, amount: i128)
burn_from(env, spender: Address, from: Address, amount: i128)
```
Every `transfer` and `transfer_from` call checks KYC and compliance before any balance change. A reentrancy guard prevents nested transfers.

---

#### Batch transfers
```
batch_transfer(env, from: Address, recipients: Vec<RecipientEntry>)       // max 10
batch_transfer_from(env, spender: Address, from: Address, recipients: Vec<RecipientEntry>)
```

---

#### Minting
```
mint(env, to: Address, amount: i128)        // admin-only
set_max_supply(env, max_supply: i128)       // admin-only; 0 = no cap
```

---

#### Compliance metadata
```
set_compliance_metadata(env, key: String, value: String)    // admin-only
get_compliance_metadata(env, key: String) → String
get_all_compliance_metadata(env) → ComplianceMetadata
```
Valid keys: `"legal_ent"`, `"gov_law"`, `"isin"`, `"pros_hash"`.

---

#### Compliance contract references
```
kyc_registry(env) → String
compliance_engine(env) → String
asset_type(env) → String
update_kyc_registry(env, new_registry: Address)       // admin-only
update_compliance_engine(env, new_engine: Address)    // admin-only
```

---

#### Account freeze
```
freeze_account(env, target: Address)    // admin-only
unfreeze_account(env, target: Address)  // admin-only
is_frozen(env, addr: Address) → bool
```
Frozen accounts cannot send or receive tokens.

---

#### Admin management
```
propose_admin(env, new_admin: Address)
accept_admin(env)
```

---

#### Admin recovery (multisig)
```
set_recovery_config(env, config: RecoveryConfig)
get_recovery_config(env) → RecoveryConfig
initiate_recovery(env, caller: Address, proposed_admin: Address)
approve_recovery(env, caller: Address)
get_recovery_proposal(env) → Option<RecoveryProposal>
```
Multisig admin recovery: any recovery member calls `initiate_recovery`, then members call `approve_recovery` until the threshold is reached. The proposed admin becomes the new admin automatically.

---

#### Versioning and migration
```
contract_version(env) → String
get_migration_history(env) → Vec<MigrationRecord>
record_migration(env, from_version: String, to_version: String, description: String)  // admin-only
```

---

#### Token export
```
get_token_export(env) → TokenExportMetadata
```
Returns a snapshot of all on-chain token metadata in a single call. Used by explorers and dashboards.

---

## Invoice Token

**Crate:** `contracts/invoice-token`  
**Purpose:** Tokenizes accounts-receivable invoices. Supports a full lifecycle state machine and multiple invoices per contract.

### Lifecycle state machine
```
Created ──issue()──► Issued ──partial_settle()──► PartiallySettled ──settle()──► FullySettled
                        │                                                              │
                        └──────────────────settle()────────────────────────────────────┘
                                                                          (supply → 0) ► Redeemed
```

### Terminology: settlement, redemption, and burn

These three operations are distinct and are **not** synonyms. Settlement records payment against the invoice; redemption is the holder-side exit that burns tokens once settlement has occurred; `burn` is the generic SEP-41 supply-reduction primitive.

| Term | Method(s) | Caller | State transition | Effect on supply / outstanding amount |
|---|---|---|---|---|
| Partial settlement | `partial_settle(invoice_id, settlement_amount)` | Admin | `Issued` → `PartiallySettled` (or stays `PartiallySettled`) | No change to token supply. Increases the recorded `settlement_amount`; the outstanding (unsettled) amount decreases by `settlement_amount`. |
| Full settlement | `settle(invoice_id)` | Admin | `Issued` or `PartiallySettled` → `FullySettled` | No change to token supply. Records the remaining amount as settled; outstanding amount becomes zero and redemption becomes available. |
| Redemption | `redeem(invoice_id, from, amount)` | Token holder (`from.require_auth()`) | `FullySettled` → `Redeemed` once total supply reaches zero; otherwise stays `FullySettled` | Burns the redeemed tokens, reducing total supply. When supply reaches zero the invoice moves to `Redeemed`. Requires prior settlement (`NotSettled` otherwise). |
| Burn (SEP-41) | `burn(invoice_id, from, amount)`, `burn_from(spender, invoice_id, from, amount)` | Token holder (`from`) or approved spender (`spender`) | No lifecycle transition; status is unchanged | Reduces total supply by `amount`. Does not record settlement and does not by itself move the invoice to `Redeemed`. |

Notes:
- Settlement methods (`settle`, `partial_settle`) are admin-only and are blocked while the lifecycle pause is active (`LifecyclePaused`).
- `redeem` is the only path that advances an invoice to `Redeemed`, and only when the resulting supply is zero.
- `burn` / `burn_from` are the standard SEP-41 supply-reduction methods; they are not a substitute for `redeem` when the goal is to close out a settled invoice.

#### Example lifecycle (plain amounts)

Assume an invoice with `face_value_usd = 1000` and 1000 tokens issued (1 token per unit of face value):

1. `create_invoice(meta)` — invoice is `Created`; supply is 0.
2. `issue(invoice_id, to, 1000)` — invoice is `Issued`; supply is 1000; outstanding amount is 1000.
3. `partial_settle(invoice_id, 400)` — invoice is `PartiallySettled`; supply is still 1000; outstanding amount is 600.
4. `settle(invoice_id)` — invoice is `FullySettled`; supply is still 1000; outstanding amount is 0; redemption is now open.
5. `redeem(invoice_id, holder, 1000)` — holder's tokens are burned; supply becomes 0; invoice becomes `Redeemed`.

If instead a holder calls `burn(invoice_id, holder, 1000)` at step 5, supply also becomes 0, but the call is a plain SEP-41 burn and does not itself perform the redemption accounting path.

### Data structures

#### `InvoiceMeta`
```rust
pub struct InvoiceMeta {
    pub invoice_id:            String,
    pub issuer:                String,
    pub debtor:                String,
    pub face_value_usd:        i128,    // stroops (7 decimals)
    pub discount_rate_bps:     u32,     // basis points
    pub due_date:              u64,     // Unix timestamp
    pub currency:              String,  // ISO 4217 (e.g. "USD")
    pub ipfs_doc_hash:         String,  // CIDv0 or CIDv1
    pub transfer_fee_bps:      u32,
    pub fee_recipient:         Option<Address>,
    pub notification_webhook:  String,  // must start with "https://" if non-empty
}
```

#### `InvoiceStatus`
| Value | Code | Meaning |
|---|---|---|
| `Created` | 0 | Registered; no tokens minted |
| `Issued` | 1 | Tokens minted; not yet settled |
| `PartiallySettled` | 2 | Partial payment recorded |
| `FullySettled` | 3 | Full payment recorded; redemption open |
| `Redeemed` | 4 | All tokens burned; supply is zero |

#### `JournalEntry`
```rust
pub struct JournalEntry {
    pub from_status: InvoiceStatus,
    pub to_status:   InvoiceStatus,
    pub ledger:      u32,
    pub timestamp:   u64,
}
```

### Errors
| Code | Name | When |
|---|---|---|
| 1 | `AlreadyInitialized` | `initialize` called after deploy |
| 2 | `AlreadySettled` | Attempting to re-settle |
| 3 | `NotSettled` | Redeeming before settlement |
| 4 | `InsufficientBalance` | Holder balance too low |
| 5 | `NegativeAmount` | Amount ≤ 0 |
| 8 | `KycNotApproved` | Recipient lacks active KYC |
| 9 | `CompliancePaused` | Global pause is active |
| 12 | `PastDueDate` | Invoice due date already passed |
| 13 | `InvoiceNotFound` | No invoice with given ID |
| 14 | `InvoiceAlreadyExists` | Duplicate invoice_id on create |
| 16 | `InvalidLifecycleTransition` | Transition not permitted from current state |
| 17 | `OverSettlement` | Amount exceeds face value |
| 18 | `UnderSettlement` | Settlement amount is zero or negative |
| 19 | `LifecyclePaused` | Lifecycle pause is active |
| 20 | `InvalidMetadata` | Malformed ISIN, currency, or other field |

### Methods

#### Invoice management
```
create_invoice(env, meta: InvoiceMeta)               // admin-only
list_invoices(env, start: u32, limit: u32) → Vec<String>  // limit capped at 50
get_meta(env, invoice_id: String) → InvoiceMeta
update_meta(env, invoice_id: String, new_meta: InvoiceMeta)  // admin-only; only in Created/Issued
```

---

#### Lifecycle
```
issue(env, invoice_id: String, to: Address, amount: i128)
    // admin-only; mints tokens; KYC-gated; capped at face_value_usd
settle(env, invoice_id: String)
    // admin-only; records full settlement; opens redemption
partial_settle(env, invoice_id: String, settlement_amount: i128)
    // admin-only; records partial payment
redeem(env, invoice_id: String, from: Address, amount: i128)
    // holder; burns tokens proportional to settlement ratio
```

---

#### Lifecycle pause
```
pause_lifecycle(env)    // admin-only; blocks settle() and redeem()
unpause_lifecycle(env)  // admin-only
lifecycle_paused(env) → bool
```
Ordinary transfers in `Issued` state are unaffected by the lifecycle pause.

---

#### Queries
```
invoice_status(env, invoice_id: String) → InvoiceStatus
settlement_amount(env, invoice_id: String) → i128
get_journal(env, invoice_id: String) → Vec<JournalEntry>
is_settled(env) → bool          // convenience; checks first invoice
total_supply(env) → i128
balance(env, id: Address) → i128
```

---

#### SEP-41 methods
```
name(env) → String       // "Veritoken Invoice"
symbol(env) → String     // "VTINV"
decimals(env) → u32      // 7
transfer(env, from, to, amount, invoice_id)
transfer_from(env, spender, from, to, amount, invoice_id)
approve(env, from, spender, amount, expiration_ledger, invoice_id)
burn(env, invoice_id, from, amount)
burn_from(env, spender, invoice_id, from, amount)
```

---

## Property Token

**Crate:** `contracts/property-token`  
**Purpose:** Fractional real estate ownership. Each token is one share. Dividends accumulate in a pool and are distributed pro-rata with O(1) gas per holder using a dividend-per-share accumulator.

### Data structures

#### `PropertyMeta`
```rust
pub struct PropertyMeta {
    pub property_id:         String,
    pub legal_name:          String,
    pub jurisdiction:        String,
    pub address:             String,
    pub total_valuation_usd: i128,
    pub total_shares:        i128,
    pub property_type:       String,  // "residential" | "commercial" | "land"
    pub ipfs_title_hash:     String,
    pub kyc_tier_required:   u32,     // minimum KYC tier for shareholders
}
```

#### `DividendEvent`
```rust
pub struct DividendEvent {
    pub amount:             i128,
    pub timestamp:          u64,
    pub running_total_dps:  i128,  // cumulative dividend-per-share at this point
    pub distribution_type:  u32,   // 0=Rent, 1=Capital, 2=Other
}
```

#### `DistributionType`
| Value | Name |
|---|---|
| 0 | `Rent` |
| 1 | `Capital` |
| 2 | `Other` |

#### `ForcedTransferEntry`
Audit log record for admin-initiated forced transfers.
```rust
pub struct ForcedTransferEntry {
    pub from:      Address,
    pub to:        Address,
    pub shares:    i128,
    pub timestamp: u64,
}
```

### Errors
| Code | Name | When |
|---|---|---|
| 1 | `AlreadyInitialized` | Constructor called twice |
| 2 | `NegativeShares` | Share amount ≤ 0 |
| 3 | `InsufficientShares` | Sender share balance too low |
| 4 | `NoShares` | Attempting to claim dividend with zero shares |
| 5 | `KycNotApproved` | Recipient lacks active KYC |
| 6 | `KycTierTooLow` | Recipient KYC tier below `kyc_tier_required` |
| 7 | `CompliancePaused` | Global pause is active |
| 8 | `Blocklisted` | Sender or receiver is on the blocklist |
| 9 | `TransferBlocked` | Compliance engine returned false |
| 10 | `InvalidMetadata` | Malformed legal_name, jurisdiction, or ipfs_title_hash |

### Methods

#### Metadata
```
get_meta(env) → PropertyMeta
update_meta(env, new_meta: PropertyMeta)   // admin-only; property_id is immutable
```

---

#### Share management
```
mint(env, to: Address, shares: i128)                        // admin-only; KYC-gated
transfer(env, from: Address, to: Address, shares: i128)     // KYC + compliance gated
forced_transfer(env, from: Address, to: Address, shares: i128)  // admin-only; bypasses compliance
total_shares(env) → i128
balance(env, id: Address) → i128
minted_shares(env) → i128
```

---

#### Dividends
```
deposit_dividend(env, from: Address, amount: i128, distribution_type: u32)
    // admin-only; distributes pro-rata to all current holders
claim_dividend(env, holder: Address) → i128
    // holder claims their accumulated dividend; returns amount claimed
pending_dividend(env, holder: Address) → i128
    // read-only; returns unclaimed dividend for a holder
dividend_deposit_count(env) → u32
get_dividend_history(env, start: u32, limit: u32) → Vec<DividendEvent>
get_dividend_summary(env, holder: Address) → DividendSummary
```

---

#### Holder queries
```
holder_count(env) → u32
holder_slots_remaining(env) → u32   // available slots before max_holders is reached
get_forced_transfer_log(env, start: u32, limit: u32) → Vec<ForcedTransferEntry>
forced_transfer_count(env) → u32
```

---

#### SEP-41 allowance
```
approve(env, from: Address, spender: Address, amount: i128, expiration_ledger: u32)
allowance(env, from: Address, spender: Address) → i128
transfer_from(env, spender: Address, from: Address, to: Address, shares: i128)
```

---

#### Admin
```
propose_admin(env, new_admin: Address)
accept_admin(env)
update_kyc_registry(env, new_registry: Address)
update_compliance_engine(env, new_engine: Address)
```

---

## Carbon Credit Token

**Crate:** `contracts/carbon-credit-token`  
**Purpose:** Issues verified carbon credits (1 token = 1 tonne CO₂e). Retirement permanently burns tokens and writes an immutable on-chain receipt.

### Data structures

#### `ProjectMeta`
```rust
pub struct ProjectMeta {
    pub project_id:          String,
    pub standard:            String,  // "VCS" | "Gold Standard" | "CDM" | "ACR"
    pub vintage_year:        u32,     // 1990–2050
    pub project_name:        String,
    pub project_type:        String,  // "forestry" | "renewable" | "methane_capture"
    pub country:             String,
    pub verifier:            String,
    pub ipfs_cert_hash:      String,
    pub registry_url:        String,
    pub registry_project_id: String,
}
```

#### `RetirementReceipt`
```rust
pub struct RetirementReceipt {
    pub retiree:              Address,
    pub amount:               i128,
    pub timestamp:            u64,
    pub beneficiary:          String,
    pub retirement_reason:    String,
    pub beneficiary_address:  Option<Address>,
}
```

#### `ReceiptVerification`
```rust
pub struct ReceiptVerification {
    pub index:      u32,
    pub valid:      bool,
    pub retiree:    Address,
    pub amount:     i128,
    pub timestamp:  u64,
    pub project_id: String,
    pub serial:     String,  // "<project_id>-<index>"
}
```

### Errors
| Code | Name | When |
|---|---|---|
| 1 | `AlreadyInitialized` | Constructor called twice |
| 2 | `InsufficientBalance` | Holder balance too low to retire |
| 3 | `KycNotApproved` | Sender lacks active KYC |
| 4 | `CompliancePaused` | Global pause is active |
| 5 | `Blocklisted` | Sender is on the blocklist |
| 6 | `TransferBlocked` | Compliance engine returned false |
| 7 | `BatchTooLarge` | Batch retirement list exceeds 10 entries |
| 8 | `InvalidAmount` | Individual retirement amount ≤ 0 |

### Methods

#### Metadata
```
get_meta(env) → ProjectMeta
update_meta(env, new_meta: ProjectMeta)   // admin-only; project_id is immutable
get_registry_link(env) → (String, String) // (registry_url, registry_project_id)
```

---

#### SEP-41 standard interface
```
name(env) → String        // "Veritoken Carbon Credit"
symbol(env) → String      // "VTCC"
decimals(env) → u32       // 0 (whole tonnes only)
total_supply(env) → i128
balance(env, id: Address) → i128
transfer(env, from: Address, to: Address, amount: i128)
transfer_from(env, spender: Address, from: Address, to: Address, amount: i128)
approve(env, from: Address, spender: Address, amount: i128, expiration_ledger: u32)
allowance(env, from: Address, spender: Address) → i128
burn(env, from: Address, amount: i128)
burn_from(env, spender: Address, from: Address, amount: i128)
```

---

#### Minting
```
mint(env, to: Address, amount: i128)   // admin-only; KYC-gated
```

---

#### Retirement
```
retire(env, retiree: Address, amount: i128, beneficiary: String, reason: String)
    → RetirementReceipt
    // KYC-gated; permanently burns tokens; writes immutable receipt

batch_retire(env, retiree: Address, retirements: Vec<(i128, String, String)>)
    → Vec<RetirementReceipt>
    // max 10 entries per call
```

---

#### Receipts
```
retirement_count(env) → u32
total_retired(env) → i128
get_receipt(env, index: u32) → RetirementReceipt
get_receipts(env, start: u32, limit: u32) → Vec<RetirementReceipt>  // limit capped at 100
verify_receipt(env, index: u32) → ReceiptVerification
```

---

#### Admin
```
propose_admin(env, new_admin: Address)
accept_admin(env)
update_kyc_registry(env, new_registry: Address)
update_compliance_engine(env, new_engine: Address)
```

---

## Common Workflows

### Deploy and initialize a new asset

```bash
# 1. Deploy the contract (constructor runs atomically — no separate initialize call needed)
stellar contract deploy \
  --wasm target/wasm32-unknown-unknown/release/invoice_token.wasm \
  --source <ADMIN_KEY> \
  -- \
  --admin <ADMIN_ADDRESS> \
  --kyc_registry <KYC_REGISTRY_ID> \
  --compliance_engine <COMPLIANCE_ENGINE_ID> \
  --meta '{ "invoice_id": "INV-001", ... }'
```

### Approve a KYC holder
```bash
stellar contract invoke --id <KYC_REGISTRY_ID> \
  --source <VERIFIER_KEY> \
  -- approve \
  --verifier <VERIFIER_ADDRESS> \
  --subject <HOLDER_ADDRESS> \
  --tier 1 \
  --expiry 1893456000 \
  --jurisdiction US
```

### Update compliance rules (with time-lock)
```bash
# Step 1 — propose
stellar contract invoke --id <COMPLIANCE_ENGINE_ID> --source <ADMIN_KEY> \
  -- propose_rules --new_rules '{ "max_transfer_amount": 1000000000, "paused": false, ... }'

# Step 2 — activate (after rule_change_delay seconds have elapsed)
stellar contract invoke --id <COMPLIANCE_ENGINE_ID> --source <ANY_KEY> \
  -- activate_rules
```

### Emergency pause
```bash
stellar contract invoke --id <COMPLIANCE_ENGINE_ID> --source <ADMIN_KEY> -- pause
```

### Retire carbon credits
```bash
stellar contract invoke --id <CARBON_TOKEN_ID> --source <HOLDER_KEY> \
  -- retire \
  --retiree <HOLDER_ADDRESS> \
  --amount 100 \
  --beneficiary "Acme Corp 2024 offset" \
  --reason "Annual Scope 1 compliance"
```

---

*This reference is generated from the contract source at `contracts/`. If you find a discrepancy between this document and the source, the source is authoritative. See [storage-patterns.md](storage-patterns.md) for storage tier and TTL details.*
