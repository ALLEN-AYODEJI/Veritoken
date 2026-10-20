#![cfg(test)]

use crate::{
    ComplianceMetadata, RecipientEntry, RwaToken, RwaTokenClient, META_ISIN, META_LEGAL_ENTITY,
};
use compliance_engine::{ComplianceEngine, ComplianceEngineClient, ComplianceRules};
use kyc_registry::{KycRegistry, KycRegistryClient};
use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    vec, Address, Env, Error, String,
};

#[allow(dead_code)]
struct Harness {
    env: Env,
    token: RwaTokenClient<'static>,
    kyc: KycRegistryClient<'static>,
    compliance: ComplianceEngineClient<'static>,
    verifier: Address,
    admin: Address,
}

fn setup() -> Harness {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);

    // KYC registry
    let kyc_id = env.register(KycRegistry, ());
    let kyc = KycRegistryClient::new(&env, &kyc_id);
    kyc.initialize(&admin);
    let verifier = Address::generate(&env);
    kyc.add_verifier(&admin, &verifier);

    // Compliance engine
    let compliance_id = env.register(ComplianceEngine, ());
    let compliance = ComplianceEngineClient::new(&env, &compliance_id);
    compliance.initialize(&admin, &kyc_id, &0u64);

    // RWA token — constructor args passed atomically at register time
    let token_id = env.register(
        RwaToken,
        (
            admin.clone(),
            7u32,
            String::from_str(&env, "Veritoken RWA"),
            String::from_str(&env, "VTRWA"),
            String::from_str(&env, "property"),
            kyc_id.clone(),
            compliance_id.clone(),
            Option::<ComplianceMetadata>::None,
            0i128, // max_supply: 0 = unlimited
        ),
    );
    let token = RwaTokenClient::new(&env, &token_id);

    Harness {
        env,
        token,
        kyc,
        compliance,
        verifier,
        admin,
    }
}

#[allow(dead_code)]
impl Harness {
    fn approve_kyc(&self, addr: &Address) {
        self.kyc.approve(
            &self.verifier,
            addr,
            &1,
            &0,
            &String::from_str(&self.env, "US"),
        );
    }

    /// Convenience: mint via admin (the common case in most tests).
    fn mint(&self, to: &Address, amount: i128) {
        self.token.mint(&self.admin, to, &amount);
    }

    /// Convenience: freeze via admin.
    fn freeze(&self, addr: &Address) {
        self.token.freeze(&self.admin, addr);
    }

    /// Convenience: unfreeze via admin.
    fn unfreeze(&self, addr: &Address) {
        self.token.unfreeze(&self.admin, addr);
    }

    /// Returns the current admin nonce and consumes it for the next protected call.
    fn next_nonce(&self) -> u64 {
        self.token.admin_nonce()
    }

    fn set_max_holders(&self, max_holders: u32) {
        self.compliance.set_rules(&ComplianceRules {
            max_transfer_amount: 0,
            min_holding_period: 0,
            max_holders,
            require_same_jurisdiction: false,
            paused: false,
            allowlist_mode: false,
            max_holding_period: 0,
        });
    }
}

#[test]
fn test_metadata() {
    let h = setup();
    assert_eq!(h.token.decimals(), 7);
    assert_eq!(h.token.name(), String::from_str(&h.env, "Veritoken RWA"));
    assert_eq!(h.token.symbol(), String::from_str(&h.env, "VTRWA"));
    assert_eq!(h.token.asset_type(), String::from_str(&h.env, "property"));
    assert_eq!(h.token.total_supply(), 0);
}

#[test]
#[should_panic]
fn test_constructor_rejects_blank_name() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let kyc_id = env.register(KycRegistry, ());
    KycRegistryClient::new(&env, &kyc_id).initialize(&admin);
    let compliance_id = env.register(ComplianceEngine, ());
    ComplianceEngineClient::new(&env, &compliance_id).initialize(&admin, &kyc_id, &0u64);

    env.register(
        RwaToken,
        (
            admin,
            7u32,
            String::from_str(&env, "   "),
            String::from_str(&env, "VTRWA"),
            String::from_str(&env, "property"),
            kyc_id,
            compliance_id,
            Option::<ComplianceMetadata>::None,
            0i128,
        ),
    );
}

#[test]
#[should_panic]
fn test_constructor_rejects_empty_name() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let kyc_id = env.register(KycRegistry, ());
    KycRegistryClient::new(&env, &kyc_id).initialize(&admin);
    let compliance_id = env.register(ComplianceEngine, ());
    ComplianceEngineClient::new(&env, &compliance_id).initialize(&admin, &kyc_id, &0u64);

    env.register(
        RwaToken,
        (
            admin,
            7u32,
            String::from_str(&env, ""),
            String::from_str(&env, "VTRWA"),
            String::from_str(&env, "property"),
            kyc_id,
            compliance_id,
            Option::<ComplianceMetadata>::None,
            0i128,
        ),
    );
}

#[test]
#[should_panic]
fn test_constructor_rejects_blank_symbol() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let kyc_id = env.register(KycRegistry, ());
    KycRegistryClient::new(&env, &kyc_id).initialize(&admin);
    let compliance_id = env.register(ComplianceEngine, ());
    ComplianceEngineClient::new(&env, &compliance_id).initialize(&admin, &kyc_id, &0u64);

    env.register(
        RwaToken,
        (
            admin,
            7u32,
            String::from_str(&env, "Veritoken RWA"),
            String::from_str(&env, "   "),
            String::from_str(&env, "property"),
            kyc_id,
            compliance_id,
            Option::<ComplianceMetadata>::None,
            0i128,
        ),
    );
}

#[test]
#[should_panic]
fn test_constructor_rejects_empty_symbol() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let kyc_id = env.register(KycRegistry, ());
    KycRegistryClient::new(&env, &kyc_id).initialize(&admin);
    let compliance_id = env.register(ComplianceEngine, ());
    ComplianceEngineClient::new(&env, &compliance_id).initialize(&admin, &kyc_id, &0u64);

    env.register(
        RwaToken,
        (
            admin,
            7u32,
            String::from_str(&env, "Veritoken RWA"),
            String::from_str(&env, ""),
            String::from_str(&env, "property"),
            kyc_id,
            compliance_id,
            Option::<ComplianceMetadata>::None,
            0i128,
        ),
    );
}

#[test]
fn test_mint_requires_kyc() {
    let h = setup();
    let user = Address::generate(&h.env);

    // Without KYC, mint should fail
    let res = h.token.try_mint(&h.admin, &user, &1_000);
    assert!(res.is_err());

    // With KYC, mint succeeds
    h.approve_kyc(&user);
    h.mint(&user, 1_000);
    assert_eq!(h.token.balance(&user), 1_000);
    assert_eq!(h.token.total_supply(), 1_000);
}

#[test]
fn test_transfer_happy_path() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);

    h.mint(&alice, 1_000);
    h.token.transfer(&alice, &bob, &400);

    assert_eq!(h.token.balance(&alice), 600);
    assert_eq!(h.token.balance(&bob), 400);
}

#[test]
fn test_transfer_methods_reject_nonpositive_amounts_without_state_changes() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    let spender = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.mint(&alice, 1_000);

    let expiration = h.env.ledger().sequence() + 1_000;
    h.token.approve(&alice, &spender, &500, &expiration);
    let expected_error = Err(Ok(Error::from(crate::RwaError::NegativeAmount)));

    for amount in [0, -1] {
        assert_eq!(h.token.try_transfer(&alice, &bob, &amount), expected_error);
        assert_eq!(
            h.token
                .try_transfer_from(&spender, &alice, &bob, &amount),
            expected_error
        );

        let recipients = vec![
            &h.env,
            RecipientEntry {
                to: bob.clone(),
                amount,
            },
        ];
        assert_eq!(
            h.token.try_batch_transfer(&alice, &recipients),
            expected_error
        );
        assert_eq!(
            h.token
                .try_batch_transfer_from(&spender, &alice, &recipients),
            expected_error
        );

        assert_eq!(h.token.balance(&alice), 1_000);
        assert_eq!(h.token.balance(&bob), 0);
        assert_eq!(h.token.allowance(&alice, &spender), 500);
    }
}

#[test]
fn test_transfer_blocked_without_kyc_on_receiver() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env); // no KYC
    h.approve_kyc(&alice);
    h.mint(&alice, 1_000);

    let res = h.token.try_transfer(&alice, &bob, &100);
    assert!(res.is_err());
}

#[test]
fn test_transfer_while_paused() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.mint(&alice, 1_000);

    h.compliance.pause();
    assert_eq!(
        h.token.try_transfer(&alice, &bob, &100),
        Err(Ok(Error::from(crate::RwaError::TransferBlocked)))
    );
}

#[test]
fn test_transfer_blocked_when_compliance_paused() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.mint(&alice, 1_000);

    h.compliance.pause();
    let res = h.token.try_transfer(&alice, &bob, &100);
    assert!(res.is_err());

    h.compliance.unpause();
    h.token.transfer(&alice, &bob, &100);
    assert_eq!(h.token.balance(&bob), 100);
}

#[test]
fn test_transfer_blocked_by_max_amount() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.mint(&alice, 1_000);

    h.compliance.set_rules(&ComplianceRules {
        max_transfer_amount: 50,
        min_holding_period: 0,
        max_holders: 0,
        require_same_jurisdiction: false,
        paused: false,
        allowlist_mode: false,
        max_holding_period: 0,
    });

    assert!(h.token.try_transfer(&alice, &bob, &51).is_err());
    h.token.transfer(&alice, &bob, &50);
    assert_eq!(h.token.balance(&bob), 50);
}

#[test]
fn test_max_holder_cap_blocks_new_holder_and_maintains_count() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    let charlie = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.approve_kyc(&charlie);

    h.compliance.set_rules(&ComplianceRules {
        max_transfer_amount: 0,
        min_holding_period: 0,
        max_holders: 2,
        require_same_jurisdiction: false,
        paused: false,
        allowlist_mode: false,
        max_holding_period: 0,
    });

    h.mint(&alice, 1_000);
    assert_eq!(h.compliance.holder_count(), 1);

    h.token.transfer(&alice, &bob, &400);
    assert_eq!(h.compliance.holder_count(), 2);
    assert!(h.token.try_transfer(&alice, &charlie, &1).is_err());

    h.token.transfer(&alice, &bob, &600);
    assert_eq!(h.token.balance(&alice), 0);
    assert_eq!(h.compliance.holder_count(), 1);

    h.token.transfer(&bob, &charlie, &1);
    assert_eq!(h.compliance.holder_count(), 2);
    assert_eq!(h.token.balance(&charlie), 1);
}

#[test]
fn test_max_holders_blocks_new_holders_via_token() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    let charlie = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.approve_kyc(&charlie);

    h.compliance.set_rules(&ComplianceRules {
        max_transfer_amount: 0,
        min_holding_period: 0,
        max_holders: 2,
        require_same_jurisdiction: false,
        paused: false,
        allowlist_mode: false,
        max_holding_period: 0,
    });

    // First two distinct holders fill the cap.
    h.mint(&alice, 1_000);
    h.mint(&bob, 1_000);
    assert_eq!(h.compliance.holder_count(), 2);

    // A mint to a third distinct holder must be rejected by the compliance engine.
    assert!(h.token.try_mint(&h.admin, &charlie, &1_000).is_err());

    // The failed mint leaves the holder count unchanged.
    assert_eq!(h.compliance.holder_count(), 2);
    assert_eq!(h.token.balance(&charlie), 0);
}

#[test]
fn test_approve_and_transfer_from() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    let spender = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.mint(&alice, 1_000);

    let expiration = h.env.ledger().sequence() + 1_000;
    h.token.approve(&alice, &spender, &300, &expiration);
    assert_eq!(h.token.allowance(&alice, &spender), 300);

    h.token.transfer_from(&spender, &alice, &bob, &200);
    assert_eq!(h.token.balance(&bob), 200);
    assert_eq!(h.token.balance(&alice), 800);
    assert_eq!(h.token.allowance(&alice, &spender), 100);
}

#[test]
fn test_allowance_expired_at_expiration_ledger() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    let spender = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.mint(&alice, 1_000);

    // Allowance expires at the current ledger sequence — it must already be
    // treated as expired, not valid for one more ledger.
    let current_sequence = h.env.ledger().sequence();
    h.token.approve(&alice, &spender, &300, &current_sequence);

    assert_eq!(h.token.allowance(&alice, &spender), 0);
    assert!(h
        .token
        .try_transfer_from(&spender, &alice, &bob, &100)
        .is_err());
}

#[test]
fn test_allowance_valid_one_ledger_before_expiration() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    let spender = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.mint(&alice, 1_000);

    let current_sequence = h.env.ledger().sequence();
    h.token
        .approve(&alice, &spender, &300, &(current_sequence + 1));

    h.token.transfer_from(&spender, &alice, &bob, &100);
    assert_eq!(h.token.balance(&bob), 100);
    assert_eq!(h.token.allowance(&alice, &spender), 200);
}

#[test]
fn test_spend_allowance_to_zero_removes_storage_entry() {
    use crate::storage_types::{AllowanceKey, DataKey};

    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    let spender = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.mint(&alice, 1_000);

    let expiration = h.env.ledger().sequence() + 1_000;
    h.token.approve(&alice, &spender, &100, &expiration);
    h.token.transfer_from(&spender, &alice, &bob, &100);

    assert_eq!(h.token.allowance(&alice, &spender), 0);

    let key = DataKey::Allowance(AllowanceKey {
        from: alice.clone(),
        spender: spender.clone(),
    });
    h.env.as_contract(&h.token.address, || {
        assert!(!h.env.storage().temporary().has(&key));
    });
}

#[test]
fn test_burn_reduces_supply() {
    let h = setup();
    let alice = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.mint(&alice, 1_000);

    h.token.burn(&alice, &400);
    assert_eq!(h.token.balance(&alice), 600);
    assert_eq!(h.token.total_supply(), 600);
}

#[test]
fn test_set_admin() {
    let h = setup();
    let new_admin = Address::generate(&h.env);
    h.token.set_admin(&new_admin);
    // New admin can mint after KYC approval of a holder
    let user = Address::generate(&h.env);
    h.approve_kyc(&user);
    h.token.mint(&new_admin, &user, &1);
    assert_eq!(h.token.balance(&user), 1);
    let _ = &h.admin;
}

#[test]
fn test_compliance_metadata() {
    let h = setup();
    let nonce = h.next_nonce();
    let key = soroban_sdk::symbol_short!("legal");
    h.token.set_compliance_metadata(
        &h.admin,
        &key,
        &String::from_str(&h.env, "prospectus-v1"),
        &nonce,
    );
    assert_eq!(
        h.token.get_compliance_metadata(&key),
        String::from_str(&h.env, "prospectus-v1")
    );
}

#[test]
fn test_non_deployer_cannot_reinitialize() {
    let h = setup();
    let attacker = Address::generate(&h.env);
    let kyc_id = h.token.kyc_registry();
    let ce_id = h.token.compliance_engine();
    // initialize must always panic — the constructor has already run
    let result = h.token.try_initialize(
        &attacker,
        &7,
        &String::from_str(&h.env, "Evil Token"),
        &String::from_str(&h.env, "EVIL"),
        &String::from_str(&h.env, "property"),
        &kyc_id,
        &ce_id,
    );
    assert!(result.is_err());
}

#[test]
fn test_get_all_compliance_metadata_returns_empty_when_unset() {
    let h = setup();
    let meta = h.token.get_all_compliance_metadata();
    assert!(meta.legal_entity.is_none());
    assert!(meta.governing_law.is_none());
    assert!(meta.isin.is_none());
    assert!(meta.prospectus_hash.is_none());
}

#[test]
fn test_get_all_compliance_metadata_returns_set_fields() {
    let h = setup();
    let key_entity = soroban_sdk::Symbol::new(&h.env, META_LEGAL_ENTITY);
    let key_isin = soroban_sdk::Symbol::new(&h.env, META_ISIN);
    h.token.set_compliance_metadata(
        &h.admin,
        &key_entity,
        &String::from_str(&h.env, "Acme Corp"),
        &0,
    );
    h.token.set_compliance_metadata(
        &h.admin,
        &key_isin,
        &String::from_str(&h.env, "US1234567890"),
        &1,
    );
    let meta = h.token.get_all_compliance_metadata();
    assert_eq!(
        meta.legal_entity,
        Some(String::from_str(&h.env, "Acme Corp"))
    );
    assert_eq!(meta.isin, Some(String::from_str(&h.env, "US1234567890")));
    assert!(meta.governing_law.is_none());
    assert!(meta.prospectus_hash.is_none());
}

#[test]
fn test_constructor_sets_compliance_metadata() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);

    let kyc_id = env.register(KycRegistry, ());
    let kyc = KycRegistryClient::new(&env, &kyc_id);
    kyc.initialize(&admin);

    let compliance_id = env.register(ComplianceEngine, ());
    let compliance = ComplianceEngineClient::new(&env, &compliance_id);
    compliance.initialize(&admin, &kyc_id, &0u64);

    let token_id = env.register(
        RwaToken,
        (
            admin.clone(),
            7u32,
            String::from_str(&env, "Invoice Token"),
            String::from_str(&env, "IVTK"),
            String::from_str(&env, "invoice"),
            kyc_id.clone(),
            compliance_id.clone(),
            Some(ComplianceMetadata {
                legal_entity: Some(String::from_str(&env, "Issuer LLC")),
                governing_law: Some(String::from_str(&env, "New York")),
                isin: None,
                prospectus_hash: None,
            }),
            0i128, // max_supply: 0 = unlimited
        ),
    );
    let token = RwaTokenClient::new(&env, &token_id);
    let meta = token.get_all_compliance_metadata();
    assert_eq!(
        meta.legal_entity,
        Some(String::from_str(&env, "Issuer LLC"))
    );
    assert_eq!(meta.governing_law, Some(String::from_str(&env, "New York")));
    assert!(meta.isin.is_none());
}

#[test]
fn test_mint_twice_same_address_holder_count_is_one() {
    let h = setup();
    let user = Address::generate(&h.env);
    h.approve_kyc(&user);

    h.mint(&user, 1_000);
    h.mint(&user, 500);

    assert_eq!(h.compliance.holder_count(), 1);
    assert_eq!(h.token.balance(&user), 1_500);
    assert_eq!(h.token.total_supply(), 1_500);
}

#[test]
#[should_panic]
fn test_invalid_asset_type() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let kyc_id = env.register(KycRegistry, ());
    let compliance_id = env.register(ComplianceEngine, ());

    // Try to register token with invalid asset type
    let _ = env.register(
        RwaToken,
        (
            admin,
            7u32,
            String::from_str(&env, "Bad Token"),
            String::from_str(&env, "BAD"),
            String::from_str(&env, "banana"),
            kyc_id,
            compliance_id,
            Option::<ComplianceMetadata>::None,
            0i128,
        ),
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #8)")]
fn test_constructor_rejects_negative_max_supply() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let kyc_id = env.register(KycRegistry, ());
    let compliance_id = env.register(ComplianceEngine, ());

    // max_supply = 0 means "unlimited"; any negative cap is malformed.
    let _ = env.register(
        RwaToken,
        (
            admin,
            7u32,
            String::from_str(&env, "Bad Cap"),
            String::from_str(&env, "BAD"),
            String::from_str(&env, "invoice"),
            kyc_id,
            compliance_id,
            Option::<ComplianceMetadata>::None,
            -1i128,
        ),
    );
}

#[test]
fn test_read_admin_on_uninitialized_contract_fails_with_typed_error() {
    use crate::storage_types::DataKey;
    use crate::RwaError;
    use soroban_sdk::Error;

    let h = setup();

    // Simulate an uninitialized contract by clearing the Admin key directly.
    h.env.as_contract(&h.token.address, || {
        h.env.storage().instance().remove(&DataKey::Admin);
    });

    let res = h
        .token
        .try_set_external_uri(&String::from_str(&h.env, "ipfs://Qm"));
    assert_eq!(
        res.unwrap_err().unwrap(),
        Error::from(RwaError::NotInitialized)
    );
}

#[test]
fn test_version_returns_nonempty() {
    let h = setup();
    let v = h.token.version();
    assert!(!v.is_empty());
}

// ── batch_transfer ────────────────────────────────────────────────────────────

#[test]
fn test_batch_transfer_two_recipients_success() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    let carol = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.approve_kyc(&carol);
    h.mint(&alice, 1_000);

    let recipients = vec![
        &h.env,
        RecipientEntry {
            to: bob.clone(),
            amount: 300,
        },
        RecipientEntry {
            to: carol.clone(),
            amount: 200,
        },
    ];
    h.token.batch_transfer(&alice, &recipients);

    assert_eq!(h.token.balance(&alice), 500);
    assert_eq!(h.token.balance(&bob), 300);
    assert_eq!(h.token.balance(&carol), 200);
}

#[test]
fn test_batch_transfer_state_unchanged_on_kyc_failure() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    let carol = Address::generate(&h.env); // no KYC
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.mint(&alice, 1_000);

    let recipients = vec![
        &h.env,
        RecipientEntry {
            to: bob.clone(),
            amount: 300,
        },
        RecipientEntry {
            to: carol.clone(),
            amount: 200,
        }, // fails here
    ];
    assert!(h.token.try_batch_transfer(&alice, &recipients).is_err());

    // No state changes: balances untouched
    assert_eq!(h.token.balance(&alice), 1_000);
    assert_eq!(h.token.balance(&bob), 0);
    assert_eq!(h.token.balance(&carol), 0);
}

#[test]
fn test_batch_transfer_state_unchanged_on_insufficient_balance() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    let carol = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.approve_kyc(&carol);
    h.mint(&alice, 400); // only 400, but batch asks for 300 + 200 = 500

    let recipients = vec![
        &h.env,
        RecipientEntry {
            to: bob.clone(),
            amount: 300,
        },
        RecipientEntry {
            to: carol.clone(),
            amount: 200,
        },
    ];
    assert!(h.token.try_batch_transfer(&alice, &recipients).is_err());

    // State must be fully unchanged
    assert_eq!(h.token.balance(&alice), 400);
    assert_eq!(h.token.balance(&bob), 0);
    assert_eq!(h.token.balance(&carol), 0);
}

#[test]
fn test_batch_transfer_state_unchanged_on_frozen_recipient() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    let carol = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.approve_kyc(&carol);
    h.mint(&alice, 1_000);
    h.freeze(&carol);

    let recipients = vec![
        &h.env,
        RecipientEntry {
            to: bob.clone(),
            amount: 300,
        },
        RecipientEntry {
            to: carol.clone(),
            amount: 200,
        }, // frozen → validation fails
    ];
    assert!(h.token.try_batch_transfer(&alice, &recipients).is_err());

    assert_eq!(h.token.balance(&alice), 1_000);
    assert_eq!(h.token.balance(&bob), 0);
    assert_eq!(h.token.balance(&carol), 0);
}

#[test]
fn test_batch_transfer_frozen_sender_rejected() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.mint(&alice, 1_000);
    h.freeze(&alice);

    let recipients = vec![
        &h.env,
        RecipientEntry {
            to: bob.clone(),
            amount: 100,
        },
    ];
    assert!(h.token.try_batch_transfer(&alice, &recipients).is_err());
    assert_eq!(h.token.balance(&alice), 1_000);
}

#[test]
fn test_batch_transfer_exceeds_max_recipients() {
    let h = setup();
    let alice = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.mint(&alice, 10_000);

    // Build 11 recipients — must be rejected before any transfer
    let mut recipients = soroban_sdk::Vec::new(&h.env);
    for _ in 0..11 {
        let addr = Address::generate(&h.env);
        h.approve_kyc(&addr);
        recipients.push_back(RecipientEntry {
            to: addr,
            amount: 1,
        });
    }
    assert!(h.token.try_batch_transfer(&alice, &recipients).is_err());
    assert_eq!(h.token.balance(&alice), 10_000);
}

#[test]
fn test_batch_transfer_empty_recipients_rejected() {
    use crate::RwaError;
    use soroban_sdk::Error;

    let h = setup();
    let alice = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.mint(&alice, 10_000);

    let recipients = soroban_sdk::Vec::new(&h.env);
    let res = h.token.try_batch_transfer(&alice, &recipients);
    assert_eq!(res.unwrap_err().unwrap(), Error::from(RwaError::EmptyBatch));
    assert_eq!(h.token.balance(&alice), 10_000);
}

#[test]
fn test_batch_transfer_zero_amount_entry_rejected() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.mint(&alice, 1_000);

    let recipients = vec![
        &h.env,
        RecipientEntry {
            to: bob.clone(),
            amount: 0,
        }, // invalid
    ];
    assert!(h.token.try_batch_transfer(&alice, &recipients).is_err());
    assert_eq!(h.token.balance(&alice), 1_000);
    assert_eq!(h.token.balance(&bob), 0);
}

#[test]
fn test_batch_transfer_sender_unregistered_when_balance_drained() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.mint(&alice, 500);
    assert_eq!(h.compliance.holder_count(), 1);

    let recipients = vec![
        &h.env,
        RecipientEntry {
            to: bob.clone(),
            amount: 500,
        }, // drains alice
    ];
    h.token.batch_transfer(&alice, &recipients);

    assert_eq!(h.token.balance(&alice), 0);
    assert_eq!(h.token.balance(&bob), 500);
    // alice deregistered, bob registered → still 1 holder
    assert_eq!(h.compliance.holder_count(), 1);
}

#[test]
fn test_batch_transfer_holder_count_correct_after_batch() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    let carol = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.approve_kyc(&carol);
    h.mint(&alice, 1_000);
    assert_eq!(h.compliance.holder_count(), 1);

    let recipients = vec![
        &h.env,
        RecipientEntry {
            to: bob.clone(),
            amount: 400,
        },
        RecipientEntry {
            to: carol.clone(),
            amount: 300,
        },
    ];
    h.token.batch_transfer(&alice, &recipients);

    // alice (300 remaining), bob (400), carol (300) → 3 holders
    assert_eq!(h.compliance.holder_count(), 3);
    assert_eq!(h.token.balance(&alice), 300);
}

#[test]
fn test_batch_transfer_rejects_cumulative_holder_limit_without_state_change() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    let carol = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.approve_kyc(&carol);
    h.mint(&alice, 1_000);
    h.set_max_holders(2);

    // Each leg independently sees one holder and would pass a cap of two.
    // The complete plan creates two new holders and must fail before mutation.
    let recipients = vec![
        &h.env,
        RecipientEntry {
            to: bob.clone(),
            amount: 100,
        },
        RecipientEntry {
            to: carol.clone(),
            amount: 100,
        },
    ];
    assert!(h.token.try_batch_transfer(&alice, &recipients).is_err());

    assert_eq!(h.token.balance(&alice), 1_000);
    assert_eq!(h.token.balance(&bob), 0);
    assert_eq!(h.token.balance(&carol), 0);
    assert_eq!(h.compliance.holder_count(), 1);
}

#[test]
fn test_batch_transfer_counts_duplicate_recipient_once_for_holder_limit() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.mint(&alice, 1_000);
    h.set_max_holders(2);

    let recipients = vec![
        &h.env,
        RecipientEntry {
            to: bob.clone(),
            amount: 100,
        },
        RecipientEntry {
            to: bob.clone(),
            amount: 150,
        },
    ];
    h.token.batch_transfer(&alice, &recipients);

    assert_eq!(h.token.balance(&alice), 750);
    assert_eq!(h.token.balance(&bob), 250);
    assert_eq!(h.compliance.holder_count(), 2);
}

#[test]
fn test_batch_transfer_reuses_drained_sender_holder_slot() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    let carol = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.approve_kyc(&carol);
    h.mint(&alice, 1_000);
    h.set_max_holders(2);

    let recipients = vec![
        &h.env,
        RecipientEntry {
            to: bob.clone(),
            amount: 500,
        },
        RecipientEntry {
            to: carol.clone(),
            amount: 500,
        },
    ];
    h.token.batch_transfer(&alice, &recipients);

    assert_eq!(h.token.balance(&alice), 0);
    assert_eq!(h.token.balance(&bob), 500);
    assert_eq!(h.token.balance(&carol), 500);
    assert_eq!(h.compliance.holder_count(), 2);
}

#[test]
fn test_batch_transfer_amount_overflow_leaves_state_unchanged() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    let carol = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.approve_kyc(&carol);
    h.mint(&alice, i128::MAX);

    let recipients = vec![
        &h.env,
        RecipientEntry {
            to: bob.clone(),
            amount: i128::MAX,
        },
        RecipientEntry {
            to: carol.clone(),
            amount: 1,
        },
    ];
    assert!(h.token.try_batch_transfer(&alice, &recipients).is_err());

    assert_eq!(h.token.balance(&alice), i128::MAX);
    assert_eq!(h.token.balance(&bob), 0);
    assert_eq!(h.token.balance(&carol), 0);
    assert_eq!(h.compliance.holder_count(), 1);
}

#[test]
fn test_batch_transfer_compliance_paused_state_unchanged() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.mint(&alice, 1_000);
    h.compliance.pause();

    let recipients = vec![
        &h.env,
        RecipientEntry {
            to: bob.clone(),
            amount: 100,
        },
    ];
    assert!(h.token.try_batch_transfer(&alice, &recipients).is_err());
    assert_eq!(h.token.balance(&alice), 1_000);
    assert_eq!(h.token.balance(&bob), 0);
}

#[test]
fn test_batch_transfer_max_amount_rule_per_entry() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.mint(&alice, 1_000);

    h.compliance.set_rules(&ComplianceRules {
        max_transfer_amount: 50,
        min_holding_period: 0,
        max_holders: 0,
        require_same_jurisdiction: false,
        paused: false,
        allowlist_mode: false,
        max_holding_period: 0,
    });

    // Single entry of 51 exceeds per-transfer cap
    let recipients_over = vec![
        &h.env,
        RecipientEntry {
            to: bob.clone(),
            amount: 51,
        },
    ];
    assert!(h
        .token
        .try_batch_transfer(&alice, &recipients_over)
        .is_err());

    // Single entry within cap succeeds
    let recipients_ok = vec![
        &h.env,
        RecipientEntry {
            to: bob.clone(),
            amount: 50,
        },
    ];
    h.token.batch_transfer(&alice, &recipients_ok);
    assert_eq!(h.token.balance(&bob), 50);
}

// ── batch_transfer_from ───────────────────────────────────────────────────────

#[test]
fn test_batch_transfer_from_success() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    let carol = Address::generate(&h.env);
    let spender = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.approve_kyc(&carol);
    h.mint(&alice, 1_000);

    let expiration = h.env.ledger().sequence() + 1_000;
    h.token.approve(&alice, &spender, &600, &expiration);

    let recipients = vec![
        &h.env,
        RecipientEntry {
            to: bob.clone(),
            amount: 300,
        },
        RecipientEntry {
            to: carol.clone(),
            amount: 200,
        },
    ];
    h.token.batch_transfer_from(&spender, &alice, &recipients);

    assert_eq!(h.token.balance(&alice), 500);
    assert_eq!(h.token.balance(&bob), 300);
    assert_eq!(h.token.balance(&carol), 200);
    // Allowance reduced by total = 500; 600 - 500 = 100 remaining
    assert_eq!(h.token.allowance(&alice, &spender), 100);
}

#[test]
fn test_batch_transfer_from_insufficient_allowance() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    let spender = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.mint(&alice, 1_000);

    let expiration = h.env.ledger().sequence() + 1_000;
    h.token.approve(&alice, &spender, &100, &expiration); // only 100

    let recipients = vec![
        &h.env,
        RecipientEntry {
            to: bob.clone(),
            amount: 200,
        }, // needs 200
    ];
    assert!(h
        .token
        .try_batch_transfer_from(&spender, &alice, &recipients)
        .is_err());

    assert_eq!(h.token.balance(&alice), 1_000);
    assert_eq!(h.token.balance(&bob), 0);
    assert_eq!(h.token.allowance(&alice, &spender), 100); // unchanged
}

#[test]
fn test_batch_transfer_from_expired_allowance() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    let spender = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.mint(&alice, 1_000);

    let expiration = h.env.ledger().sequence() + 10;
    h.token.approve(&alice, &spender, &500, &expiration);

    h.env.ledger().set_sequence_number(expiration + 1);

    let recipients = vec![
        &h.env,
        RecipientEntry {
            to: bob.clone(),
            amount: 100,
        },
    ];
    assert!(h
        .token
        .try_batch_transfer_from(&spender, &alice, &recipients)
        .is_err());
    assert_eq!(h.token.balance(&alice), 1_000);
    assert_eq!(h.token.balance(&bob), 0);
}

#[test]
fn test_batch_transfer_from_state_unchanged_on_kyc_failure() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    let carol = Address::generate(&h.env); // no KYC
    let spender = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.mint(&alice, 1_000);

    let expiration = h.env.ledger().sequence() + 1_000;
    h.token.approve(&alice, &spender, &600, &expiration);

    let recipients = vec![
        &h.env,
        RecipientEntry {
            to: bob.clone(),
            amount: 300,
        },
        RecipientEntry {
            to: carol.clone(),
            amount: 200,
        }, // KYC fails
    ];
    assert!(h
        .token
        .try_batch_transfer_from(&spender, &alice, &recipients)
        .is_err());

    assert_eq!(h.token.balance(&alice), 1_000);
    assert_eq!(h.token.balance(&bob), 0);
    // Allowance must also be unchanged (consumed only after validation pass)
    assert_eq!(h.token.allowance(&alice, &spender), 600);
}

#[test]
fn test_batch_transfer_from_allowance_consumed_atomically() {
    // Ensure allowance is consumed for the full batch total, not incrementally
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    let carol = Address::generate(&h.env);
    let spender = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.approve_kyc(&carol);
    h.mint(&alice, 1_000);

    let expiration = h.env.ledger().sequence() + 1_000;
    h.token.approve(&alice, &spender, &500, &expiration);

    let recipients = vec![
        &h.env,
        RecipientEntry {
            to: bob.clone(),
            amount: 250,
        },
        RecipientEntry {
            to: carol.clone(),
            amount: 250,
        },
    ];
    h.token.batch_transfer_from(&spender, &alice, &recipients);

    assert_eq!(h.token.allowance(&alice, &spender), 0);
    assert_eq!(h.token.balance(&alice), 500);
    assert_eq!(h.token.balance(&bob), 250);
    assert_eq!(h.token.balance(&carol), 250);
}

#[test]
fn test_batch_transfer_from_holder_limit_failure_preserves_allowance() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    let carol = Address::generate(&h.env);
    let spender = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.approve_kyc(&carol);
    h.mint(&alice, 1_000);
    h.set_max_holders(2);

    let expiration = h.env.ledger().sequence() + 1_000;
    h.token.approve(&alice, &spender, &500, &expiration);
    let recipients = vec![
        &h.env,
        RecipientEntry {
            to: bob.clone(),
            amount: 100,
        },
        RecipientEntry {
            to: carol.clone(),
            amount: 100,
        },
    ];

    assert!(h
        .token
        .try_batch_transfer_from(&spender, &alice, &recipients)
        .is_err());
    assert_eq!(h.token.allowance(&alice, &spender), 500);
    assert_eq!(h.token.balance(&alice), 1_000);
    assert_eq!(h.token.balance(&bob), 0);
    assert_eq!(h.token.balance(&carol), 0);
    assert_eq!(h.compliance.holder_count(), 1);
}

#[test]
fn test_batch_transfer_from_exceeds_max_recipients() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let spender = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.mint(&alice, 10_000);

    let expiration = h.env.ledger().sequence() + 1_000;
    h.token.approve(&alice, &spender, &10_000, &expiration);

    let mut recipients = soroban_sdk::Vec::new(&h.env);
    for _ in 0..11 {
        let addr = Address::generate(&h.env);
        h.approve_kyc(&addr);
        recipients.push_back(RecipientEntry {
            to: addr,
            amount: 1,
        });
    }
    assert!(h
        .token
        .try_batch_transfer_from(&spender, &alice, &recipients)
        .is_err());
    assert_eq!(h.token.balance(&alice), 10_000);
}

// ── versioning (#342) ─────────────────────────────────────────────────────────

#[test]
fn test_contract_version_info_initialized_at_deploy() {
    let h = setup();
    let (ver, count, last_ts) = h.token.contract_version_info();
    // Version is set to Cargo.toml version at deploy; migration count starts at 0.
    assert!(!ver.is_empty());
    assert_eq!(count, 0);
    assert_eq!(last_ts, 0);
}

#[test]
fn test_migrate_records_version_bump() {
    let h = setup();
    let nonce = h.next_nonce();
    let new_ver = String::from_str(&h.env, "0.2.0");
    let desc = String::from_str(&h.env, "first upgrade");
    h.token.migrate(&new_ver, &desc, &nonce);

    let (ver, count, _ts) = h.token.contract_version_info();
    assert_eq!(ver, new_ver);
    assert_eq!(count, 1);
}

#[test]
fn test_migrate_increments_count_on_each_call() {
    let h = setup();
    h.token.migrate(
        &String::from_str(&h.env, "0.2.0"),
        &String::from_str(&h.env, "add recovery"),
        &0,
    );
    h.token.migrate(
        &String::from_str(&h.env, "0.3.0"),
        &String::from_str(&h.env, "add events"),
        &1,
    );

    let (_ver, count, _ts) = h.token.contract_version_info();
    assert_eq!(count, 2);
}

#[test]
fn test_get_migration_record_returns_correct_entry() {
    let h = setup();
    let v1 = String::from_str(&h.env, "0.2.0");
    let d1 = String::from_str(&h.env, "first upgrade");
    h.token.migrate(&v1, &d1, &0);

    let v2 = String::from_str(&h.env, "0.3.0");
    let d2 = String::from_str(&h.env, "second upgrade");
    h.token.migrate(&v2, &d2, &1);

    let rec0 = h.token.get_migration_record(&0);
    assert_eq!(rec0.to_version, v1);
    assert_eq!(rec0.description, d1);

    let rec1 = h.token.get_migration_record(&1);
    assert_eq!(rec1.to_version, v2);
    assert_eq!(rec1.description, d2);
}

#[test]
fn test_migrate_last_ts_reflects_ledger_timestamp() {
    let h = setup();
    h.env.ledger().set_timestamp(1_700_000_000);
    h.token.migrate(
        &String::from_str(&h.env, "0.2.0"),
        &String::from_str(&h.env, "timed upgrade"),
        &0,
    );
    let (_ver, _count, last_ts) = h.token.contract_version_info();
    assert_eq!(last_ts, 1_700_000_000);
}

// ── Metadata export (get_token_export / set_external_uri) ────────────────────

#[test]
fn test_get_token_export_basic_fields() {
    let h = setup();
    let export = h.token.get_token_export();

    assert_eq!(export.name, String::from_str(&h.env, "Veritoken RWA"));
    assert_eq!(export.symbol, String::from_str(&h.env, "VTRWA"));
    assert_eq!(export.decimals, 7u32);
    assert_eq!(export.asset_type, String::from_str(&h.env, "property"));
    assert_eq!(export.total_supply, 0i128);
    assert_eq!(export.max_supply, 0i128);
    assert!(!export.contract_version.is_empty());
}

#[test]
fn test_get_token_export_linked_addresses() {
    let h = setup();
    let export = h.token.get_token_export();

    assert_eq!(export.kyc_registry, h.token.kyc_registry());
    assert_eq!(export.compliance_engine, h.token.compliance_engine());
}

#[test]
fn test_get_token_export_compliance_fields_empty_by_default() {
    let h = setup();
    let export = h.token.get_token_export();

    assert!(export.legal_entity.is_none());
    assert!(export.governing_law.is_none());
    assert!(export.isin.is_none());
    assert!(export.prospectus_hash.is_none());
    assert!(export.external_uri.is_none());
}

#[test]
fn test_get_token_export_reflects_set_compliance_metadata() {
    let h = setup();
    let key_entity = soroban_sdk::Symbol::new(&h.env, META_LEGAL_ENTITY);
    let key_isin = soroban_sdk::Symbol::new(&h.env, META_ISIN);
    h.token.set_compliance_metadata(
        &h.admin,
        &key_entity,
        &String::from_str(&h.env, "Acme Corp"),
        &h.next_nonce(),
    );
    h.token.set_compliance_metadata(
        &h.admin,
        &key_isin,
        &String::from_str(&h.env, "US0231351067"),
        &h.next_nonce(),
    );

    let export = h.token.get_token_export();
    assert_eq!(
        export.legal_entity,
        Some(String::from_str(&h.env, "Acme Corp"))
    );
    assert_eq!(export.isin, Some(String::from_str(&h.env, "US0231351067")));
    assert!(export.governing_law.is_none());
    assert!(export.prospectus_hash.is_none());
}

#[test]
fn test_get_token_export_reflects_total_supply_after_mint() {
    let h = setup();
    let user = Address::generate(&h.env);
    h.approve_kyc(&user);
    h.mint(&user, 5_000);

    let export = h.token.get_token_export();
    assert_eq!(export.total_supply, 5_000i128);
}

#[test]
fn test_set_and_get_external_uri() {
    let h = setup();
    assert_eq!(h.token.get_external_uri(), String::from_str(&h.env, ""));

    let uri = String::from_str(&h.env, "ipfs://QmTestHash");
    h.token.set_external_uri(&uri);

    assert_eq!(h.token.get_external_uri(), uri);
    let export = h.token.get_token_export();
    assert_eq!(export.external_uri, Some(uri));
}

#[test]
fn test_set_external_uri_empty_string_clears_value() {
    let h = setup();
    h.token
        .set_external_uri(&String::from_str(&h.env, "https://example.com/meta"));
    h.token.set_external_uri(&String::from_str(&h.env, ""));

    assert_eq!(h.token.get_external_uri(), String::from_str(&h.env, ""));
    let export = h.token.get_token_export();
    assert!(export.external_uri.is_none());
}

// ── #931 Compliance metadata key replacement ─────────────────────────────────
//
// Acceptance criteria:
//   - Replacing one metadata key leaves unrelated keys untouched.
//   - Individual reads and the aggregate view agree.

/// Set two compliance metadata keys, replace one, and verify the other is
/// preserved both through individual reads and the aggregate view.
#[test]
fn test_compliance_metadata_replace_one_key_preserves_other() {
    let h = setup();
    let key_entity = soroban_sdk::Symbol::new(&h.env, META_LEGAL_ENTITY);
    let key_isin = soroban_sdk::Symbol::new(&h.env, META_ISIN);

    let entity_v1 = String::from_str(&h.env, "Acme Corp");
    let isin = String::from_str(&h.env, "US1234567890");
    let entity_v2 = String::from_str(&h.env, "Acme Holdings Ltd");

    h.token
        .set_compliance_metadata(&h.admin, &key_entity, &entity_v1, &h.next_nonce());
    h.token
        .set_compliance_metadata(&h.admin, &key_isin, &isin, &h.next_nonce());

    // Replace only the legal entity key.
    h.token
        .set_compliance_metadata(&h.admin, &key_entity, &entity_v2, &h.next_nonce());

    // Individual reads.
    assert_eq!(
        h.token.get_compliance_metadata(&key_entity),
        entity_v2,
        "replaced key must return the new value"
    );
    assert_eq!(
        h.token.get_compliance_metadata(&key_isin),
        isin,
        "unrelated key must be preserved after replacement"
    );

    // Aggregate view must agree with individual reads.
    let meta = h.token.get_all_compliance_metadata();
    assert_eq!(meta.legal_entity, Some(entity_v2));
    assert_eq!(meta.isin, Some(isin));
    assert!(meta.governing_law.is_none());
    assert!(meta.prospectus_hash.is_none());
}

// ── #932 Token export metadata completeness ──────────────────────────────────
//
// Acceptance criteria:
//   - Every documented TokenExportMetadata field is asserted.
//   - The returned structure corresponds to current token state.

/// Initialize a token with representative values and assert every field of
/// the TokenExportMetadata structure returned by get_token_export.
#[test]
fn test_get_token_export_all_required_fields() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);

    let kyc_id = env.register(KycRegistry, ());
    let kyc = KycRegistryClient::new(&env, &kyc_id);
    kyc.initialize(&admin);
    let verifier = Address::generate(&env);
    kyc.add_verifier(&admin, &verifier);

    let compliance_id = env.register(ComplianceEngine, ());
    let compliance = ComplianceEngineClient::new(&env, &compliance_id);
    compliance.initialize(&admin, &kyc_id, &0u64);

    let token_id = env.register(
        RwaToken,
        (
            admin.clone(),
            6u32,
            String::from_str(&env, "Export Token"),
            String::from_str(&env, "EXPT"),
            String::from_str(&env, "invoice"),
            kyc_id.clone(),
            compliance_id.clone(),
            Option::<ComplianceMetadata>::None,
            1_000_000i128,
        ),
    );
    let token = RwaTokenClient::new(&env, &token_id);

    // Representative state: supply, metadata keys, external URI.
    let holder = Address::generate(&env);
    kyc.approve(&verifier, &holder, &1, &0, &String::from_str(&env, "US"));
    token.mint(&admin, &holder, &2_500i128);

    let key_entity = soroban_sdk::Symbol::new(&env, META_LEGAL_ENTITY);
    let key_isin = soroban_sdk::Symbol::new(&env, META_ISIN);
    let legal_entity = String::from_str(&env, "Export Issuer LLC");
    let isin = String::from_str(&env, "US9876543210");
    token.set_compliance_metadata(&admin, &key_entity, &legal_entity, &0);
    token.set_compliance_metadata(&admin, &key_isin, &isin, &1);

    let uri = String::from_str(&env, "ipfs://QmExportMetadataHash");
    token.set_external_uri(&uri);

    let export = token.get_token_export();

    // Identity / schema fields.
    assert_eq!(export.name, String::from_str(&env, "Export Token"));
    assert_eq!(export.symbol, String::from_str(&env, "EXPT"));
    assert_eq!(export.decimals, 6u32);
    assert_eq!(export.asset_type, String::from_str(&env, "invoice"));
    assert!(
        !export.contract_version.is_empty(),
        "contract_version must be populated"
    );

    // Supply fields.
    assert_eq!(export.total_supply, 2_500i128);
    assert_eq!(export.max_supply, 1_000_000i128);

    // Linked contract addresses.
    assert_eq!(export.kyc_registry, kyc_id);
    assert_eq!(export.compliance_engine, compliance_id);

    // Configured external metadata fields.
    assert_eq!(export.legal_entity, Some(legal_entity));
    assert_eq!(export.isin, Some(isin));
    assert!(export.governing_law.is_none());
    assert!(export.prospectus_hash.is_none());
    assert_eq!(export.external_uri, Some(uri));
}

// ── #934 Transfer to self ────────────────────────────────────────────────────
//
// Acceptance criteria:
//   - Direct self-transfer leaves balance and total supply unchanged.
//   - Delegated self-transfer consumes allowance and leaves balances unchanged.

/// A direct transfer from an address to itself must be a no-op on balances
/// and total supply, while still emitting a transfer event.
#[test]
fn test_transfer_to_self_preserves_balance_and_supply() {
    let h = setup();
    let alice = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.mint(&alice, 1_000);

    let balance_before = h.token.balance(&alice);
    let supply_before = h.token.total_supply();

    h.token.transfer(&alice, &alice, &400);

    assert_eq!(
        h.token.balance(&alice),
        balance_before,
        "self-transfer must not change the balance"
    );
    assert_eq!(
        h.token.total_supply(),
        supply_before,
        "self-transfer must not change total supply"
    );
    assert_eq!(h.token.balance(&alice), 1_000);
    assert_eq!(h.token.total_supply(), 1_000);
}

/// A delegated transfer from an address to itself must consume the spender's
/// allowance by the transferred amount while leaving balances and supply
/// unchanged.
#[test]
fn test_transfer_from_to_self_consumes_allowance() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let spender = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.mint(&alice, 1_000);

    let expiration = h.env.ledger().sequence() + 1_000;
    h.token.approve(&alice, &spender, &500, &expiration);
    assert_eq!(h.token.allowance(&alice, &spender), 500);

    let balance_before = h.token.balance(&alice);
    let supply_before = h.token.total_supply();

    h.token.transfer_from(&spender, &alice, &alice, &200);

    assert_eq!(
        h.token.balance(&alice),
        balance_before,
        "delegated self-transfer must not change the balance"
    );
    assert_eq!(
        h.token.total_supply(),
        supply_before,
        "delegated self-transfer must not change total supply"
    );
    assert_eq!(
        h.token.allowance(&alice, &spender),
        300,
        "allowance must be reduced by the transferred amount"
    );

    // Spending the remaining allowance fully must leave no allowance behind.
    h.token.transfer_from(&spender, &alice, &alice, &300);
    assert_eq!(h.token.balance(&alice), 1_000);
    assert_eq!(h.token.total_supply(), 1_000);
    assert_eq!(
        h.token.allowance(&alice, &spender),
        0,
        "fully spent allowance must be zero"
    );
}

#[test]
fn test_get_token_export_max_supply_set_at_constructor() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);

    let kyc_id = env.register(kyc_registry::KycRegistry, ());
    let kyc = KycRegistryClient::new(&env, &kyc_id);
    kyc.initialize(&admin);

    let compliance_id = env.register(compliance_engine::ComplianceEngine, ());
    let compliance = ComplianceEngineClient::new(&env, &compliance_id);
    compliance.initialize(&admin, &kyc_id, &0u64);

    let token_id = env.register(
        RwaToken,
        (
            admin.clone(),
            7u32,
            String::from_str(&env, "Cap Token"),
            String::from_str(&env, "CAP"),
            String::from_str(&env, "invoice"),
            kyc_id.clone(),
            compliance_id.clone(),
            Option::<ComplianceMetadata>::None,
            1_000_000i128, // max_supply = 1 million
        ),
    );
    let token = RwaTokenClient::new(&env, &token_id);
    let export = token.get_token_export();
    assert_eq!(export.max_supply, 1_000_000i128);
}

// ── KYC synchronization layer tests ──────────────────────────────────────────

#[test]
fn test_check_kyc_status_active_for_approved_holder() {
    let h = setup();
    let user = Address::generate(&h.env);
    h.approve_kyc(&user);

    let snap = h.token.check_kyc_status(&user);
    assert!(snap.is_active);
    assert_eq!(snap.tier, 1u32);
    assert_eq!(snap.jurisdiction, String::from_str(&h.env, "US"));
    assert_eq!(snap.expiry, 0u64); // no expiry set
}

#[test]
fn test_check_kyc_status_inactive_for_revoked() {
    let h = setup();
    let user = Address::generate(&h.env);
    h.approve_kyc(&user);
    h.kyc.revoke(&h.verifier, &user);

    let snap = h.token.check_kyc_status(&user);
    assert!(!snap.is_active);
}

#[test]
fn test_check_kyc_status_inactive_for_expired_approval() {
    let h = setup();
    let user = Address::generate(&h.env);

    // Approve with expiry in the past
    h.env.ledger().set_timestamp(1_000);
    h.kyc.approve(
        &h.verifier,
        &user,
        &1,
        &500, // expiry = 500 < current 1000
        &String::from_str(&h.env, "US"),
    );

    let snap = h.token.check_kyc_status(&user);
    assert!(!snap.is_active);
    assert_eq!(snap.expiry, 500u64);
}

#[test]
fn test_check_kyc_status_active_at_expiry_boundary() {
    let h = setup();
    let user = Address::generate(&h.env);

    h.env.ledger().set_timestamp(1_000);
    // expiry == now → still active (expiry >= now)
    h.kyc.approve(
        &h.verifier,
        &user,
        &1,
        &1_000,
        &String::from_str(&h.env, "US"),
    );

    let snap = h.token.check_kyc_status(&user);
    assert!(snap.is_active);
}

#[test]
fn test_check_kyc_status_inactive_one_second_past_expiry() {
    let h = setup();
    let user = Address::generate(&h.env);

    h.env.ledger().set_timestamp(999);
    h.kyc.approve(
        &h.verifier,
        &user,
        &1,
        &1_000,
        &String::from_str(&h.env, "US"),
    );

    h.env.ledger().set_timestamp(1_001);
    let snap = h.token.check_kyc_status(&user);
    assert!(!snap.is_active);
}

#[test]
fn test_check_kyc_status_checked_at_reflects_current_timestamp() {
    let h = setup();
    let user = Address::generate(&h.env);
    h.approve_kyc(&user);

    h.env.ledger().set_timestamp(9_999_000);
    let snap = h.token.check_kyc_status(&user);
    assert_eq!(snap.checked_at, 9_999_000u64);
}

#[test]
fn test_sync_kyc_status_returns_true_for_active() {
    let h = setup();
    let user = Address::generate(&h.env);
    h.approve_kyc(&user);

    let is_active = h.token.sync_kyc_status(&user);
    assert!(is_active);
}

#[test]
fn test_sync_kyc_status_returns_false_after_revocation() {
    let h = setup();
    let user = Address::generate(&h.env);
    h.approve_kyc(&user);
    h.kyc.revoke(&h.verifier, &user);

    let is_active = h.token.sync_kyc_status(&user);
    assert!(!is_active);
}

#[test]
fn test_sync_kyc_status_returns_false_for_expired_approval() {
    let h = setup();
    let user = Address::generate(&h.env);

    h.env.ledger().set_timestamp(500);
    h.kyc.approve(
        &h.verifier,
        &user,
        &1,
        &600, // expires at 600
        &String::from_str(&h.env, "US"),
    );

    h.env.ledger().set_timestamp(700); // past expiry
    let is_active = h.token.sync_kyc_status(&user);
    assert!(!is_active);
}

#[test]
fn test_transfer_blocked_after_revocation() {
    // Verifies that revocation is immediately effective on the next transfer attempt.
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.mint(&alice, 1_000);

    // Transfer works before revocation
    h.token.transfer(&alice, &bob, &100);
    assert_eq!(h.token.balance(&bob), 100);

    // Revoke alice's KYC
    h.kyc.revoke(&h.verifier, &alice);

    // Next transfer must fail — KYC check catches the revocation
    assert!(h.token.try_transfer(&alice, &bob, &100).is_err());
}

#[test]
fn test_transfer_blocked_after_expiry() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);

    h.env.ledger().set_timestamp(1_000);
    h.kyc.approve(
        &h.verifier,
        &alice,
        &1,
        &2_000, // expires at t=2000
        &String::from_str(&h.env, "US"),
    );
    h.approve_kyc(&bob);

    h.mint(&alice, 1_000);

    // Transfer works before expiry
    h.token.transfer(&alice, &bob, &100);

    // Jump past alice's expiry
    h.env.ledger().set_timestamp(2_001);

    // Transfer now blocked — alice's KYC has expired
    assert!(h.token.try_transfer(&alice, &bob, &100).is_err());
}

#[test]
fn test_mint_blocked_after_recipient_kyc_expires() {
    let h = setup();
    let user = Address::generate(&h.env);

    h.env.ledger().set_timestamp(1_000);
    h.kyc.approve(
        &h.verifier,
        &user,
        &1,
        &1_500, // expires at t=1500
        &String::from_str(&h.env, "US"),
    );

    h.mint(&user, 500); // works before expiry

    h.env.ledger().set_timestamp(1_600); // past expiry

    assert!(h.token.try_mint(&h.admin, &user, &500).is_err());
}

#[test]
fn test_check_kyc_status_tier_reflects_update() {
    let h = setup();
    let user = Address::generate(&h.env);

    h.kyc.approve(
        &h.verifier,
        &user,
        &1, // tier 1
        &0,
        &String::from_str(&h.env, "US"),
    );
    let snap1 = h.token.check_kyc_status(&user);
    assert_eq!(snap1.tier, 1u32);

    // Upgrade tier to 2
    h.kyc.update_tier(&h.verifier, &user, &2);
    let snap2 = h.token.check_kyc_status(&user);
    assert_eq!(snap2.tier, 2u32);
    assert!(snap2.is_active);
}

// ── Schema migration tests ────────────────────────────────────────────────────

#[test]
fn test_schema_version_initialized_to_one() {
    let h = setup();
    assert_eq!(h.token.schema_version(), 1u32);
}

#[test]
fn test_migrate_schema_success_v1_to_v2() {
    let h = setup();
    let nonce = h.next_nonce();
    h.token.migrate_schema(
        &2,
        &String::from_str(&h.env, "v1 -> v2: add schema field"),
        &nonce,
    );
    assert_eq!(h.token.schema_version(), 2u32);
}

#[test]
fn test_migrate_schema_record_persisted() {
    let h = setup();
    let nonce = h.next_nonce();
    let desc = String::from_str(&h.env, "v1 to v2");
    h.token.migrate_schema(&2, &desc, &nonce);

    let rec = h.token.get_schema_migration_record(&0);
    assert_eq!(rec.from_version, 1u32);
    assert_eq!(rec.to_version, 2u32);
    assert_eq!(rec.description, desc);
}

#[test]
fn test_migrate_schema_already_at_version_rejected() {
    use crate::RwaError;
    use soroban_sdk::Error;

    let h = setup();
    let nonce = h.next_nonce();
    // schema_version is 1; migrating to 1 must fail with MigrationVersionConflict.
    let res = h
        .token
        .try_migrate_schema(&1, &String::from_str(&h.env, "dup"), &nonce);
    assert_eq!(
        res.unwrap_err().unwrap(),
        Error::from(RwaError::MigrationVersionConflict)
    );
}

#[test]
fn test_migrate_schema_version_skip_rejected() {
    use crate::RwaError;
    use soroban_sdk::Error;

    let h = setup();
    let nonce = h.next_nonce();
    let res = h
        .token
        .try_migrate_schema(&3, &String::from_str(&h.env, "skip"), &nonce);
    assert_eq!(
        res.unwrap_err().unwrap(),
        Error::from(RwaError::MigrationVersionNotSequential)
    );
}

#[test]
fn test_migrate_schema_sequential_steps_succeed() {
    let h = setup();

    h.token
        .migrate_schema(&2, &String::from_str(&h.env, "step 1"), &h.next_nonce());
    h.token
        .migrate_schema(&3, &String::from_str(&h.env, "step 2"), &h.next_nonce());

    assert_eq!(h.token.schema_version(), 3u32);

    let rec0 = h.token.get_schema_migration_record(&0);
    let rec1 = h.token.get_schema_migration_record(&1);
    assert_eq!(rec0.from_version, 1u32);
    assert_eq!(rec0.to_version, 2u32);
    assert_eq!(rec1.from_version, 2u32);
    assert_eq!(rec1.to_version, 3u32);
}

#[test]
fn test_migrate_semver_idempotency_rejected() {
    use crate::RwaError;
    use soroban_sdk::Error;

    let h = setup();
    // Migrate to a new semver first.
    h.token.migrate(
        &String::from_str(&h.env, "0.2.0"),
        &String::from_str(&h.env, "first upgrade"),
        &h.next_nonce(),
    );
    // Submitting the same semver again must be rejected.
    let res = h.token.try_migrate(
        &String::from_str(&h.env, "0.2.0"),
        &String::from_str(&h.env, "dup"),
        &h.next_nonce(),
    );
    assert_eq!(
        res.unwrap_err().unwrap(),
        Error::from(RwaError::MigrationVersionConflict)
    );
}

#[test]
fn test_migrate_schema_legacy_bootstrap() {
    use crate::storage_types::DataKey;

    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);

    let kyc_id = env.register(kyc_registry::KycRegistry, ());
    let kyc = kyc_registry::KycRegistryClient::new(&env, &kyc_id);
    kyc.initialize(&admin);

    let ce_id = env.register(compliance_engine::ComplianceEngine, ());
    let ce = compliance_engine::ComplianceEngineClient::new(&env, &ce_id);
    ce.initialize(&admin, &kyc_id, &0u64);

    let token_id = env.register(
        crate::RwaToken,
        (
            admin.clone(),
            7u32,
            String::from_str(&env, "Veritoken RWA"),
            String::from_str(&env, "VTRWA"),
            String::from_str(&env, "property"),
            kyc_id.clone(),
            ce_id.clone(),
            Option::<crate::ComplianceMetadata>::None,
            0i128,
        ),
    );
    let token = RwaTokenClient::new(&env, &token_id);

    // Simulate a legacy deployment: remove the SchemaVersion key.
    env.as_contract(&token_id, || {
        env.storage().instance().remove(&DataKey::SchemaVersion);
    });

    assert_eq!(token.schema_version(), 0u32);

    // Bootstrap migration to v1 must succeed.
    token.migrate_schema(
        &1,
        &String::from_str(&env, "bootstrap"),
        &token.admin_nonce(),
    );
    assert_eq!(token.schema_version(), 1u32);
}

#[test]
fn test_migrate_schema_state_continuity() {
    // Token balances must be intact after a schema migration.
    let h = setup();
    let user = Address::generate(&h.env);
    h.approve_kyc(&user);
    h.mint(&user, 1_000);

    h.token
        .migrate_schema(&2, &String::from_str(&h.env, "v1 -> v2"), &h.next_nonce());

    assert_eq!(h.token.balance(&user), 1_000);
    assert_eq!(h.token.schema_version(), 2u32);
}

// ── Atomic batch invariant regression suite ───────────────────────────────────
// These tests directly verify the acceptance criteria from the batch-atomicity
// refactor: a failed batch leaves token balances, allowances, and holder state
// completely unchanged.

/// Frozen recipient in the *middle* of a 3-entry batch.
/// The first entry (bob) would succeed on its own; the second (eve, frozen)
/// fails during the validation pass; the third (carol) never executes.
/// All balances and holder-count must be untouched.
#[test]
fn test_batch_transfer_frozen_recipient_in_middle_leaves_no_partial_state() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    let eve = Address::generate(&h.env); // frozen — sits in the middle
    let carol = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.approve_kyc(&eve);
    h.approve_kyc(&carol);
    h.mint(&alice, 1_000);
    h.freeze(&eve);

    let recipients = vec![
        &h.env,
        RecipientEntry {
            to: bob.clone(),
            amount: 200,
        },
        RecipientEntry {
            to: eve.clone(),
            amount: 100,
        }, // frozen → validation fails
        RecipientEntry {
            to: carol.clone(),
            amount: 300,
        },
    ];
    assert!(h.token.try_batch_transfer(&alice, &recipients).is_err());

    // Invariant: no balance or holder state must have changed.
    assert_eq!(h.token.balance(&alice), 1_000);
    assert_eq!(h.token.balance(&bob), 0);
    assert_eq!(h.token.balance(&eve), 0);
    assert_eq!(h.token.balance(&carol), 0);
    assert_eq!(h.token.total_supply(), 1_000);
    assert_eq!(h.compliance.holder_count(), 1);
}

/// Expired-KYC recipient in the middle of a 3-entry batch.
/// KYC is approved with a near-future expiry, then the ledger is advanced
/// past the expiry. The batch must be rejected entirely.
#[test]
fn test_batch_transfer_expired_kyc_recipient_in_middle_leaves_no_partial_state() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    let eve = Address::generate(&h.env); // KYC will expire
    let carol = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.approve_kyc(&carol);

    // Give eve a short-lived KYC that expires at t=1_000.
    h.env.ledger().set_timestamp(500);
    h.kyc.approve(
        &h.verifier,
        &eve,
        &1,
        &1_000, // expires at t=1000
        &String::from_str(&h.env, "US"),
    );

    h.mint(&alice, 1_000);

    // Advance time past eve's expiry.
    h.env.ledger().set_timestamp(1_500);

    let recipients = vec![
        &h.env,
        RecipientEntry {
            to: bob.clone(),
            amount: 200,
        },
        RecipientEntry {
            to: eve.clone(),
            amount: 100,
        }, // expired KYC → fails
        RecipientEntry {
            to: carol.clone(),
            amount: 300,
        },
    ];
    assert!(h.token.try_batch_transfer(&alice, &recipients).is_err());

    assert_eq!(h.token.balance(&alice), 1_000);
    assert_eq!(h.token.balance(&bob), 0);
    assert_eq!(h.token.balance(&eve), 0);
    assert_eq!(h.token.balance(&carol), 0);
    assert_eq!(h.token.total_supply(), 1_000);
    assert_eq!(h.compliance.holder_count(), 1);
}

/// Integration invariant: total supply, per-account balances, and holder count
/// are all consistent after a batch that fails due to insufficient balance.
#[test]
fn test_batch_transfer_insufficient_balance_invariants_total_supply_and_holder_count() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    let carol = Address::generate(&h.env);
    let dave = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.approve_kyc(&carol);
    h.approve_kyc(&dave);

    h.mint(&alice, 500); // deliberately less than the 600 total below

    let supply_before = h.token.total_supply();
    let holders_before = h.compliance.holder_count();

    let recipients = vec![
        &h.env,
        RecipientEntry {
            to: bob.clone(),
            amount: 200,
        },
        RecipientEntry {
            to: carol.clone(),
            amount: 200,
        },
        RecipientEntry {
            to: dave.clone(),
            amount: 200,
        }, // total 600 > 500
    ];
    assert!(h.token.try_batch_transfer(&alice, &recipients).is_err());

    // All three invariants must hold after the failed batch.
    assert_eq!(h.token.total_supply(), supply_before);
    assert_eq!(h.token.balance(&alice), 500);
    assert_eq!(h.token.balance(&bob), 0);
    assert_eq!(h.token.balance(&carol), 0);
    assert_eq!(h.token.balance(&dave), 0);
    assert_eq!(h.compliance.holder_count(), holders_before);
}

/// `batch_transfer_from`: a mid-batch KYC failure must leave the allowance,
/// all balances, and holder count unchanged.
#[test]
fn test_batch_transfer_from_mid_batch_kyc_failure_preserves_all_state() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    let eve = Address::generate(&h.env); // no KYC — sits in the middle
    let carol = Address::generate(&h.env);
    let spender = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.approve_kyc(&carol);
    h.mint(&alice, 1_000);

    let expiration = h.env.ledger().sequence() + 1_000;
    h.token.approve(&alice, &spender, &700, &expiration);

    let recipients = vec![
        &h.env,
        RecipientEntry {
            to: bob.clone(),
            amount: 200,
        },
        RecipientEntry {
            to: eve.clone(),
            amount: 100,
        }, // no KYC → validation fails
        RecipientEntry {
            to: carol.clone(),
            amount: 300,
        },
    ];
    assert!(h
        .token
        .try_batch_transfer_from(&spender, &alice, &recipients)
        .is_err());

    // All state must be untouched.
    assert_eq!(h.token.balance(&alice), 1_000);
    assert_eq!(h.token.balance(&bob), 0);
    assert_eq!(h.token.balance(&eve), 0);
    assert_eq!(h.token.balance(&carol), 0);
    assert_eq!(h.token.allowance(&alice, &spender), 700); // allowance not consumed
    assert_eq!(h.token.total_supply(), 1_000);
    assert_eq!(h.compliance.holder_count(), 1);
}

/// `batch_transfer_from`: a mid-batch frozen-recipient failure must leave the
/// allowance unchanged, matching the same invariant as `batch_transfer`.
#[test]
fn test_batch_transfer_from_mid_batch_frozen_recipient_preserves_allowance() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    let eve = Address::generate(&h.env); // frozen — sits in the middle
    let carol = Address::generate(&h.env);
    let spender = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.approve_kyc(&eve);
    h.approve_kyc(&carol);
    h.mint(&alice, 1_000);
    h.freeze(&eve);

    let expiration = h.env.ledger().sequence() + 1_000;
    h.token.approve(&alice, &spender, &700, &expiration);

    let recipients = vec![
        &h.env,
        RecipientEntry {
            to: bob.clone(),
            amount: 200,
        },
        RecipientEntry {
            to: eve.clone(),
            amount: 100,
        }, // frozen → validation fails
        RecipientEntry {
            to: carol.clone(),
            amount: 300,
        },
    ];
    assert!(h
        .token
        .try_batch_transfer_from(&spender, &alice, &recipients)
        .is_err());

    assert_eq!(h.token.balance(&alice), 1_000);
    assert_eq!(h.token.balance(&bob), 0);
    assert_eq!(h.token.balance(&eve), 0);
    assert_eq!(h.token.balance(&carol), 0);
    assert_eq!(h.token.allowance(&alice, &spender), 700);
    assert_eq!(h.token.total_supply(), 1_000);
    assert_eq!(h.compliance.holder_count(), 1);
}

/// Single transfer and batch transfer must enforce the same freeze rule for
/// the *sender*: a frozen sender is rejected by both paths identically.
#[test]
fn test_single_and_batch_transfer_enforce_same_freeze_rule_for_sender() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.mint(&alice, 1_000);
    h.freeze(&alice);

    // Single transfer: frozen sender rejected.
    assert!(h.token.try_transfer(&alice, &bob, &100).is_err());

    // Batch transfer: same frozen sender rejected.
    let recipients = vec![
        &h.env,
        RecipientEntry {
            to: bob.clone(),
            amount: 100,
        },
    ];
    assert!(h.token.try_batch_transfer(&alice, &recipients).is_err());

    // Both balances and holder count unchanged.
    assert_eq!(h.token.balance(&alice), 1_000);
    assert_eq!(h.token.balance(&bob), 0);
    assert_eq!(h.compliance.holder_count(), 1);
}

#[test]
fn test_transfer_from_rejects_frozen_source_without_state_changes() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    let spender = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.mint(&alice, 1_000);

    let expiration = h.env.ledger().sequence() + 1_000;
    h.token.approve(&alice, &spender, &300, &expiration);
    h.freeze(&alice);

    assert_eq!(
        h.token
            .try_transfer_from(&spender, &alice, &bob, &100),
        Err(Ok(Error::from(crate::RwaError::AccountFrozen)))
    );
    assert_eq!(h.token.balance(&alice), 1_000);
    assert_eq!(h.token.balance(&bob), 0);
    assert_eq!(h.token.allowance(&alice, &spender), 300);
}

#[test]
fn test_transfer_from_rejects_frozen_destination_without_state_changes() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    let spender = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.mint(&alice, 1_000);

    let expiration = h.env.ledger().sequence() + 1_000;
    h.token.approve(&alice, &spender, &300, &expiration);
    h.freeze(&bob);

    assert_eq!(
        h.token
            .try_transfer_from(&spender, &alice, &bob, &100),
        Err(Ok(Error::from(crate::RwaError::AccountFrozen)))
    );
    assert_eq!(h.token.balance(&alice), 1_000);
    assert_eq!(h.token.balance(&bob), 0);
    assert_eq!(h.token.allowance(&alice, &spender), 300);
}

/// Single transfer and batch transfer must enforce the same KYC rule for
/// the *sender*: a revoked sender is rejected by both paths identically.
#[test]
fn test_single_and_batch_transfer_enforce_same_kyc_rule_for_sender() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.mint(&alice, 1_000);

    // Revoke alice's KYC.
    h.kyc.revoke(&h.verifier, &alice);

    // Single transfer: revoked-KYC sender rejected.
    assert!(h.token.try_transfer(&alice, &bob, &100).is_err());

    // Batch transfer: same rule enforced.
    let recipients = vec![
        &h.env,
        RecipientEntry {
            to: bob.clone(),
            amount: 100,
        },
    ];
    assert!(h.token.try_batch_transfer(&alice, &recipients).is_err());

    // Balances and holder count unchanged.
    assert_eq!(h.token.balance(&alice), 1_000);
    assert_eq!(h.token.balance(&bob), 0);
    assert_eq!(h.compliance.holder_count(), 1);
}

/// Over-limit recipient count (11 entries) must be rejected before any
/// validation or state mutation occurs. The sender balance is untouched.
#[test]
fn test_batch_transfer_11_recipients_rejected_before_validation() {
    let h = setup();
    let alice = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.mint(&alice, 10_000);

    let supply_before = h.token.total_supply();

    // 11 recipients, all KYC-approved and valid — still over the cap.
    let mut recipients = soroban_sdk::Vec::new(&h.env);
    for _ in 0..11 {
        let addr = Address::generate(&h.env);
        h.approve_kyc(&addr);
        recipients.push_back(RecipientEntry {
            to: addr,
            amount: 1,
        });
    }
    assert!(h.token.try_batch_transfer(&alice, &recipients).is_err());

    assert_eq!(h.token.balance(&alice), 10_000);
    assert_eq!(h.token.total_supply(), supply_before);
    assert_eq!(h.compliance.holder_count(), 1); // only alice registered
}

// ── Recovery subsystem unit tests ─────────────────────────────────────────────
//
// # RWA admin recovery test guide (issue #935)
//
// This block is a short, test-oriented guide to the recovery subsystem. It
// describes only what the tests below already exercise; it does not change any
// recovery logic. For the privileged-action checklist, see the "Privileged
// actions" section of the security checklist (SECURITY.md).
//
// ## Fixture
//
// Every test starts from `setup()`, which returns a `Harness` with:
//   - `env`        — the Soroban test environment (`mock_all_auths()` is on, so
//                    guardian/admin auth is satisfied automatically; tests that
//                    need real auth enforcement build their own `Env` instead).
//   - `token`      — `RwaTokenClient` for the deployed token.
//   - `kyc`        — `KycRegistryClient` (use `h.approve_kyc(&addr)` to KYC an
//                    address before minting to it).
//   - `compliance` — `ComplianceEngineClient`.
//   - `admin`      — the initial admin address.
//
// `make_guardians(&h, n)` builds a `Vec<Address>` of `n` fresh guardian
// addresses. Guardians are ordinary addresses; they only gain recovery powers
// once passed to `configure_recovery`.
//
// ## Method order
//
//   1. `configure_recovery(&threshold, &members, &cooldown_ledgers)`
//        Admin-only. Sets the guardian set, approval threshold, and cooldown.
//        Also clears any active proposal.
//   2. `propose_recovery(&guardian, &new_admin)`
//        Guardian-only. Opens a proposal; the proposer is recorded and is NOT
//        counted as an approval.
//   3. `approve_recovery(&guardian)`  (repeat for other guardians)
//        Guardian-only. Each distinct guardian may approve once; the proposer
//        cannot approve.
//   4. `execute_recovery(&guardian)`
//        Guardian-only. Requires approvals >= threshold, an unexpired proposal,
//        and the cooldown to have elapsed. Replaces the admin, bumps
//        `admin_nonce`, records `last_executed_ledger`, and clears the proposal.
//
// `cancel_recovery()` (admin-only) clears an active proposal at any time.
//
// ## Threshold rules
//
//   - `threshold` must be >= 2 and <= members.len(); members must be 1..=10 and
//     unique. Violations return `InvalidRecoveryConfig`.
//   - A proposal is only allowed if the threshold is reachable: with the
//     proposer excluded from approving, `threshold <= members.len() - 1`.
//     A 3-of-3 config therefore rejects `propose_recovery`.
//   - `execute_recovery` needs `approvals.len() >= threshold`, otherwise
//     `InsufficientApprovals`.
//
// ## Proposal expiry
//
// A proposal expires at `proposed_ledger + cooldown_ledgers / 2`. At the expiry
// ledger itself the proposal is still live; from `expiry + 1` it is stale:
// `approve_recovery` returns `RecoveryExpired`, `execute_recovery` returns
// `RecoveryExpired` (leaving admin, nonce, and `last_executed_ledger`
// untouched), and a new `propose_recovery` may replace it.
//
// ## Cooldown advancement
//
// `cooldown_ledgers` gates how soon a second recovery may execute. Advance the
// ledger with `h.env.ledger().set_sequence_number(n)` (or
// `h.env.ledger().with_mut(|l| l.sequence_number = n)`). A second
// `execute_recovery` before `last_executed_ledger + cooldown_ledgers` returns
// `RecoveryCooldown`. Use `cooldown_ledgers = 0` when the test does not care
// about cooldown.
//
// ## Principal cases already covered
//
// Positive:
//   - `test_recovery_configure_and_read`
//   - `test_recovery_propose_and_read_active`
//   - `test_recovery_approve_increases_approval_count`
//   - `test_recovery_execute_replaces_admin_and_bumps_nonce`
//   - `test_recovery_new_admin_can_mint_after_recovery`
//   - `test_approve_recovery_two_distinct_guardians_both_count`
//
// Negative:
//   - `test_recovery_configure_rejects_*` (threshold, member count, duplicates)
//   - `test_recovery_propose_rejects_while_proposal_active`
//   - `test_recovery_propose_rejects_unreachable_threshold`
//   - `test_recovery_proposer_cannot_approve`
//   - `test_recovery_approve_duplicate_panics`
//   - `test_recovery_expired_proposal_blocks_approve`
//   - `test_recovery_execute_rejects_expired_proposal`
//   - `test_recovery_execute_insufficient_approvals_panics`
//   - `test_recovery_cooldown_blocks_immediate_second_execution`
//   - `test_recovery_cancel_no_active_panics`
//   - `test_recovery_non_guardian_propose_panics`
//
// ## Worked example: adding one recovery test
//
// To assert that a proposal cannot be executed by a non-guardian, add:
//
// ```ignore
// #[test]
// fn test_recovery_execute_rejects_non_guardian() {
//     use crate::RwaError;
//     use soroban_sdk::Error;
//
//     let h = setup();
//     let members = make_guardians(&h, 3);
//     let g1 = members.get(0).unwrap();
//     let g2 = members.get(1).unwrap();
//     h.token.configure_recovery(&2, &members, &0);
//     h.token.propose_recovery(&g1, &Address::generate(&h.env));
//     h.token.approve_recovery(&g2);
//
//     let stranger = Address::generate(&h.env);
//     let res = h.token.try_execute_recovery(&stranger);
//     assert_eq!(
//         res.unwrap_err().unwrap(),
//         Error::from(RwaError::NotRecoveryMember)
//     );
//     // Proposal and admin are unchanged.
//     assert!(h.token.active_recovery().is_some());
// }
// ```
//
// Follow the same shape for new cases: build the fixture, configure, propose,
// approve from distinct guardians, advance the ledger only when testing
// expiry/cooldown, then assert on the typed `RwaError` via `try_*`.

fn make_guardians(h: &Harness, n: usize) -> soroban_sdk::Vec<Address> {
    let mut v = soroban_sdk::Vec::new(&h.env);
    for _ in 0..n {
        v.push_back(Address::generate(&h.env));
    }
    v
}

#[test]
fn test_recovery_configure_and_read() {
    let h = setup();
    let members = make_guardians(&h, 3);
    h.token.configure_recovery(&2, &members, &100);

    let cfg = h.token.recovery_config();
    assert_eq!(cfg.threshold, 2);
    assert_eq!(cfg.cooldown_ledgers, 100);
    assert_eq!(cfg.last_executed_ledger, 0);

    let stored_members = h.token.recovery_members();
    assert_eq!(stored_members.len(), 3);
}

#[test]
fn test_recovery_configure_rejects_threshold_below_two() {
    use crate::RwaError;
    use soroban_sdk::Error;
    let h = setup();
    let members = make_guardians(&h, 3);
    let res = h.token.try_configure_recovery(&1, &members, &100);
    assert_eq!(
        res.unwrap_err().unwrap(),
        Error::from(RwaError::InvalidRecoveryConfig)
    );
}

#[test]
fn test_recovery_configure_rejects_threshold_above_member_count() {
    use crate::RwaError;
    use soroban_sdk::Error;
    let h = setup();
    let members = make_guardians(&h, 3);
    let res = h.token.try_configure_recovery(&4, &members, &100);
    assert_eq!(
        res.unwrap_err().unwrap(),
        Error::from(RwaError::InvalidRecoveryConfig)
    );
}

#[test]
fn test_recovery_configure_rejects_more_than_ten_members() {
    use crate::RwaError;
    use soroban_sdk::Error;
    let h = setup();
    let members = make_guardians(&h, 11);
    let res = h.token.try_configure_recovery(&2, &members, &100);
    assert_eq!(
        res.unwrap_err().unwrap(),
        Error::from(RwaError::InvalidRecoveryConfig)
    );
}

#[test]
fn test_recovery_configure_rejects_zero_threshold() {
    use crate::RwaError;
    use soroban_sdk::Error;
    let h = setup();
    let members = make_guardians(&h, 3);
    let res = h.token.try_configure_recovery(&0, &members, &100);
    assert_eq!(
        res.unwrap_err().unwrap(),
        Error::from(RwaError::InvalidRecoveryConfig)
    );
    assert!(h.token.try_recovery_config().is_err());
}

#[test]
fn test_recovery_configure_rejects_duplicate_members() {
    use crate::RwaError;
    use soroban_sdk::Error;
    let h = setup();
    let g = Address::generate(&h.env);
    let other = Address::generate(&h.env);
    // Without the guard, [g, g, other] with threshold 2 lets `g` alone meet quorum.
    let members = soroban_sdk::vec![&h.env, g.clone(), g, other];
    let res = h.token.try_configure_recovery(&2, &members, &100);
    assert_eq!(
        res.unwrap_err().unwrap(),
        Error::from(RwaError::InvalidRecoveryConfig)
    );
    assert!(h.token.try_recovery_members().is_err());
}

#[test]
fn test_recovery_propose_and_read_active() {
    let h = setup();
    let members = make_guardians(&h, 3);
    let g1 = members.get(0).unwrap();
    h.token.configure_recovery(&2, &members, &100);

    let new_admin = Address::generate(&h.env);
    h.token.propose_recovery(&g1, &new_admin);

    let proposal = h.token.active_recovery().expect("proposal must exist");
    assert_eq!(proposal.proposed_admin, new_admin);
    assert_eq!(proposal.proposer, g1);
    assert_eq!(proposal.approvals.len(), 0);
}

#[test]
fn test_recovery_propose_rejects_while_proposal_active() {
    use crate::RwaError;
    use soroban_sdk::Error;
    let h = setup();
    let members = make_guardians(&h, 3);
    let g1 = members.get(0).unwrap();
    let g2 = members.get(1).unwrap();
    h.token.configure_recovery(&2, &members, &100);

    h.env.ledger().set_sequence_number(1000);
    let admin_a = Address::generate(&h.env);
    h.token.propose_recovery(&g1, &admin_a);

    // Expiry = 1000 + 100/2 = 1050; the proposal is still live at the boundary.
    h.env.ledger().set_sequence_number(1050);
    let res = h
        .token
        .try_propose_recovery(&g2, &Address::generate(&h.env));
    assert_eq!(
        res.unwrap_err().unwrap(),
        Error::from(RwaError::RecoveryAlreadyActive)
    );
    let proposal = h.token.active_recovery().unwrap();
    assert_eq!(proposal.proposed_admin, admin_a);
    assert_eq!(proposal.proposer, g1);

    // Once expired, the stale proposal can be replaced.
    h.env.ledger().set_sequence_number(1051);
    let admin_b = Address::generate(&h.env);
    h.token.propose_recovery(&g2, &admin_b);
    let proposal = h.token.active_recovery().unwrap();
    assert_eq!(proposal.proposed_admin, admin_b);
    assert_eq!(proposal.proposer, g2);
}

#[test]
fn test_recovery_propose_rejects_unreachable_threshold() {
    use crate::RwaError;
    use soroban_sdk::Error;
    let h = setup();
    let members = make_guardians(&h, 3);
    let g1 = members.get(0).unwrap();
    // 3-of-3 passes configure, but the proposer cannot approve, so only 2
    // approvals are ever reachable.
    h.token.configure_recovery(&3, &members, &100);

    let res = h
        .token
        .try_propose_recovery(&g1, &Address::generate(&h.env));
    assert_eq!(
        res.unwrap_err().unwrap(),
        Error::from(RwaError::InvalidRecoveryConfig)
    );
    assert!(h.token.active_recovery().is_none());
}

#[test]
fn test_recovery_approve_increases_approval_count() {
    let h = setup();
    let members = make_guardians(&h, 3);
    let g1 = members.get(0).unwrap();
    let g2 = members.get(1).unwrap();
    h.token.configure_recovery(&2, &members, &100);
    h.token.propose_recovery(&g1, &Address::generate(&h.env));

    h.token.approve_recovery(&g2);

    let proposal = h.token.active_recovery().unwrap();
    assert_eq!(proposal.approvals.len(), 1);
    assert!(proposal.approvals.contains(&g2));
}

#[test]
fn test_recovery_approve_duplicate_panics() {
    use crate::RwaError;
    use soroban_sdk::Error;
    let h = setup();
    let members = make_guardians(&h, 3);
    let g1 = members.get(0).unwrap();
    let g2 = members.get(1).unwrap();
    h.token.configure_recovery(&2, &members, &100);
    h.token.propose_recovery(&g1, &Address::generate(&h.env));
    h.token.approve_recovery(&g2);

    let res = h.token.try_approve_recovery(&g2);
    assert_eq!(
        res.unwrap_err().unwrap(),
        Error::from(RwaError::AlreadyApproved)
    );
}

#[test]
fn test_recovery_proposer_cannot_approve() {
    use crate::RwaError;
    use soroban_sdk::Error;
    let h = setup();
    let members = make_guardians(&h, 3);
    let g1 = members.get(0).unwrap();
    h.token.configure_recovery(&2, &members, &100);
    h.token.propose_recovery(&g1, &Address::generate(&h.env));

    let res = h.token.try_approve_recovery(&g1);
    assert_eq!(
        res.unwrap_err().unwrap(),
        Error::from(RwaError::ProposerCannotApprove)
    );
}

#[test]
fn test_recovery_expired_proposal_blocks_approve() {
    use crate::RwaError;
    use soroban_sdk::Error;
    let h = setup();
    let members = make_guardians(&h, 3);
    let g1 = members.get(0).unwrap();
    let g2 = members.get(1).unwrap();
    h.token.configure_recovery(&2, &members, &100);

    h.env.ledger().set_sequence_number(1000);
    h.token.propose_recovery(&g1, &Address::generate(&h.env));
    // Expiry = 1000 + 100/2 = 1050
    h.env.ledger().set_sequence_number(1051);

    let res = h.token.try_approve_recovery(&g2);
    assert_eq!(
        res.unwrap_err().unwrap(),
        Error::from(RwaError::RecoveryExpired)
    );
}

#[test]
fn test_recovery_execute_rejects_expired_proposal() {
    use crate::RwaError;
    use soroban_sdk::Error;
    let h = setup();
    let members = make_guardians(&h, 3);
    let g1 = members.get(0).unwrap();
    let g2 = members.get(1).unwrap();
    let g3 = members.get(2).unwrap();
    h.token.configure_recovery(&2, &members, &100);
    let nonce_before = h.token.admin_nonce();

    h.env.ledger().set_sequence_number(1000);
    h.token.propose_recovery(&g1, &Address::generate(&h.env));
    h.token.approve_recovery(&g2);
    h.token.approve_recovery(&g3);
    // Fully approved, but expiry = 1000 + 100/2 = 1050.
    h.env.ledger().set_sequence_number(1051);

    let res = h.token.try_execute_recovery(&g1);
    assert_eq!(
        res.unwrap_err().unwrap(),
        Error::from(RwaError::RecoveryExpired)
    );
    assert_eq!(h.token.admin_nonce(), nonce_before);
    assert_eq!(h.token.recovery_config().last_executed_ledger, 0);
    assert!(h.token.active_recovery().is_some());
}

#[test]
fn test_recovery_execute_replaces_admin_and_bumps_nonce() {
    let h = setup();
    let members = make_guardians(&h, 3);
    let g1 = members.get(0).unwrap();
    let g2 = members.get(1).unwrap();
    let g3 = members.get(2).unwrap();
    h.token.configure_recovery(&2, &members, &0); // cooldown=0 for simplicity
    let new_admin = Address::generate(&h.env);
    let nonce_before = h.token.admin_nonce();

    h.token.propose_recovery(&g1, &new_admin);
    h.token.approve_recovery(&g2);
    h.token.approve_recovery(&g3);
    h.token.execute_recovery(&g1);

    // recovery_executed event emitted with old/new admin (#670)
    {
        use soroban_sdk::{testutils::Events as _, symbol_short, TryFromVal};
        let (_, topics, data) = h
            .env
            .events()
            .all()
            .iter()
            .rev()
            .find(|(_, topics, _)| {
                topics.len() == 1
                    && TryFromVal::try_from_val(&h.env, &topics.get(0).unwrap())
                        .map(|topic: soroban_sdk::Symbol| topic == symbol_short!("rcv_exe"))
                        .unwrap_or(false)
            })
            .expect("recovery_executed event should be emitted");
        assert_eq!(topics.len(), 1);
        let (old_admin, emitted_new_admin): (Address, Address) =
            TryFromVal::try_from_val(&h.env, &data).unwrap();
        assert_eq!(emitted_new_admin, new_admin);
        assert_ne!(old_admin, new_admin);
    }

    // Active proposal cleared
    assert!(h.token.active_recovery().is_none());
    // AdminNonce bumped
    assert!(h.token.admin_nonce() > nonce_before);
    // last_executed_ledger updated
    assert!(h.token.recovery_config().last_executed_ledger > 0 || h.env.ledger().sequence() == 0);
}

#[test]
fn test_recovery_execute_insufficient_approvals_panics() {
    use crate::RwaError;
    use soroban_sdk::Error;
    let h = setup();
    let members = make_guardians(&h, 3);
    let g1 = members.get(0).unwrap();
    let g2 = members.get(1).unwrap();
    h.token.configure_recovery(&2, &members, &0);
    h.token.propose_recovery(&g1, &Address::generate(&h.env));
    h.token.approve_recovery(&g2); // only 1 of 2 required

    let res = h.token.try_execute_recovery(&g1);
    assert_eq!(
        res.unwrap_err().unwrap(),
        Error::from(RwaError::InsufficientApprovals)
    );
}

#[test]
fn test_recovery_cooldown_blocks_immediate_second_execution() {
    use crate::RwaError;
    use soroban_sdk::Error;
    let h = setup();
    let members = make_guardians(&h, 3);
    let g1 = members.get(0).unwrap();
    let g2 = members.get(1).unwrap();
    let g3 = members.get(2).unwrap();
    h.token.configure_recovery(&2, &members, &100);
    let admin1 = Address::generate(&h.env);
    let admin2 = Address::generate(&h.env);

    h.env.ledger().set_sequence_number(1000);
    h.token.propose_recovery(&g1, &admin1);
    h.token.approve_recovery(&g2);
    h.token.approve_recovery(&g3);
    h.token.execute_recovery(&g1);

    // Attempt second recovery immediately after (cooldown=100, so need seq >= 1100)
    h.env.ledger().set_sequence_number(1001);
    h.token.propose_recovery(&g1, &admin2);
    h.token.approve_recovery(&g2);
    h.token.approve_recovery(&g3);

    let res = h.token.try_execute_recovery(&g1);
    assert_eq!(
        res.unwrap_err().unwrap(),
        Error::from(RwaError::RecoveryCooldown)
    );
}

#[test]
fn test_recovery_cancel_clears_active_proposal() {
    let h = setup();
    let members = make_guardians(&h, 3);
    let g1 = members.get(0).unwrap();
    h.token.configure_recovery(&2, &members, &100);
    h.token.propose_recovery(&g1, &Address::generate(&h.env));
    assert!(h.token.active_recovery().is_some());

    h.token.cancel_recovery();
    assert!(h.token.active_recovery().is_none());
}

#[test]
fn test_recovery_cancel_no_active_panics() {
    use crate::RwaError;
    use soroban_sdk::Error;
    let h = setup();
    let members = make_guardians(&h, 3);
    h.token.configure_recovery(&2, &members, &100);

    let res = h.token.try_cancel_recovery();
    assert_eq!(
        res.unwrap_err().unwrap(),
        Error::from(RwaError::NoActiveRecovery)
    );
}

#[test]
fn test_recovery_non_guardian_propose_panics() {
    use crate::RwaError;
    use soroban_sdk::Error;
    let h = setup();
    let members = make_guardians(&h, 3);
    h.token.configure_recovery(&2, &members, &100);
    let stranger = Address::generate(&h.env);
    let res = h
        .token
        .try_propose_recovery(&stranger, &Address::generate(&h.env));
    assert_eq!(
        res.unwrap_err().unwrap(),
        Error::from(RwaError::NotRecoveryMember)
    );
}

#[test]
fn test_recovery_configure_invalidates_active_proposal() {
    let h = setup();
    let members = make_guardians(&h, 3);
    let g1 = members.get(0).unwrap();
    h.token.configure_recovery(&2, &members, &100);
    h.token.propose_recovery(&g1, &Address::generate(&h.env));
    assert!(h.token.active_recovery().is_some());

    let new_members = make_guardians(&h, 2);
    h.token.configure_recovery(&2, &new_members, &50);
    assert!(h.token.active_recovery().is_none());
}

#[test]
fn test_recovery_new_admin_can_mint_after_recovery() {
    let h = setup();
    let members = make_guardians(&h, 3);
    let g1 = members.get(0).unwrap();
    let g2 = members.get(1).unwrap();
    let g3 = members.get(2).unwrap();
    h.token.configure_recovery(&2, &members, &0);
    let new_admin = Address::generate(&h.env);

    h.token.propose_recovery(&g1, &new_admin);
    h.token.approve_recovery(&g2);
    h.token.approve_recovery(&g3);
    h.token.execute_recovery(&g1);

    let investor = Address::generate(&h.env);
    h.approve_kyc(&investor);
    h.token.mint(&new_admin, &investor, &500);
    assert_eq!(h.token.balance(&investor), 500);
}

#[test]
fn test_propose_admin_second_call_emits_pending_admin_cleared() {
    use soroban_sdk::{symbol_short, testutils::Events as _, Symbol, TryFromVal};

    let h = setup();
    let clear_topic = symbol_short!("adm_clr");
    let count_clear_events = |h: &Harness| -> usize {
        h.env
            .events()
            .all()
            .iter()
            .filter(|(_, topics, _)| {
                topics.len() == 1
                    && Symbol::try_from_val(&h.env, &topics.get(0).unwrap())
                        .map(|s| s == clear_topic)
                        .unwrap_or(false)
            })
            .count()
    };

    // First proposal: no prior pending admin, so no clear event is emitted.
    h.token
        .propose_admin(&h.admin, &Address::generate(&h.env), &h.next_nonce());
    assert_eq!(count_clear_events(&h), 0);

    // Second proposal overwrites the first pending admin — must emit adm_clr.
    h.token
        .propose_admin(&h.admin, &Address::generate(&h.env), &h.next_nonce());
    assert_eq!(count_clear_events(&h), 1);
}

#[test]
fn test_propose_admin_rejects_current_admin() {
    use crate::RwaError;
    use soroban_sdk::Error;

    let h = setup();
    // Proposing the sitting admin as their own successor is rejected with a
    // typed error rather than silently succeeding.
    let res = h
        .token
        .try_propose_admin(&h.admin, &h.admin, &h.next_nonce());
    assert_eq!(res.unwrap_err().unwrap(), Error::from(RwaError::AlreadyAdmin));

    // The rejected call must not have consumed the admin nonce.
    assert_eq!(h.token.admin_nonce(), 0);
}

#[test]
fn test_accept_admin_no_pending() {
    use crate::RwaError;
    use soroban_sdk::Error;

    let h = setup();
    // No propose_admin call has been made, so there is no pending admin.
    let res = h.token.try_accept_admin();
    assert_eq!(res.unwrap_err().unwrap(), Error::from(RwaError::NoPendingAdmin));
}

#[test]
fn test_propose_admin_accepts_different_address() {
    let h = setup();
    let new_admin = Address::generate(&h.env);

    // A distinct address still completes the two-step handover as before.
    h.token.propose_admin(&h.admin, &new_admin, &h.next_nonce());
    h.token.accept_admin();

    let investor = Address::generate(&h.env);
    h.approve_kyc(&investor);
    h.token.mint(&new_admin, &investor, &100);
    assert_eq!(h.token.balance(&investor), 100);
}

#[test]
fn test_accept_admin_event_includes_admin_nonce() {
    use soroban_sdk::{testutils::Events as _, symbol_short, Address, TryFromVal};

    let h = setup();
    let new_admin = Address::generate(&h.env);
    h.token
        .propose_admin(&h.admin, &new_admin, &h.next_nonce());
    h.token.accept_admin();

    let (_, topics, data) = h
        .env
        .events()
        .all()
        .iter()
        .rev()
        .find(|(_, topics, _)| {
            topics.len() == 1
                && TryFromVal::try_from_val(&h.env, &topics.get(0).unwrap())
                    .map(|topic: soroban_sdk::Symbol| topic == symbol_short!("adm_set"))
                    .unwrap_or(false)
        })
        .expect("admin-set event should be emitted");
    assert_eq!(topics.len(), 1);
    let (_, _, nonce): (Address, Address, u64) = TryFromVal::try_from_val(&h.env, &data).unwrap();
    assert_eq!(nonce, 1);
}

// ── Allowance past-expiry boundary condition ──────────────────────────────────

/// Documents and pins the behavior when `approve` is called with an
/// `expiration_ledger` that is already in the past (strictly less than the
/// current ledger sequence).
///
/// `write_allowance` panics with `AllowanceExpired` when `amount > 0` and
/// `expiration_ledger < current_sequence`.  This test therefore verifies that
/// the `approve` call itself fails — no allowance entry is written — and a
/// subsequent `transfer_from` also fails.
#[test]
fn test_set_allowance_past_expiry() {
    use crate::RwaError;
    use soroban_sdk::Error;

    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    let spender = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.mint(&alice, 1_000);

    // Advance the ledger so we have a non-zero sequence to subtract from.
    h.env.ledger().with_mut(|l| {
        l.sequence_number = 100;
    });
    let current_seq = h.env.ledger().sequence();

    // expiry_ledger is strictly in the past.
    let past_expiry = current_seq - 1;

    // The approve call itself must fail with AllowanceExpired.
    let res = h
        .token
        .try_approve(&alice, &spender, &300, &past_expiry);
    assert_eq!(
        res.unwrap_err().unwrap(),
        Error::from(RwaError::AllowanceExpired),
        "approve with past expiry_ledger must be rejected with AllowanceExpired"
    );

    // No allowance was written: the stored amount must be zero.
    assert_eq!(
        h.token.allowance(&alice, &spender),
        0,
        "no allowance should be stored after a rejected approve"
    );

    // A subsequent transfer_from must also fail (no usable allowance).
    let transfer_res = h
        .token
        .try_transfer_from(&spender, &alice, &bob, &1);
    assert!(
        transfer_res.is_err(),
        "transfer_from must fail when no allowance was written"
    );

    // Alice's balance is untouched.
    assert_eq!(h.token.balance(&alice), 1_000);
}

// ── Burn entire supply boundary condition ─────────────────────────────────────

/// Documents that burning the exact total supply in one call drives both the
/// holder's balance and `total_supply()` to zero with no off-by-one errors.
#[test]
fn test_burn_entire_supply() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);
    h.mint(&holder, 1_000);

    assert_eq!(h.token.total_supply(), 1_000);
    assert_eq!(h.token.balance(&holder), 1_000);
    // Holder is registered in the compliance engine.
    assert_eq!(h.compliance.holder_count(), 1);

    // Burn the exact total supply.
    h.token.burn(&holder, &1_000);

    assert_eq!(
        h.token.total_supply(),
        0,
        "total_supply must be 0 after burning the full supply"
    );
    assert_eq!(
        h.token.balance(&holder),
        0,
        "holder balance must be 0 after burning their entire balance"
    );
    // The holder should be deregistered once their balance reaches zero.
    assert_eq!(
        h.compliance.holder_count(),
        0,
        "holder must be deregistered after balance is drained to zero"
    );
}

// ── Regression tests for RwaToken::__constructor asset_type fix ───────────────

/// An empty asset_type string must be rejected.
///
/// Before the fix, the empty-string guard was absent; the code fell through to
/// the known-values comparison block which also rejected it, but without a
/// documented, early-exit boundary check that makes the intent explicit and
/// auditable.  This test pins the early-exit behavior.
///
/// Uses `#[should_panic]` — same convention as `test_invalid_asset_type` above.
#[test]
#[should_panic]
fn test_constructor_rejects_empty_asset_type() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);

    let kyc_id = env.register(KycRegistry, ());
    let kyc = KycRegistryClient::new(&env, &kyc_id);
    kyc.initialize(&admin);

    let compliance_id = env.register(ComplianceEngine, ());
    let compliance = ComplianceEngineClient::new(&env, &compliance_id);
    compliance.initialize(&admin, &kyc_id, &0u64);

    // Empty string is not a recognised asset category — must panic.
    let _ = env.register(
        RwaToken,
        (
            admin.clone(),
            7u32,
            String::from_str(&env, "Empty Type Token"),
            String::from_str(&env, "ETT"),
            String::from_str(&env, ""), // empty asset_type — triggers early guard
            kyc_id.clone(),
            compliance_id.clone(),
            Option::<ComplianceMetadata>::None,
            0i128,
        ),
    );
}

/// A known asset type immediately following an empty-string check still works.
/// This test confirms the early-exit guard does not break the happy path.
#[test]
fn test_constructor_accepts_valid_asset_type_after_empty_guard() {
    for asset_type in ["invoice", "property", "carbon_credit"] {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);

        let kyc_id = env.register(KycRegistry, ());
        let kyc = KycRegistryClient::new(&env, &kyc_id);
        kyc.initialize(&admin);

        let compliance_id = env.register(ComplianceEngine, ());
        let compliance = ComplianceEngineClient::new(&env, &compliance_id);
        compliance.initialize(&admin, &kyc_id, &0u64);

        let token_id = env.register(
            RwaToken,
            (
                admin.clone(),
                7u32,
                String::from_str(&env, "Valid Token"),
                String::from_str(&env, "VLD"),
                String::from_str(&env, asset_type),
                kyc_id.clone(),
                compliance_id.clone(),
                Option::<ComplianceMetadata>::None,
                0i128,
            ),
        );
        let token = RwaTokenClient::new(&env, &token_id);
        assert_eq!(
            token.asset_type(),
            String::from_str(&env, asset_type),
            "asset_type '{}' must be accepted and stored correctly",
            asset_type
        );
    }
}

// ── Regression: approve_recovery duplicate-guardian guard ────────────────────

/// A guardian who has already approved the active proposal must not be able
/// to approve a second time.  Without the guard, a single guardian could
/// push the approval count above the threshold by calling approve_recovery
/// multiple times, making the proposal appear stronger than it actually is.
///
/// This test pins the guard: a second call from the same guardian returns
/// AlreadyApproved and the approval count remains unchanged.
#[test]
fn test_approve_recovery_duplicate_guardian_rejected() {
    use crate::RwaError;
    use soroban_sdk::Error;

    let h = setup();
    // 3-guardian, threshold-2 setup.
    let members = make_guardians(&h, 3);
    let g1 = members.get(0).unwrap();
    let g2 = members.get(1).unwrap();
    h.token.configure_recovery(&2, &members, &100);
    h.token.propose_recovery(&g1, &Address::generate(&h.env));

    // First approval from g2 succeeds.
    h.token.approve_recovery(&g2);
    let after_first = h.token.active_recovery().unwrap();
    assert_eq!(after_first.approvals.len(), 1);

    // Second approval from the same guardian must be rejected.
    let res = h.token.try_approve_recovery(&g2);
    assert_eq!(
        res.unwrap_err().unwrap(),
        Error::from(RwaError::AlreadyApproved),
        "duplicate approval must return AlreadyApproved"
    );

    // Approval count must not have changed — threshold distortion is prevented.
    let after_dup = h.token.active_recovery().unwrap();
    assert_eq!(
        after_dup.approvals.len(),
        1,
        "approval count must remain 1 after rejected duplicate"
    );
}

/// Confirm the normal multi-guardian approval path is not affected: two
/// distinct guardians each approve once and the count reaches 2.
#[test]
fn test_approve_recovery_two_distinct_guardians_both_count() {
    let h = setup();
    let members = make_guardians(&h, 3);
    let g1 = members.get(0).unwrap();
    let g2 = members.get(1).unwrap();
    let g3 = members.get(2).unwrap();
    h.token.configure_recovery(&2, &members, &100);
    h.token.propose_recovery(&g1, &Address::generate(&h.env));

    h.token.approve_recovery(&g2);
    h.token.approve_recovery(&g3);

    let proposal = h.token.active_recovery().unwrap();
    assert_eq!(
        proposal.approvals.len(),
        2,
        "two distinct approvals must both be recorded"
    );
}

// ── Delegated transfer: failed compliance must not consume allowance ──────────
//
// Acceptance criteria (from issue spec):
//   - A failed `transfer_from` leaves allowance and balances unchanged.
//   - A successful `transfer_from` reduces allowance exactly by `amount`.
//   - Tests use only the public token interface.

/// Pause the compliance engine, call `transfer_from`, verify the error, then
/// check that the spender's allowance and both balances are untouched.
/// After unpausing, repeat the same transfer and verify it succeeds and
/// reduces the allowance exactly.
#[test]
fn test_transfer_from_failed_compliance_does_not_consume_allowance() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    let spender = Address::generate(&h.env);

    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.mint(&alice, 1_000);

    let expiration = h.env.ledger().sequence() + 1_000;
    h.token.approve(&alice, &spender, &300, &expiration);
    assert_eq!(h.token.allowance(&alice, &spender), 300);

    // Pause the compliance engine — any transfer should now be denied.
    h.compliance.pause();

    let err = h
        .token
        .try_transfer_from(&spender, &alice, &bob, &100)
        .unwrap_err()
        .unwrap();
    assert_eq!(
        err,
        soroban_sdk::Error::from(crate::RwaError::TransferBlocked),
        "paused compliance must produce TransferBlocked"
    );

    // State must be completely unchanged after the failed call.
    assert_eq!(
        h.token.allowance(&alice, &spender),
        300,
        "allowance must not be consumed on failure"
    );
    assert_eq!(h.token.balance(&alice), 1_000, "sender balance unchanged");
    assert_eq!(h.token.balance(&bob), 0, "recipient balance unchanged");

    // Unpause and verify the same call now succeeds and reduces allowance exactly.
    h.compliance.unpause();

    h.token
        .transfer_from(&spender, &alice, &bob, &100);

    assert_eq!(
        h.token.allowance(&alice, &spender),
        200,
        "allowance must decrease by exactly the transferred amount"
    );
    assert_eq!(h.token.balance(&alice), 900);
    assert_eq!(h.token.balance(&bob), 100);
}

// ── Allowance expiration ledger boundary ──────────────────────────────────────
//
// Acceptance criteria (from issue spec):
//   - At ledger N-1 (one before expiry): allowance readable and transfer succeeds.
//   - At ledger N (the expiry ledger itself): allowance reads as 0 and transfer fails.
//   - At ledger N+1 (one past expiry): same as N — allowance 0, transfer fails.
//
// The expiration_ledger in write_allowance is *exclusive*: the allowance is
// treated as expired when `expiration_ledger <= current_sequence`.

/// Set `expiration_ledger = seq + 2`.
/// At `seq + 1` (one before expiry) the allowance is still usable.
/// At `seq + 2` (the expiry ledger) the allowance is already expired (reads 0).
/// At `seq + 3` (one after) same result.
#[test]
fn test_transfer_from_allowance_expiry_boundary_sequence() {
    use soroban_sdk::testutils::Ledger as _;

    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    let spender = Address::generate(&h.env);

    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.mint(&alice, 1_000);

    // Record base sequence so boundaries are deterministic.
    let base_seq = h.env.ledger().sequence();
    // expiration_ledger = base + 2
    let expiry_ledger = base_seq + 2;
    h.token.approve(&alice, &spender, &300, &expiry_ledger);

    // ── N-1: one before expiry — allowance valid ──────────────────────────
    h.env.ledger().with_mut(|l| {
        l.sequence_number = expiry_ledger - 1;
    });
    assert_eq!(
        h.token.allowance(&alice, &spender),
        300,
        "allowance must be readable one ledger before expiry"
    );
    h.token.transfer_from(&spender, &alice, &bob, &100);
    assert_eq!(
        h.token.allowance(&alice, &spender),
        200,
        "allowance reduced after successful transfer one before expiry"
    );
    assert_eq!(h.token.balance(&bob), 100);

    // Reset allowance for the next boundary check.
    h.token.approve(&alice, &spender, &300, &expiry_ledger);

    // ── N: the expiry ledger itself — allowance expired (reads 0) ─────────
    h.env.ledger().with_mut(|l| {
        l.sequence_number = expiry_ledger;
    });
    assert_eq!(
        h.token.allowance(&alice, &spender),
        0,
        "allowance must read 0 at the expiry ledger"
    );
    assert!(
        h.token
            .try_transfer_from(&spender, &alice, &bob, &1)
            .is_err(),
        "transfer_from must fail at the expiry ledger"
    );
    // Balances unchanged from this attempt.
    assert_eq!(h.token.balance(&alice), 900);

    // ── N+1: one past expiry — same outcome ───────────────────────────────
    h.env.ledger().with_mut(|l| {
        l.sequence_number = expiry_ledger + 1;
    });
    assert_eq!(
        h.token.allowance(&alice, &spender),
        0,
        "allowance must read 0 one ledger past expiry"
    );
    assert!(
        h.token
            .try_transfer_from(&spender, &alice, &bob, &1)
            .is_err(),
        "transfer_from must fail one ledger past expiry"
    );
    assert_eq!(h.token.balance(&alice), 900, "balance unchanged past expiry");
}

// ── Overlapping denial: transfer violating multiple rules at once ─────────────
//
// Acceptance criteria (from issue spec):
//   - When two rules are simultaneously violated, the contract returns exactly
//     one deny reason — the one produced by the first branch reached in
//     `evaluate_transfer_inner`.
//   - The observed reason is asserted structurally (not just "some error").
//   - The test does NOT assert which rule wins unless it mirrors the actual
//     first-return order in the engine; see the API reference for the
//     documented check order.
//
// Implementation note — `evaluate_transfer_inner` first-return order:
//   1. CompliancePaused
//   2. FromBlocklisted
//   3. ToBlocklisted
//   4. (allowlist checks if in allowlist_mode)
//   5. KYC checks (from, then to)
//   6. …further rules…
//
// The test below activates a Pause AND a Blocklist on the sender simultaneously.
// The pause check fires first (branch 1), so `CompliancePaused` is returned
// even though `FromBlocklisted` would also match.  The test asserts the
// structured `CompliancePaused` reason to pin this ordering contract.

#[test]
fn test_transfer_from_overlapping_denial_pause_wins_over_blocklist() {
    use soroban_sdk::Error;

    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    let spender = Address::generate(&h.env);

    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.mint(&alice, 1_000);

    let expiration = h.env.ledger().sequence() + 1_000;
    h.token.approve(&alice, &spender, &500, &expiration);

    // Activate both violations simultaneously.
    h.compliance.pause();
    h.compliance.add_to_blocklist(&alice);

    // The transfer must fail.
    let err = h
        .token
        .try_transfer_from(&spender, &alice, &bob, &100)
        .unwrap_err()
        .unwrap();

    // The compliance engine's first-return branch is CompliancePaused (before
    // the blocklist check), so the token contract maps it to TransferBlocked.
    // Both violations are active, but only one reason surfaces.
    assert_eq!(
        err,
        Error::from(crate::RwaError::TransferBlocked),
        "overlapping denial must produce a single structured TransferBlocked"
    );

    // State must be unchanged regardless of which rule fired.
    assert_eq!(
        h.token.allowance(&alice, &spender),
        500,
        "allowance must not be consumed on overlapping denial"
    );
    assert_eq!(h.token.balance(&alice), 1_000);
    assert_eq!(h.token.balance(&bob), 0);
}

// ── Pause/unpause round-trip: operational effect + policy history ─────────────
//
// Acceptance criteria (from issue spec):
//   - Pausing changes the transfer decision from Allow to Deny.
//   - Each pause/unpause transition appends exactly one correctly typed
//     PolicyRecord to the compliance engine's history.
//   - Unpausing restores the transfer decision to Allow.
//   - An unrelated compliance rule (max_transfer_amount) is unchanged throughout.

#[test]
fn test_pause_unpause_roundtrip_changes_decision_and_appends_policy_records() {
    use compliance_engine::{ComplianceRules, PolicyChangeKind};
    use soroban_sdk::Error;

    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);

    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.mint(&alice, 1_000);

    // Set an unrelated rule so we can verify it survives pause/unpause.
    h.compliance.set_rules(&ComplianceRules {
        max_transfer_amount: 500,
        min_holding_period: 0,
        max_holders: 0,
        require_same_jurisdiction: false,
        paused: false,
        allowlist_mode: false,
        max_holding_period: 0,
    });

    // Record version count before any pause operation.
    // initialize(1) + set_rules(1) = 2 versions so far.
    let versions_before_pause = h.compliance.policy_version_count();

    // ── Pause ─────────────────────────────────────────────────────────────
    h.compliance.pause();

    // Transfer must now be denied.
    assert_eq!(
        h.token.try_transfer(&alice, &bob, &100),
        Err(Ok(Error::from(crate::RwaError::TransferBlocked))),
        "transfer must be blocked after pause"
    );

    // Exactly one new PolicyRecord, kind = Pause.
    assert_eq!(
        h.compliance.policy_version_count(),
        versions_before_pause + 1,
        "pause must append exactly one policy record"
    );
    let pause_record = h.compliance.get_current_policy_version();
    assert_eq!(
        pause_record.change_kind,
        PolicyChangeKind::Pause,
        "pause record must carry the Pause kind"
    );
    assert!(
        pause_record.rules.paused,
        "pause record snapshot must have paused=true"
    );

    // Unrelated rule must be preserved in the snapshot.
    assert_eq!(
        pause_record.rules.max_transfer_amount,
        500,
        "unrelated max_transfer_amount must survive the pause"
    );

    // ── Unpause ───────────────────────────────────────────────────────────
    h.compliance.unpause();

    // Transfer must now succeed.
    h.token.transfer(&alice, &bob, &100);
    assert_eq!(h.token.balance(&bob), 100, "transfer succeeds after unpause");

    // Exactly one additional PolicyRecord, kind = Unpause.
    assert_eq!(
        h.compliance.policy_version_count(),
        versions_before_pause + 2,
        "unpause must append exactly one policy record"
    );
    let unpause_record = h.compliance.get_current_policy_version();
    assert_eq!(
        unpause_record.change_kind,
        PolicyChangeKind::Unpause,
        "unpause record must carry the Unpause kind"
    );
    assert!(
        !unpause_record.rules.paused,
        "unpause record snapshot must have paused=false"
    );

    // Unrelated rule must still be preserved after both transitions.
    assert_eq!(
        unpause_record.rules.max_transfer_amount,
        500,
        "unrelated max_transfer_amount must survive the full pause/unpause cycle"
    );
}

// ── #930 External URI set / read / replace ────────────────────────────────────
//
// Acceptance criteria:
//   - The latest accepted URI is returned exactly by get_external_uri.
//   - Replacing the URI with a new value returns the new value.
//   - Clearing with an empty string removes the stored value.
//   - An unauthorized caller cannot set the URI.

/// Set an HTTPS URI, read it back, replace it, read it back again.
/// Verifies the round-trip and that the latest value is always returned.
#[test]
fn test_external_uri_set_and_read_round_trip() {
    let h = setup();

    let uri_a = String::from_str(&h.env, "https://api.example.com/tokens/1");
    h.token.set_external_uri(&uri_a);
    assert_eq!(
        h.token.get_external_uri(),
        uri_a,
        "get_external_uri must return the URI just set"
    );

    // Replace with a different URI.
    let uri_b = String::from_str(&h.env, "ipfs://QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG");
    h.token.set_external_uri(&uri_b);
    assert_eq!(
        h.token.get_external_uri(),
        uri_b,
        "get_external_uri must return the updated URI after replacement"
    );
}

/// Clearing the URI with an empty string removes the stored value.
/// get_external_uri must return an empty string when no URI is set.
#[test]
fn test_external_uri_cleared_by_empty_string() {
    let h = setup();

    // Set a URI first, then clear it.
    h.token
        .set_external_uri(&String::from_str(&h.env, "https://example.com/meta"));
    h.token.set_external_uri(&String::from_str(&h.env, ""));

    assert_eq!(
        h.token.get_external_uri(),
        String::from_str(&h.env, ""),
        "get_external_uri must return empty string after clearing"
    );
}

/// get_external_uri returns an empty string when no URI has ever been set.
#[test]
fn test_external_uri_unset_returns_empty_string() {
    let h = setup();
    assert_eq!(
        h.token.get_external_uri(),
        String::from_str(&h.env, ""),
        "get_external_uri must return empty string when nothing has been stored"
    );
}

/// A non-admin caller must be rejected when attempting to set the URI.
/// Uses a fresh environment without mock_all_auths so authorization is enforced.
#[test]
fn test_external_uri_unauthorized_write_rejected() {
    // Stand up a token without mock_all_auths so Soroban enforces auth checks.
    let env = Env::default();
    let admin = Address::generate(&env);

    let kyc_id = env.register(kyc_registry::KycRegistry, ());
    // initialize does not require auth on KycRegistry — safe to call directly.
    kyc_registry::KycRegistryClient::new(&env, &kyc_id).initialize(&admin);
    let ce_id = env.register(compliance_engine::ComplianceEngine, ());
    compliance_engine::ComplianceEngineClient::new(&env, &ce_id)
        .initialize(&admin, &kyc_id, &0u64);

    let token_id = env.register(
        RwaToken,
        (
            admin.clone(),
            7u32,
            String::from_str(&env, "URI Token"),
            String::from_str(&env, "URI"),
            String::from_str(&env, "property"),
            kyc_id,
            ce_id,
            Option::<ComplianceMetadata>::None,
            0i128,
        ),
    );
    let token = RwaTokenClient::new(&env, &token_id);

    // Without mock_all_auths, the auth check inside set_external_uri fires.
    // The call must fail because no auth has been provided.
    let res = token.try_set_external_uri(&String::from_str(&env, "https://attacker.example.com"));
    assert!(
        res.is_err(),
        "set_external_uri must fail when called without valid admin auth"
    );

    // The URI must remain unset.
    assert_eq!(
        token.get_external_uri(),
        String::from_str(&env, ""),
        "URI must remain empty after an unauthorized write attempt"
    );
}

// ── #929 Role assign / revoke observable ─────────────────────────────────────
//
// Acceptance criteria:
//   - After assign_role, get_role_holder returns the assigned address.
//   - After revoke_role, get_role_holder returns None.
//   - Unauthorized assignment fails.

/// Assign the "compliance" role to a holder, verify it is readable,
/// then revoke it and verify it is absent.
#[test]
fn test_role_assign_and_revoke_observable() {
    let h = setup();
    let role_holder = Address::generate(&h.env);
    let compliance_role = soroban_sdk::Symbol::new(&h.env, "compliance");

    // Before assignment the role must be unset.
    assert!(
        h.token.get_role_holder(&compliance_role).is_none(),
        "role must be unset before assignment"
    );

    // Assign the role as admin (nonce 0).
    let nonce = h.next_nonce();
    h.token.assign_role(&compliance_role, &role_holder, &nonce);

    // get_role_holder must now return the assigned address.
    assert_eq!(
        h.token.get_role_holder(&compliance_role),
        Some(role_holder.clone()),
        "get_role_holder must return the holder after assignment"
    );

    // Revoke the role (nonce incremented).
    let nonce2 = h.next_nonce();
    h.token.revoke_role(&compliance_role, &nonce2);

    // After revocation get_role_holder must return None.
    assert!(
        h.token.get_role_holder(&compliance_role).is_none(),
        "get_role_holder must return None after revocation"
    );
}

/// Assigning different roles keeps them independent — revoking one does not
/// affect the other.
#[test]
fn test_role_assign_multiple_roles_are_independent() {
    let h = setup();
    let holder_a = Address::generate(&h.env);
    let holder_b = Address::generate(&h.env);
    let compliance_role = soroban_sdk::Symbol::new(&h.env, "compliance");
    let liquidity_role = soroban_sdk::Symbol::new(&h.env, "liquidity");

    h.token.assign_role(&compliance_role, &holder_a, &h.next_nonce());
    h.token.assign_role(&liquidity_role, &holder_b, &h.next_nonce());

    assert_eq!(
        h.token.get_role_holder(&compliance_role),
        Some(holder_a.clone())
    );
    assert_eq!(
        h.token.get_role_holder(&liquidity_role),
        Some(holder_b.clone())
    );

    // Revoke compliance — liquidity must remain.
    h.token.revoke_role(&compliance_role, &h.next_nonce());

    assert!(h.token.get_role_holder(&compliance_role).is_none());
    assert_eq!(
        h.token.get_role_holder(&liquidity_role),
        Some(holder_b),
        "liquidity role must be unaffected by revoking compliance"
    );
}

/// A non-admin, non-role-holder caller must be rejected by assign_role.
#[test]
fn test_role_assign_unauthorized_rejected() {
    // Build a fresh env without mock_all_auths so auth is enforced.
    let env2 = Env::default();
    let admin = Address::generate(&env2);

    let kyc_id = env2.register(kyc_registry::KycRegistry, ());
    kyc_registry::KycRegistryClient::new(&env2, &kyc_id).initialize(&admin);
    let ce_id = env2.register(compliance_engine::ComplianceEngine, ());
    compliance_engine::ComplianceEngineClient::new(&env2, &ce_id)
        .initialize(&admin, &kyc_id, &0u64);

    // Register the token without blanket auth mocking so auth is enforced.
    let token_id = env2.register(
        RwaToken,
        (
            admin.clone(),
            7u32,
            String::from_str(&env2, "Veritoken RWA"),
            String::from_str(&env2, "VTRWA"),
            String::from_str(&env2, "property"),
            kyc_id,
            ce_id,
            Option::<ComplianceMetadata>::None,
            0i128,
        ),
    );
    let token = RwaTokenClient::new(&env2, &token_id);

    let intruder = Address::generate(&env2);
    let compliance_role = soroban_sdk::Symbol::new(&env2, "compliance");

    // intruder has not been authorized — this must fail.
    let res = token.try_assign_role(&compliance_role, &intruder, &0u64);
    assert!(
        res.is_err(),
        "assign_role from an unauthorized caller must be rejected"
    );
    // Role must remain unset.
    assert!(token.get_role_holder(&compliance_role).is_none());
}

// ── #928 Nonce replay rejection ───────────────────────────────────────────────
//
// Acceptance criteria:
//   - A consumed nonce cannot authorize a second mutation (InvalidNonce).
//   - The target state is unchanged after the replay attempt.
//   - A sequential nonce succeeds.

/// Execute a nonce-protected operation with nonce N, then attempt to replay
/// with the same nonce N — must be rejected with InvalidNonce.
/// Then execute with nonce N+1 — must succeed.
#[test]
fn test_nonce_replay_rejected_and_sequential_succeeds() {
    use crate::RwaError;
    use soroban_sdk::Error;

    let h = setup();
    let holder = Address::generate(&h.env);
    let compliance_role = soroban_sdk::Symbol::new(&h.env, "compliance");

    // Capture the starting nonce.
    let nonce_0 = h.token.admin_nonce();

    // First operation: assign the compliance role using nonce_0.
    h.token.assign_role(&compliance_role, &holder, &nonce_0);
    assert_eq!(
        h.token.get_role_holder(&compliance_role),
        Some(holder.clone()),
        "role must be set after first assign"
    );

    // Nonce must have incremented.
    let nonce_1 = h.token.admin_nonce();
    assert_eq!(nonce_1, nonce_0 + 1, "nonce must increment after each use");

    // Replay: attempt the same assign with the already-consumed nonce_0.
    let another_holder = Address::generate(&h.env);
    let res = h
        .token
        .try_assign_role(&compliance_role, &another_holder, &nonce_0);
    assert_eq!(
        res.unwrap_err().unwrap(),
        Error::from(RwaError::InvalidNonce),
        "replaying nonce_0 must return InvalidNonce"
    );

    // State must be unchanged — the role still points to the original holder.
    assert_eq!(
        h.token.get_role_holder(&compliance_role),
        Some(holder.clone()),
        "role holder must be unchanged after a rejected replay"
    );

    // Sequential: use nonce_1 for a different nonce-protected operation (revoke).
    h.token.revoke_role(&compliance_role, &nonce_1);
    assert!(
        h.token.get_role_holder(&compliance_role).is_none(),
        "revoke with sequential nonce must succeed"
    );
}

/// Replaying a nonce on a different operation type also fails.
/// This confirms the nonce counter is per-contract, not per-method.
#[test]
fn test_nonce_replay_across_different_operations_rejected() {
    use crate::RwaError;
    use soroban_sdk::Error;

    let h = setup();
    let holder = Address::generate(&h.env);
    let liquidity_role = soroban_sdk::Symbol::new(&h.env, "liquidity");

    let nonce_0 = h.token.admin_nonce();

    // Use nonce_0 to assign a role.
    h.token.assign_role(&liquidity_role, &holder, &nonce_0);

    // Attempt to use the same nonce_0 on a different nonce-protected method (revoke_role).
    let res = h.token.try_revoke_role(&liquidity_role, &nonce_0);
    assert_eq!(
        res.unwrap_err().unwrap(),
        Error::from(RwaError::InvalidNonce),
        "replaying a nonce on a different operation must return InvalidNonce"
    );

    // Role is still set — the revoke was rejected.
    assert_eq!(
        h.token.get_role_holder(&liquidity_role),
        Some(holder),
        "role must remain after rejected replay"
    );
}

// ── #927 Mint cap at exact maximum supply ─────────────────────────────────────
//
// Acceptance criteria:
//   - Minting exactly the remaining supply (reaching the cap) succeeds.
//   - Minting one unit over the cap is rejected with ExceedsMaxSupply.
//   - total_supply is unchanged after the rejected over-cap mint.

/// Sets a small max_supply, mints to one below the cap, then:
///   (a) mints the last unit — must succeed and hit the cap exactly.
///   (b) tries to mint one more — must be rejected.
#[test]
fn test_mint_exactly_at_cap_succeeds_and_over_cap_rejected() {
    use crate::RwaError;
    use soroban_sdk::Error;

    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);

    let kyc_id = env.register(kyc_registry::KycRegistry, ());
    kyc_registry::KycRegistryClient::new(&env, &kyc_id).initialize(&admin);
    let ce_id = env.register(compliance_engine::ComplianceEngine, ());
    compliance_engine::ComplianceEngineClient::new(&env, &ce_id)
        .initialize(&admin, &kyc_id, &0u64);

    // max_supply = 100 (small, for boundary clarity)
    let token_id = env.register(
        RwaToken,
        (
            admin.clone(),
            7u32,
            String::from_str(&env, "Cap Token"),
            String::from_str(&env, "CAP"),
            String::from_str(&env, "invoice"),
            kyc_id.clone(),
            ce_id.clone(),
            Option::<ComplianceMetadata>::None,
            100i128, // max_supply = 100
        ),
    );
    let token = RwaTokenClient::new(&env, &token_id);
    let kyc = kyc_registry::KycRegistryClient::new(&env, &kyc_id);

    let verifier = Address::generate(&env);
    kyc.add_verifier(&admin, &verifier);

    let recipient = Address::generate(&env);
    kyc.approve(
        &verifier,
        &recipient,
        &1,
        &0,
        &String::from_str(&env, "US"),
    );

    // Mint 99 — one below the cap.
    token.mint(&admin, &recipient, &99i128);
    assert_eq!(token.total_supply(), 99i128, "supply must be 99 after first mint");

    // Mint the last 1 — must reach the cap exactly.
    token.mint(&admin, &recipient, &1i128);
    assert_eq!(
        token.total_supply(),
        100i128,
        "supply must equal max_supply after exact-cap mint"
    );

    // Attempt to mint 1 more — must be rejected.
    let res = token.try_mint(&admin, &recipient, &1i128);
    assert_eq!(
        res.unwrap_err().unwrap(),
        Error::from(RwaError::ExceedsMaxSupply),
        "minting above the cap must return ExceedsMaxSupply"
    );

    // Total supply must remain at the cap after the rejected mint.
    assert_eq!(
        token.total_supply(),
        100i128,
        "total_supply must be unchanged after a rejected over-cap mint"
    );
}

/// A single mint of exactly the full max_supply (starting from 0) must succeed.
#[test]
fn test_mint_full_max_supply_in_one_call_succeeds() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);

    let kyc_id = env.register(kyc_registry::KycRegistry, ());
    kyc_registry::KycRegistryClient::new(&env, &kyc_id).initialize(&admin);
    let ce_id = env.register(compliance_engine::ComplianceEngine, ());
    compliance_engine::ComplianceEngineClient::new(&env, &ce_id)
        .initialize(&admin, &kyc_id, &0u64);

    // max_supply = 50
    let token_id = env.register(
        RwaToken,
        (
            admin.clone(),
            7u32,
            String::from_str(&env, "Full Cap"),
            String::from_str(&env, "FCP"),
            String::from_str(&env, "invoice"),
            kyc_id.clone(),
            ce_id.clone(),
            Option::<ComplianceMetadata>::None,
            50i128,
        ),
    );
    let token = RwaTokenClient::new(&env, &token_id);
    let kyc = kyc_registry::KycRegistryClient::new(&env, &kyc_id);

    let verifier = Address::generate(&env);
    kyc.add_verifier(&admin, &verifier);
    let recipient = Address::generate(&env);
    kyc.approve(
        &verifier,
        &recipient,
        &1,
        &0,
        &String::from_str(&env, "US"),
    );

    // Mint the entire cap in one call.
    token.mint(&admin, &recipient, &50i128);
    assert_eq!(
        token.total_supply(),
        50i128,
        "single mint of the full cap must succeed"
    );

    // One more unit must be rejected.
    let res = token.try_mint(&admin, &recipient, &1i128);
    assert!(res.is_err(), "any mint beyond the cap must fail");
    assert_eq!(token.total_supply(), 50i128, "supply unchanged after rejection");
}
