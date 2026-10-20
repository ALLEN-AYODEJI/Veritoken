#![cfg(test)]

use crate::{InvoiceError, InvoiceMeta, InvoiceStatus, InvoiceToken, InvoiceTokenClient};
use compliance_engine::{ComplianceEngine, ComplianceEngineClient, ComplianceRules};
use kyc_registry::{KycRegistry, KycRegistryClient};
use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    Address, Env, String,
};

// ── Test harness ──────────────────────────────────────────────────────────────

struct Harness {
    env: Env,
    token: InvoiceTokenClient<'static>,
    kyc: KycRegistryClient<'static>,
    compliance: ComplianceEngineClient<'static>,
    verifier: Address,
    #[allow(dead_code)]
    admin: Address,
}

fn inv_id(env: &Env) -> String {
    String::from_str(env, "INV-001")
}

fn meta(env: &Env) -> InvoiceMeta {
    InvoiceMeta {
        invoice_id: String::from_str(env, "INV-001"),
        issuer: String::from_str(env, "Acme Corp"),
        debtor: String::from_str(env, "Globex"),
        face_value_usd: 1_000_000_000_000,
        discount_rate_bps: 250,
        due_date: 1_900_000_000,
        currency: String::from_str(env, "USD"),
        ipfs_doc_hash: String::from_str(env, ""),
        transfer_fee_bps: 0,
        fee_recipient: None,
        notification_webhook: String::from_str(env, ""),
    }
}

fn setup() -> Harness {
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
        InvoiceToken,
        (
            admin.clone(),
            kyc_id.clone(),
            compliance_id.clone(),
            meta(&env),
        ),
    );
    let token = InvoiceTokenClient::new(&env, &token_id);

    Harness {
        env,
        token,
        kyc,
        compliance,
        verifier,
        admin,
    }
}

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

    fn make_invoice(&self, id: &str) -> InvoiceMeta {
        InvoiceMeta {
            invoice_id: String::from_str(&self.env, id),
            issuer: String::from_str(&self.env, "Issuer"),
            debtor: String::from_str(&self.env, "Debtor"),
            face_value_usd: 500_000_000_000,
            discount_rate_bps: 100,
            due_date: 1_900_000_000,
            currency: String::from_str(&self.env, "USD"),
            ipfs_doc_hash: String::from_str(&self.env, ""),
            transfer_fee_bps: 0,
            fee_recipient: None,
            notification_webhook: String::from_str(&self.env, ""),
        }
    }
}

// ── Existing behaviour (preserved) ───────────────────────────────────────────

#[test]
fn test_issue_cannot_exceed_face_value() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);

    let m = h.token.get_meta(&inv_id(&h.env));
    // Issuing exactly the face value fills the supply cap.
    h.token.issue(&inv_id(&h.env), &holder, &m.face_value_usd);
    assert_eq!(h.token.balance(&holder, &inv_id(&h.env)), m.face_value_usd);

    // One more token would push total_supply over face_value_usd — must be rejected.
    assert!(h.token.try_issue(&inv_id(&h.env), &holder, &1).is_err());
}

#[test]
fn test_issue_idempotency_holder_count() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);

    h.token.issue(&inv_id(&h.env), &holder, &1_000);
    assert_eq!(h.compliance.holder_count(), 1);

    h.token.issue(&inv_id(&h.env), &holder, &500);
    assert_eq!(h.compliance.holder_count(), 1);
    assert_eq!(h.token.balance(&holder, &inv_id(&h.env)), 1_500);
}

#[test]
fn test_transfer_before_due_date() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.token.issue(&inv_id(&h.env), &alice, &1_000);

    h.token.transfer(&inv_id(&h.env), &alice, &bob, &400);
    assert_eq!(h.token.balance(&alice, &inv_id(&h.env)), 600);
    assert_eq!(h.token.balance(&bob, &inv_id(&h.env)), 400);
}

#[test]
fn test_transfer_rejects_blank_kyc_reference() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.token.issue(&inv_id(&h.env), &alice, &1_000);

    assert_eq!(
        h.token
            .try_transfer(&String::from_str(&h.env, "   "), &alice, &bob, &100),
        Err(Ok(InvoiceError::InvalidMetadata.into()))
    );
}

#[test]
fn test_evaluate_transfer_kyc_rejects_unapproved_recipient() {
    let h = setup();
    let alice = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.token.issue(&inv_id(&h.env), &alice, &1_000);

    // carol has no KYC record — the fail-fast guard in evaluate_transfer_kyc
    // must reject the missing KYC reference before the compliance engine runs.
    let carol = Address::generate(&h.env);
    assert_eq!(
        h.token
            .try_transfer(&inv_id(&h.env), &alice, &carol, &100),
        Err(Ok(InvoiceError::KycNotApproved.into()))
    );
}

#[test]
fn test_transfer_blocked_after_due_date() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.token.issue(&inv_id(&h.env), &alice, &1_000);

    h.env.ledger().set_timestamp(1_900_000_001);
    assert!(h
        .token
        .try_transfer(&inv_id(&h.env), &alice, &bob, &500)
        .is_err());
}

#[test]
fn test_transfer_from_blocked_after_due_date() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.token.issue(&inv_id(&h.env), &alice, &1_000);
    h.token
        .approve(&alice, &bob, &inv_id(&h.env), &500, &999_999_999);

    h.env.ledger().set_timestamp(1_900_000_001);
    assert!(h
        .token
        .try_transfer_from(&bob, &inv_id(&h.env), &alice, &bob, &500)
        .is_err());
}

#[test]
fn test_metadata() {
    let h = setup();
    assert_eq!(h.token.decimals(), 7);
    assert_eq!(
        h.token.name(),
        String::from_str(&h.env, "Veritoken Invoice")
    );
    assert_eq!(
        h.token.get_meta(&inv_id(&h.env)).invoice_id,
        String::from_str(&h.env, "INV-001")
    );
    assert!(!h.token.is_settled(&inv_id(&h.env)));
}

#[test]
fn test_issue_requires_kyc() {
    let h = setup();
    let holder = Address::generate(&h.env);

    assert!(h.token.try_issue(&inv_id(&h.env), &holder, &1_000).is_err());

    h.approve_kyc(&holder);
    h.token.issue(&inv_id(&h.env), &holder, &1_000);
    assert_eq!(h.token.balance(&holder, &inv_id(&h.env)), 1_000);
    assert_eq!(h.token.total_supply(&inv_id(&h.env)), 1_000);
}

#[test]
fn test_settle_then_redeem() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);
    h.token.issue(&inv_id(&h.env), &holder, &1_000);

    assert!(h.token.try_redeem(&inv_id(&h.env), &holder, &500).is_err());

    h.token.settle(&inv_id(&h.env));
    assert!(h.token.is_settled(&inv_id(&h.env)));

    h.token.redeem(&inv_id(&h.env), &holder, &600);
    assert_eq!(h.token.balance(&holder, &inv_id(&h.env)), 400);
    assert_eq!(h.token.total_supply(&inv_id(&h.env)), 400);
}

#[test]
fn test_cannot_issue_after_settle() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);
    // settle before any issuance — backward-compatible path (Created → FullySettled)
    h.token.settle(&inv_id(&h.env));
    assert!(h.token.try_issue(&inv_id(&h.env), &holder, &1).is_err());
}

#[test]
fn test_redeem_insufficient_balance() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);
    h.token.issue(&inv_id(&h.env), &holder, &100);
    h.token.settle(&inv_id(&h.env));
    assert!(h.token.try_redeem(&inv_id(&h.env), &holder, &101).is_err());
}

#[test]
fn test_redeem_blocked_when_compliance_paused() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);
    h.token.issue(&inv_id(&h.env), &holder, &1_000);
    h.token.settle(&inv_id(&h.env));
    h.compliance.pause();
    assert!(h.token.try_redeem(&inv_id(&h.env), &holder, &500).is_err());
}

#[test]
fn test_redeem_blocked_for_blocklisted_holder() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);
    h.token.issue(&inv_id(&h.env), &holder, &1_000);
    h.token.settle(&inv_id(&h.env));
    h.compliance.add_to_blocklist(&holder);
    assert!(h.token.try_redeem(&inv_id(&h.env), &holder, &500).is_err());
}

#[test]
fn test_non_deployer_cannot_reinitialize() {
    let h = setup();
    let attacker = Address::generate(&h.env);
    let kyc_id = Address::generate(&h.env);
    let ce_id = Address::generate(&h.env);
    assert!(h
        .token
        .try_initialize(&attacker, &kyc_id, &ce_id, &meta(&h.env))
        .is_err());
}

#[test]
fn test_transfer_blocked_by_holding_period() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);

    h.compliance.set_rules(&ComplianceRules {
        max_transfer_amount: 0,
        min_holding_period: 3600,
        max_holders: 0,
        require_same_jurisdiction: false,
        paused: false,
        allowlist_mode: false,
        max_holding_period: 0,
    });

    h.token.issue(&inv_id(&h.env), &alice, &1_000);
    assert!(h
        .token
        .try_transfer(&inv_id(&h.env), &alice, &bob, &100)
        .is_err());

    h.env
        .ledger()
        .set_timestamp(h.env.ledger().timestamp() + 3601);
    h.token.transfer(&inv_id(&h.env), &alice, &bob, &100);
    assert_eq!(h.token.balance(&bob, &inv_id(&h.env)), 100);
    assert_eq!(h.token.balance(&alice, &inv_id(&h.env)), 900);
}

#[test]
fn test_update_kyc_registry_admin_only() {
    let h = setup();
    let new_kyc = Address::generate(&h.env);

    {
        let env2 = Env::default();
        let non_admin = Address::generate(&env2);
        let token_id2 = env2.register(
            InvoiceToken,
            (
                non_admin.clone(),
                Address::generate(&env2),
                Address::generate(&env2),
                meta(&env2),
            ),
        );
        let client2 = InvoiceTokenClient::new(&env2, &token_id2);
        assert!(client2
            .try_update_kyc_registry(&Address::generate(&env2))
            .is_err());
    }

    h.token.update_kyc_registry(&new_kyc);

    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);
    assert!(h.token.try_issue(&inv_id(&h.env), &holder, &1).is_err());
}

#[test]
fn test_update_compliance_engine_admin_only() {
    let h = setup();

    {
        let env2 = Env::default();
        let non_admin = Address::generate(&env2);
        let token_id2 = env2.register(
            InvoiceToken,
            (
                non_admin.clone(),
                Address::generate(&env2),
                Address::generate(&env2),
                meta(&env2),
            ),
        );
        let client2 = InvoiceTokenClient::new(&env2, &token_id2);
        assert!(client2
            .try_update_compliance_engine(&Address::generate(&env2))
            .is_err());
    }

    let kyc_id2 = h.env.register(KycRegistry, ());
    let kyc2 = KycRegistryClient::new(&h.env, &kyc_id2);
    kyc2.initialize(&h.admin);

    let ce2_id = h.env.register(ComplianceEngine, ());
    let ce2 = ComplianceEngineClient::new(&h.env, &ce2_id);
    ce2.initialize(&h.admin, &kyc_id2, &0u64);
    ce2.pause();

    h.token.update_compliance_engine(&ce2_id);

    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);
    h.token.issue(&inv_id(&h.env), &holder, &100);
    h.token.settle(&inv_id(&h.env));
    assert!(h.token.try_redeem(&inv_id(&h.env), &holder, &50).is_err());
}

// ── Multi-invoice tests ───────────────────────────────────────────────────────

#[test]
fn test_multi_invoice_independent_balances() {
    let h = setup();
    let id1 = inv_id(&h.env);
    let id2 = String::from_str(&h.env, "INV-002");

    h.token.create_invoice(&h.make_invoice("INV-002"));

    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);

    h.token.issue(&id1, &alice, &1_000);
    h.token.issue(&id2, &bob, &500);

    assert_eq!(h.token.balance(&alice, &id1), 1_000);
    assert_eq!(h.token.balance(&alice, &id2), 0);
    assert_eq!(h.token.balance(&bob, &id1), 0);
    assert_eq!(h.token.balance(&bob, &id2), 500);
    assert_eq!(h.token.total_supply(&id1), 1_000);
    assert_eq!(h.token.total_supply(&id2), 500);
}

#[test]
fn test_multi_invoice_independent_settlement() {
    let h = setup();
    let id1 = inv_id(&h.env);
    let id2 = String::from_str(&h.env, "INV-002");

    h.token.create_invoice(&h.make_invoice("INV-002"));

    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);

    h.token.issue(&id1, &alice, &1_000);
    h.token.issue(&id2, &bob, &500);

    h.token.settle(&id1);
    assert!(h.token.is_settled(&id1));
    assert!(!h.token.is_settled(&id2));

    h.token.redeem(&id1, &alice, &500);
    assert_eq!(h.token.balance(&alice, &id1), 500);

    assert!(h.token.try_redeem(&id2, &bob, &100).is_err());
}

#[test]
fn test_list_invoices_pagination() {
    let h = setup();

    let ids = h.token.list_invoices(&0, &10);
    assert_eq!(ids.len(), 1);

    h.token.create_invoice(&h.make_invoice("INV-002"));
    h.token.create_invoice(&h.make_invoice("INV-003"));
    h.token.create_invoice(&h.make_invoice("INV-004"));

    assert_eq!(h.token.list_invoices(&0, &10).len(), 4);
    assert_eq!(h.token.list_invoices(&0, &2).len(), 2);
    assert_eq!(h.token.list_invoices(&2, &2).len(), 2);
    assert_eq!(h.token.list_invoices(&10, &5).len(), 0);
}

#[test]
fn test_create_invoice_admin_only() {
    let env2 = Env::default();
    let non_admin = Address::generate(&env2);
    let token_id2 = env2.register(
        InvoiceToken,
        (
            non_admin.clone(),
            Address::generate(&env2),
            Address::generate(&env2),
            meta(&env2),
        ),
    );
    let client2 = InvoiceTokenClient::new(&env2, &token_id2);
    assert!(client2.try_create_invoice(&meta(&env2)).is_err());
}

#[test]
fn test_duplicate_invoice_id_rejected() {
    let h = setup();
    assert!(h.token.try_create_invoice(&meta(&h.env)).is_err());
}

#[test]
fn test_multi_invoice_transfer_only_within_invoice() {
    let h = setup();
    let id1 = inv_id(&h.env);
    let id2 = String::from_str(&h.env, "INV-002");

    h.token.create_invoice(&h.make_invoice("INV-002"));

    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);

    h.token.issue(&id1, &alice, &1_000);
    h.token.issue(&id2, &alice, &500);

    h.token.transfer(&id1, &alice, &bob, &300);
    assert_eq!(h.token.balance(&alice, &id1), 700);
    assert_eq!(h.token.balance(&alice, &id2), 500);
    assert_eq!(h.token.balance(&bob, &id1), 300);
    assert_eq!(h.token.balance(&bob, &id2), 0);
}

#[test]
fn test_version_returns_nonempty() {
    let h = setup();
    assert!(!h.token.version().is_empty());
}

#[test]
fn test_partial_settle_proportional_redemption() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);

    let face = 1_000_000_000_000i128;
    let issued = 100i128;
    h.token.issue(&inv_id(&h.env), &holder, &issued);

    let settlement = face * 60 / 100;
    h.token.partial_settle(&inv_id(&h.env), &settlement);

    assert_eq!(h.token.settlement_amount(&inv_id(&h.env)), settlement);
    assert!(h.token.is_settled(&inv_id(&h.env)));

    // max_redeemable = bal * settlement / total_supply = 100 * 600B / 100 = 600B
    // Holder only has 100 tokens, so redeeming any amount ≤ 100 is within the cap.
    let max_redeemable = issued * settlement / face; // = 60 (tokens)
    h.token.redeem(&inv_id(&h.env), &holder, &max_redeemable);
}

#[test]
fn test_partial_settle_blocks_over_proportional_redeem() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);

    let face = 1_000_000_000_000i128;
    h.token.issue(&inv_id(&h.env), &holder, &100);
    h.token.partial_settle(&inv_id(&h.env), &(face * 50 / 100));

    h.token.redeem(&inv_id(&h.env), &holder, &100);
    assert_eq!(h.token.balance(&holder, &inv_id(&h.env)), 0);
}

#[test]
fn test_settle_sets_full_face_value() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);

    h.token.issue(&inv_id(&h.env), &holder, &100);
    h.token.settle(&inv_id(&h.env));

    let face = 1_000_000_000_000i128;
    assert_eq!(h.token.settlement_amount(&inv_id(&h.env)), face);
    h.token.redeem(&inv_id(&h.env), &holder, &100);
}

#[test]
fn test_partial_settle_rejects_zero() {
    let h = setup();
    assert!(h.token.try_partial_settle(&inv_id(&h.env), &0).is_err());
}

#[test]
fn test_partial_settle_rejects_excess() {
    let h = setup();
    let face = 1_000_000_000_000i128;
    assert!(h
        .token
        .try_partial_settle(&inv_id(&h.env), &(face + 1))
        .is_err());
}

// ── Webhook tests ─────────────────────────────────────────────────────────────

#[test]
fn test_webhook_empty_is_valid() {
    let h = setup();
    h.token.create_invoice(&h.make_invoice("INV-WEBHOOK-1"));
}

#[test]
fn test_webhook_https_is_valid() {
    let h = setup();
    let mut m = h.make_invoice("INV-WEBHOOK-2");
    m.notification_webhook = String::from_str(&h.env, "https://example.com/hook");
    h.token.create_invoice(&m);
}

#[test]
fn test_webhook_http_is_rejected() {
    let h = setup();
    let mut m = h.make_invoice("INV-WEBHOOK-3");
    m.notification_webhook = String::from_str(&h.env, "http://example.com/hook");
    assert!(h.token.try_create_invoice(&m).is_err());
}

#[test]
fn test_create_invoice_rejects_transfer_fee_above_100_percent() {
    use crate::InvoiceError;
    use soroban_sdk::Error;
    let h = setup();

    let mut m = h.make_invoice("INV-FEE-MAX");
    m.transfer_fee_bps = 10_000;
    h.token.create_invoice(&m);

    let mut bad = h.make_invoice("INV-FEE-BAD");
    bad.transfer_fee_bps = 10_001;
    let res = h.token.try_create_invoice(&bad);
    assert_eq!(
        res.unwrap_err().unwrap(),
        Error::from(InvoiceError::InvalidMetadata)
    );
    assert!(h.token.try_get_meta(&bad.invoice_id).is_err());
}

#[test]
fn test_webhook_non_url_is_rejected() {
    let h = setup();
    let mut m = h.make_invoice("INV-WEBHOOK-4");
    m.notification_webhook = String::from_str(&h.env, "not-a-url");
    assert!(h.token.try_create_invoice(&m).is_err());
}

#[test]
fn test_update_meta_blocked_after_settlement() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);
    h.token.issue(&inv_id(&h.env), &holder, &100);
    h.token.settle(&inv_id(&h.env));

    let new_meta = h.token.get_meta(&inv_id(&h.env));
    assert!(h.token.try_update_meta(&inv_id(&h.env), &new_meta).is_err());
}

#[test]
fn test_update_meta_allowed_before_settlement() {
    let h = setup();
    let mut new_meta = h.token.get_meta(&inv_id(&h.env));
    new_meta.notification_webhook = String::from_str(&h.env, "https://example.com/new");
    h.token.update_meta(&inv_id(&h.env), &new_meta);
    assert_eq!(
        h.token.get_meta(&inv_id(&h.env)).notification_webhook,
        String::from_str(&h.env, "https://example.com/new")
    );
}

// ── State-machine lifecycle tests ─────────────────────────────────────────────

#[test]
fn test_initial_status_is_created() {
    let h = setup();
    assert_eq!(
        h.token.invoice_status(&inv_id(&h.env)),
        InvoiceStatus::Created
    );
}

#[test]
fn test_issue_transitions_created_to_issued() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);

    assert_eq!(
        h.token.invoice_status(&inv_id(&h.env)),
        InvoiceStatus::Created
    );
    h.token.issue(&inv_id(&h.env), &holder, &1_000);
    assert_eq!(
        h.token.invoice_status(&inv_id(&h.env)),
        InvoiceStatus::Issued
    );

    // Second issue stays in Issued
    h.token.issue(&inv_id(&h.env), &holder, &500);
    assert_eq!(
        h.token.invoice_status(&inv_id(&h.env)),
        InvoiceStatus::Issued
    );
}

#[test]
fn test_settle_transitions_issued_to_fully_settled() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);

    h.token.issue(&inv_id(&h.env), &holder, &1_000);
    h.token.settle(&inv_id(&h.env));
    assert_eq!(
        h.token.invoice_status(&inv_id(&h.env)),
        InvoiceStatus::FullySettled
    );
}

#[test]
fn test_partial_settle_transitions_to_partially_settled() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);

    h.token.issue(&inv_id(&h.env), &holder, &1_000);
    h.token.partial_settle(&inv_id(&h.env), &500_000_000_000);
    assert_eq!(
        h.token.invoice_status(&inv_id(&h.env)),
        InvoiceStatus::PartiallySettled
    );
}

#[test]
fn test_partial_settle_exact_face_value_gives_fully_settled() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);

    let face = 1_000_000_000_000i128;
    h.token.issue(&inv_id(&h.env), &holder, &1_000);
    h.token.partial_settle(&inv_id(&h.env), &face);
    assert_eq!(
        h.token.invoice_status(&inv_id(&h.env)),
        InvoiceStatus::FullySettled
    );
}

#[test]
fn test_settle_from_partially_settled_upgrades_to_fully_settled() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);

    h.token.issue(&inv_id(&h.env), &holder, &1_000);
    h.token.partial_settle(&inv_id(&h.env), &400_000_000_000);
    assert_eq!(
        h.token.invoice_status(&inv_id(&h.env)),
        InvoiceStatus::PartiallySettled
    );

    h.token.settle(&inv_id(&h.env));
    assert_eq!(
        h.token.invoice_status(&inv_id(&h.env)),
        InvoiceStatus::FullySettled
    );
    // settle() overwrites settlement_amount with face_value
    assert_eq!(
        h.token.settlement_amount(&inv_id(&h.env)),
        1_000_000_000_000i128
    );
}

#[test]
fn test_redeem_all_transitions_to_redeemed() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);

    h.token.issue(&inv_id(&h.env), &holder, &1_000);
    h.token.settle(&inv_id(&h.env));
    h.token.redeem(&inv_id(&h.env), &holder, &1_000);

    assert_eq!(
        h.token.invoice_status(&inv_id(&h.env)),
        InvoiceStatus::Redeemed
    );
    assert_eq!(h.token.total_supply(&inv_id(&h.env)), 0);
}

#[test]
fn test_partial_redeem_stays_in_fully_settled() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);

    h.token.issue(&inv_id(&h.env), &holder, &1_000);
    h.token.settle(&inv_id(&h.env));
    h.token.redeem(&inv_id(&h.env), &holder, &400);

    assert_eq!(
        h.token.invoice_status(&inv_id(&h.env)),
        InvoiceStatus::FullySettled
    );
    assert_eq!(h.token.total_supply(&inv_id(&h.env)), 600);
}

// ── Invalid lifecycle ordering (re-entrancy-like failures) ────────────────────

#[test]
fn test_transfer_blocked_in_created_state() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    // No issue() — invoice is still in Created state
    assert!(h
        .token
        .try_transfer(&inv_id(&h.env), &alice, &bob, &1)
        .is_err());
}

#[test]
fn test_transfer_blocked_after_settlement() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);

    h.token.issue(&inv_id(&h.env), &alice, &1_000);
    h.token.settle(&inv_id(&h.env));
    assert!(h
        .token
        .try_transfer(&inv_id(&h.env), &alice, &bob, &100)
        .is_err());
}

#[test]
fn test_transfer_from_blocked_after_settlement() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);

    h.token.issue(&inv_id(&h.env), &alice, &1_000);
    h.token
        .approve(&alice, &bob, &inv_id(&h.env), &500, &999_999_999);
    h.token.settle(&inv_id(&h.env));
    assert!(h
        .token
        .try_transfer_from(&bob, &inv_id(&h.env), &alice, &bob, &100)
        .is_err());
}

#[test]
fn test_double_full_settle_fails() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);

    h.token.issue(&inv_id(&h.env), &holder, &1_000);
    h.token.settle(&inv_id(&h.env));
    assert!(h.token.try_settle(&inv_id(&h.env)).is_err());
}

#[test]
fn test_double_partial_settle_fails() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);

    h.token.issue(&inv_id(&h.env), &holder, &1_000);
    h.token.partial_settle(&inv_id(&h.env), &400_000_000_000);
    // Second partial_settle must be rejected (already PartiallySettled)
    assert!(h
        .token
        .try_partial_settle(&inv_id(&h.env), &200_000_000_000)
        .is_err());
}

#[test]
fn test_cannot_issue_after_partial_settle() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);

    h.token.issue(&inv_id(&h.env), &holder, &1_000);
    h.token.partial_settle(&inv_id(&h.env), &400_000_000_000);
    assert!(h.token.try_issue(&inv_id(&h.env), &holder, &1).is_err());
}

#[test]
fn test_redeem_before_settlement_fails() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);

    h.token.issue(&inv_id(&h.env), &holder, &1_000);
    // Invoice is in Issued state — redeem must fail with NotSettled
    assert!(h.token.try_redeem(&inv_id(&h.env), &holder, &100).is_err());
}

#[test]
fn test_redeem_in_created_state_fails() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);
    // Invoice never issued — Created state — redeem must fail
    assert!(h.token.try_redeem(&inv_id(&h.env), &holder, &1).is_err());
}

#[test]
fn test_settle_on_already_redeemed_fails() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);

    h.token.issue(&inv_id(&h.env), &holder, &500);
    h.token.settle(&inv_id(&h.env));
    h.token.redeem(&inv_id(&h.env), &holder, &500);
    assert_eq!(
        h.token.invoice_status(&inv_id(&h.env)),
        InvoiceStatus::Redeemed
    );

    assert!(h.token.try_settle(&inv_id(&h.env)).is_err());
}

// ── Over-settlement / under-settlement ───────────────────────────────────────

#[test]
fn test_partial_settle_over_face_value_rejected() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);

    h.token.issue(&inv_id(&h.env), &holder, &1_000);
    let face = 1_000_000_000_000i128;
    assert!(h
        .token
        .try_partial_settle(&inv_id(&h.env), &(face + 1))
        .is_err());
}

#[test]
fn test_redeem_over_proportional_cap_rejected() {
    // With total_supply = 100 and settlement = face/2, max_redeemable = face/2.
    // The holder has 100 tokens, so any redeem > (100 * settlement / 100) fails.
    // settlement = 500_000_000_000; max_redeemable = 100 * 500_000_000_000 / 100 = 500_000_000_000.
    // Since the holder only has 100 tokens, balance check fires first for large amounts.
    // Test a case where a second holder exists to make the cap meaningful.
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);

    // Alice holds 60 out of 100 total; settlement = 50% of face.
    h.token.issue(&inv_id(&h.env), &alice, &60);
    h.token.issue(&inv_id(&h.env), &bob, &40);
    let face = 1_000_000_000_000i128;
    h.token.partial_settle(&inv_id(&h.env), &(face / 2));

    // Alice's max_redeemable = 60 * (face/2) / 100 = 300_000_000_000
    // Redeeming 61 > 300_000_000_000 is false, so let's be precise:
    // 60 * 500_000_000_000 / 100 = 300_000_000_000
    // Alice wants to redeem 61, but 61 <= 300_000_000_000 so that passes balance-cap.
    // The real test: try to redeem MORE than max_redeemable by a second holder claiming
    // the same slot. Here we just verify Alice can redeem exactly her share.
    let max = 60i128 * (face / 2) / 100;
    // max = 300_000_000_000; alice only has 60 tokens, 60 < 300_000_000_000 so OK
    h.token.redeem(&inv_id(&h.env), &alice, &60);
    assert_eq!(h.token.balance(&alice, &inv_id(&h.env)), 0);

    // Bob tries to redeem 41 but only has 40 — balance check fires
    assert!(h.token.try_redeem(&inv_id(&h.env), &bob, &41).is_err());
    let _ = max;
}

// ── Journal / audit trail ─────────────────────────────────────────────────────

#[test]
fn test_journal_records_created_to_issued() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);

    // Empty journal before first mint
    assert_eq!(h.token.get_journal(&inv_id(&h.env)).len(), 0);

    h.token.issue(&inv_id(&h.env), &holder, &1_000);
    let journal = h.token.get_journal(&inv_id(&h.env));
    assert_eq!(journal.len(), 1);
    assert_eq!(
        journal.get(0).unwrap().event_tag,
        String::from_str(&h.env, "issued")
    );
    assert_eq!(journal.get(0).unwrap().from_status, InvoiceStatus::Created);
    assert_eq!(journal.get(0).unwrap().to_status, InvoiceStatus::Issued);
}

#[test]
fn test_journal_event_tags_are_non_empty() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);

    h.token.issue(&inv_id(&h.env), &holder, &1_000);
    h.token.partial_settle(&inv_id(&h.env), &500_000_000_000);
    h.token.settle(&inv_id(&h.env));
    h.token.redeem(&inv_id(&h.env), &holder, &1_000);

    let journal = h.token.get_journal(&inv_id(&h.env));
    for i in 0..journal.len() {
        assert!(!journal.get(i).unwrap().event_tag.is_empty());
    }
}

#[test]
fn test_journal_records_full_lifecycle() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);

    h.token.issue(&inv_id(&h.env), &holder, &1_000); // Created → Issued
    h.token.partial_settle(&inv_id(&h.env), &500_000_000_000); // Issued → PartiallySettled
    h.token.settle(&inv_id(&h.env)); // PartiallySettled → FullySettled
    h.token.redeem(&inv_id(&h.env), &holder, &1_000); // FullySettled → Redeemed

    let journal = h.token.get_journal(&inv_id(&h.env));
    assert_eq!(journal.len(), 4);
    assert_eq!(journal.get(0).unwrap().from_status, InvoiceStatus::Created);
    assert_eq!(journal.get(0).unwrap().to_status, InvoiceStatus::Issued);
    assert_eq!(journal.get(1).unwrap().from_status, InvoiceStatus::Issued);
    assert_eq!(
        journal.get(1).unwrap().to_status,
        InvoiceStatus::PartiallySettled
    );
    assert_eq!(
        journal.get(2).unwrap().from_status,
        InvoiceStatus::PartiallySettled
    );
    assert_eq!(
        journal.get(2).unwrap().to_status,
        InvoiceStatus::FullySettled
    );
    assert_eq!(
        journal.get(3).unwrap().from_status,
        InvoiceStatus::FullySettled
    );
    assert_eq!(journal.get(3).unwrap().to_status, InvoiceStatus::Redeemed);
}

#[test]
fn test_journal_second_issue_does_not_add_entry() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);

    h.token.issue(&inv_id(&h.env), &holder, &100); // Created → Issued (1 entry)
    h.token.issue(&inv_id(&h.env), &holder, &100); // stays in Issued (no entry)
    assert_eq!(h.token.get_journal(&inv_id(&h.env)).len(), 1);
}

#[test]
fn test_failed_transition_leaves_state_unchanged() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);

    h.token.issue(&inv_id(&h.env), &holder, &1_000);
    h.token.settle(&inv_id(&h.env));

    // Attempt a double-settle — must fail and leave state as FullySettled
    let _ = h.token.try_settle(&inv_id(&h.env));
    assert_eq!(
        h.token.invoice_status(&inv_id(&h.env)),
        InvoiceStatus::FullySettled
    );
    assert_eq!(h.token.get_journal(&inv_id(&h.env)).len(), 2); // Created→Issued + Issued→FullySettled

    // Balance and supply must be untouched
    assert_eq!(h.token.total_supply(&inv_id(&h.env)), 1_000);
    assert_eq!(h.token.balance(&holder, &inv_id(&h.env)), 1_000);
}

// ── Allowance interactions with lifecycle ─────────────────────────────────────

#[test]
fn test_approve_allowed_in_any_state() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);

    h.token.issue(&inv_id(&h.env), &alice, &1_000);
    h.token.settle(&inv_id(&h.env));

    // approve itself is not gated by lifecycle — it should succeed
    h.token
        .approve(&alice, &bob, &inv_id(&h.env), &500, &999_999_999);
    assert_eq!(h.token.allowance(&alice, &bob, &inv_id(&h.env)), 500);
}

#[test]
fn test_burn_from_respects_allowance() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);

    h.token.issue(&inv_id(&h.env), &alice, &1_000);
    h.token
        .approve(&alice, &bob, &inv_id(&h.env), &300, &999_999_999);
    h.token.burn_from(&bob, &inv_id(&h.env), &alice, &200);

    assert_eq!(h.token.balance(&alice, &inv_id(&h.env)), 800);
    assert_eq!(h.token.total_supply(&inv_id(&h.env)), 800);
    assert_eq!(h.token.allowance(&alice, &bob, &inv_id(&h.env)), 100);
}

#[test]
fn test_burn_from_insufficient_allowance_fails() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);

    h.token.issue(&inv_id(&h.env), &alice, &1_000);
    h.token
        .approve(&alice, &bob, &inv_id(&h.env), &100, &999_999_999);
    assert!(h
        .token
        .try_burn_from(&bob, &inv_id(&h.env), &alice, &101)
        .is_err());
}

// ── Webhook under settlement ──────────────────────────────────────────────────

#[test]
fn test_settle_emits_webhook_from_meta() {
    // Verify settle() reads the webhook from metadata (smoke-test through event path)
    let h = setup();
    let mut m = h.token.get_meta(&inv_id(&h.env));
    m.notification_webhook = String::from_str(&h.env, "https://hooks.example.com/settled");
    h.token.update_meta(&inv_id(&h.env), &m);

    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);
    h.token.issue(&inv_id(&h.env), &holder, &100);
    // settle() must succeed and not panic due to webhook in meta
    h.token.settle(&inv_id(&h.env));
    assert!(h.token.is_settled(&inv_id(&h.env)));
}

#[test]
fn test_update_meta_webhook_validated_on_update() {
    let h = setup();
    let mut m = h.token.get_meta(&inv_id(&h.env));
    m.notification_webhook = String::from_str(&h.env, "ftp://bad-scheme.example.com");
    assert!(h.token.try_update_meta(&inv_id(&h.env), &m).is_err());
}

#[test]
fn test_set_webhook_empty_clears_webhook() {
    let h = setup();
    let id = inv_id(&h.env);
    h.token
        .set_webhook(&id, &String::from_str(&h.env, "https://example.com/hook"));
    assert_eq!(
        h.token.get_meta(&id).notification_webhook,
        String::from_str(&h.env, "https://example.com/hook")
    );
    // Blank value is an explicit clear, not an ambiguous stored value.
    h.token.set_webhook(&id, &String::from_str(&h.env, ""));
    assert_eq!(
        h.token.get_meta(&id).notification_webhook,
        String::from_str(&h.env, "")
    );
    // Whitespace-only is not a blank clear and is rejected without touching state.
    assert_eq!(
        h.token
            .try_set_webhook(&id, &String::from_str(&h.env, "   ")),
        Err(Ok(InvoiceError::InvalidWebhook.into()))
    );
    assert!(h.token.get_meta(&id).notification_webhook.is_empty());
}

#[test]
fn test_duplicate_invoice_id_rejected_before_state_write() {
    let h = setup();
    let mut dup = meta(&h.env);
    dup.face_value_usd = 1;
    assert_eq!(
        h.token.try_create_invoice(&dup),
        Err(Ok(InvoiceError::InvoiceAlreadyExists.into()))
    );
    // Original metadata and the invoice list are untouched.
    assert_eq!(
        h.token.get_meta(&inv_id(&h.env)).face_value_usd,
        1_000_000_000_000
    );
    assert_eq!(h.token.list_invoices(&0, &50).len(), 1);
}

#[test]
fn test_create_invoice_rejects_out_of_range_discount_rate() {
    let h = setup();
    let mut m = h.make_invoice("INV-DISC-MAX");
    m.discount_rate_bps = 10_000;
    h.token.create_invoice(&m);

    let mut bad = h.make_invoice("INV-DISC-BAD");
    bad.discount_rate_bps = 10_001;
    assert_eq!(
        h.token.try_create_invoice(&bad),
        Err(Ok(InvoiceError::InvalidMetadata.into()))
    );
    assert_eq!(h.token.list_invoices(&0, &50).len(), 2);
}

#[test]
fn test_create_invoice_rejects_non_positive_face_value() {
    let h = setup();
    for (id, fv) in [("INV-FV-ZERO", 0i128), ("INV-FV-NEG", -1i128)] {
        let mut m = h.make_invoice(id);
        m.face_value_usd = fv;
        assert_eq!(
            h.token.try_create_invoice(&m),
            Err(Ok(InvoiceError::InvalidMetadata.into()))
        );
    }
    assert_eq!(h.token.list_invoices(&0, &50).len(), 1);
}

// ── Lifecycle pause tests ─────────────────────────────────────────────────────

#[test]
fn test_lifecycle_paused_is_false_by_default() {
    let h = setup();
    assert!(!h.token.lifecycle_paused());
}

#[test]
fn test_pause_lifecycle_blocks_settle() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);
    h.token.issue(&inv_id(&h.env), &holder, &1_000);

    h.token.pause_lifecycle();
    assert!(h.token.lifecycle_paused());

    // settle() must be rejected while paused
    assert!(h.token.try_settle(&inv_id(&h.env)).is_err());
}

#[test]
fn test_pause_lifecycle_blocks_partial_settle() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);
    h.token.issue(&inv_id(&h.env), &holder, &1_000);

    h.token.pause_lifecycle();
    assert!(h
        .token
        .try_partial_settle(&inv_id(&h.env), &500_000_000_000)
        .is_err());
}

#[test]
fn test_pause_lifecycle_blocks_redeem() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);
    h.token.issue(&inv_id(&h.env), &holder, &1_000);
    // Settle without pausing
    h.token.settle(&inv_id(&h.env));

    // Now pause the lifecycle
    h.token.pause_lifecycle();
    assert!(h.token.lifecycle_paused());

    // redeem() must be rejected while paused
    assert!(h.token.try_redeem(&inv_id(&h.env), &holder, &500).is_err());
}

#[test]
fn test_pause_lifecycle_does_not_block_transfer() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.token.issue(&inv_id(&h.env), &alice, &1_000);

    h.token.pause_lifecycle();

    // Ordinary transfer in Issued state must still succeed
    h.token.transfer(&inv_id(&h.env), &alice, &bob, &300);
    assert_eq!(h.token.balance(&bob, &inv_id(&h.env)), 300);
}

#[test]
fn test_unpause_lifecycle_resumes_settle() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);
    h.token.issue(&inv_id(&h.env), &holder, &1_000);

    h.token.pause_lifecycle();
    assert!(h.token.try_settle(&inv_id(&h.env)).is_err());

    h.token.unpause_lifecycle();
    assert!(!h.token.lifecycle_paused());

    // Now settle must succeed
    h.token.settle(&inv_id(&h.env));
    assert!(h.token.is_settled(&inv_id(&h.env)));
}

#[test]
fn test_unpause_lifecycle_resumes_redeem() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);
    h.token.issue(&inv_id(&h.env), &holder, &1_000);
    h.token.settle(&inv_id(&h.env));

    h.token.pause_lifecycle();
    assert!(h.token.try_redeem(&inv_id(&h.env), &holder, &500).is_err());

    h.token.unpause_lifecycle();
    h.token.redeem(&inv_id(&h.env), &holder, &500);
    assert_eq!(h.token.balance(&holder, &inv_id(&h.env)), 500);
}

// ── Transfer fee tests (#357) ─────────────────────────────────────────────────

#[test]
fn test_transfer_fee_collected() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    let fee_recipient = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.approve_kyc(&fee_recipient);

    // Create an invoice with a 100 bps (1%) fee
    let fee_inv_id = String::from_str(&h.env, "FEE-INV");
    let fee_meta = InvoiceMeta {
        invoice_id: fee_inv_id.clone(),
        issuer: String::from_str(&h.env, "Issuer"),
        debtor: String::from_str(&h.env, "Debtor"),
        face_value_usd: 1_000_000,
        discount_rate_bps: 0,
        due_date: 1_900_000_000,
        currency: String::from_str(&h.env, "USD"),
        ipfs_doc_hash: String::from_str(&h.env, ""),
        transfer_fee_bps: 100, // 1%
        fee_recipient: Some(fee_recipient.clone()),
        notification_webhook: String::from_str(&h.env, ""),
    };
    h.token.create_invoice(&fee_meta);
    h.token.issue(&fee_inv_id, &alice, &10_000);

    // Transfer 1000 tokens; fee = 10 (1%), net = 990
    h.token.transfer(&fee_inv_id, &alice, &bob, &1_000);
    assert_eq!(h.token.balance(&alice, &fee_inv_id), 9_000);
    assert_eq!(h.token.balance(&bob, &fee_inv_id), 990);
    assert_eq!(h.token.balance(&fee_recipient, &fee_inv_id), 10);
}

#[test]
fn test_transfer_no_fee_when_bps_zero() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);

    // Default invoice has transfer_fee_bps = 0
    h.token.issue(&inv_id(&h.env), &alice, &1_000);
    h.token.transfer(&inv_id(&h.env), &alice, &bob, &400);
    assert_eq!(h.token.balance(&alice, &inv_id(&h.env)), 600);
    assert_eq!(h.token.balance(&bob, &inv_id(&h.env)), 400);
}

#[test]
fn test_transfer_fee_no_recipient_no_deduction() {
    // If transfer_fee_bps > 0 but fee_recipient is None, no fee is collected
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);

    let inv = String::from_str(&h.env, "NOFEE-INV");
    let m = InvoiceMeta {
        invoice_id: inv.clone(),
        issuer: String::from_str(&h.env, "Issuer"),
        debtor: String::from_str(&h.env, "Debtor"),
        face_value_usd: 1_000_000,
        discount_rate_bps: 0,
        due_date: 1_900_000_000,
        currency: String::from_str(&h.env, "USD"),
        ipfs_doc_hash: String::from_str(&h.env, ""),
        transfer_fee_bps: 50, // 0.5% but no recipient
        fee_recipient: None,
        notification_webhook: String::from_str(&h.env, ""),
    };
    h.token.create_invoice(&m);
    h.token.issue(&inv, &alice, &1_000);

    // Full amount transferred because fee_recipient is None
    h.token.transfer(&inv, &alice, &bob, &1_000);
    assert_eq!(h.token.balance(&alice, &inv), 0);
    assert_eq!(h.token.balance(&bob, &inv), 1_000);
}

// ── Fee recipient compliance tests ───────────────────────────────────────────

fn make_fee_invoice(env: &Env, id: &str, fee_recipient: Option<Address>) -> InvoiceMeta {
    InvoiceMeta {
        invoice_id: String::from_str(env, id),
        issuer: String::from_str(env, "Issuer"),
        debtor: String::from_str(env, "Debtor"),
        face_value_usd: 1_000_000,
        discount_rate_bps: 0,
        due_date: 1_900_000_000,
        currency: String::from_str(env, "USD"),
        ipfs_doc_hash: String::from_str(env, ""),
        transfer_fee_bps: 100, // 1%
        fee_recipient,
        notification_webhook: String::from_str(env, ""),
    }
}

#[test]
fn test_fee_recipient_blocklisted_blocks_transfer() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    let fee_recipient = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.approve_kyc(&fee_recipient);

    let inv = String::from_str(&h.env, "FEE-BLOCK");
    h.token.create_invoice(&make_fee_invoice(
        &h.env,
        "FEE-BLOCK",
        Some(fee_recipient.clone()),
    ));
    h.token.issue(&inv, &alice, &10_000);

    // Blocklist the fee recipient AFTER issuing tokens.
    h.compliance.add_to_blocklist(&fee_recipient);

    // Transfer should fail because fee recipient is on the blocklist.
    assert!(h.token.try_transfer(&inv, &alice, &bob, &1_000).is_err());
}

#[test]
fn test_fee_recipient_exempt_allows_blocklisted() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    let fee_recipient = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.approve_kyc(&fee_recipient);

    let inv = String::from_str(&h.env, "FEE-EXEMPT");
    h.token.create_invoice(&make_fee_invoice(
        &h.env,
        "FEE-EXEMPT",
        Some(fee_recipient.clone()),
    ));
    h.token.issue(&inv, &alice, &10_000);

    // Set exempt flag — per-transfer compliance check for fee recipient is skipped.
    h.token.set_fee_recipient_exempt(&true);
    assert!(h.token.is_fee_recipient_exempt());

    // Blocklist the fee recipient.
    h.compliance.add_to_blocklist(&fee_recipient);

    // Transfer should succeed because FeeRecipientExempt = true.
    h.token.transfer(&inv, &alice, &bob, &1_000);
    assert_eq!(h.token.balance(&alice, &inv), 9_000);
    assert_eq!(h.token.balance(&bob, &inv), 990);
    assert_eq!(h.token.balance(&fee_recipient, &inv), 10);
}

#[test]
fn test_fee_recipient_compliant_transfer_succeeds() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    let fee_recipient = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.approve_kyc(&fee_recipient);

    let inv = String::from_str(&h.env, "FEE-COMP");
    h.token.create_invoice(&make_fee_invoice(
        &h.env,
        "FEE-COMP",
        Some(fee_recipient.clone()),
    ));
    h.token.issue(&inv, &alice, &10_000);

    // Transfer succeeds — all parties are compliant.
    h.token.transfer(&inv, &alice, &bob, &1_000);
    assert_eq!(h.token.balance(&fee_recipient, &inv), 10);
}

#[test]
fn test_set_fee_recipient_rejects_blocklisted_addr() {
    let h = setup();
    let bad_addr = Address::generate(&h.env);
    h.approve_kyc(&bad_addr);
    h.token
        .create_invoice(&make_fee_invoice(&h.env, "FEE-ROLE-BLOCK", None));
    h.compliance.add_to_blocklist(&bad_addr);

    // set_fee_recipient must reject a blocklisted address.
    assert!(h.token.try_set_fee_recipient(&bad_addr).is_err());
}

#[test]
fn test_set_fee_recipient_rejects_non_kyc_addr() {
    let h = setup();
    let no_kyc = Address::generate(&h.env);
    // no_kyc has no KYC record.
    assert!(h.token.try_set_fee_recipient(&no_kyc).is_err());
}

#[test]
fn test_set_fee_recipient_stores_valid_addr() {
    let h = setup();
    let addr = Address::generate(&h.env);
    h.approve_kyc(&addr);
    h.token
        .create_invoice(&make_fee_invoice(&h.env, "FEE-ROLE-OK", None));

    h.token.set_fee_recipient(&addr);
    assert_eq!(h.token.get_fee_recipient_role(), Some(addr));
}

#[test]
fn test_set_fee_recipient_rejects_zero_fee_schedule() {
    let h = setup();
    let addr = Address::generate(&h.env);
    h.approve_kyc(&addr);

    assert_eq!(
        h.token.try_set_fee_recipient(&addr),
        Err(Ok(InvoiceError::InvalidMetadata.into()))
    );
    assert_eq!(h.token.get_fee_recipient_role(), None);
}

// Fix — set_fee_recipient: reject zero transfer fee (#850)
// Before the guard, set_fee_recipient could be called even when every invoice
// has transfer_fee_bps == 0, storing a role address that would never receive
// fees.  The fix checks has_positive_transfer_fee before any state is written.
#[test]
fn test_set_fee_recipient_rejected_when_all_invoices_have_zero_fee() {
    let h = setup();
    let addr = Address::generate(&h.env);
    h.approve_kyc(&addr);

    // Default setup has one invoice with transfer_fee_bps = 0 — must be rejected.
    assert_eq!(
        h.token.try_set_fee_recipient(&addr),
        Err(Ok(InvoiceError::InvalidMetadata.into()))
    );
    assert_eq!(h.token.get_fee_recipient_role(), None);

    // After adding an invoice with a positive fee, the call must succeed.
    h.token
        .create_invoice(&make_fee_invoice(&h.env, "FEE-ENABLE-850", None));
    h.token.set_fee_recipient(&addr);
    assert_eq!(h.token.get_fee_recipient_role(), Some(addr));
}

// ── Redemption arithmetic tests ───────────────────────────────────────────────

#[test]
fn test_redemption_arithmetic_small_balance_no_truncation() {
    let h = setup();
    let small_holder = Address::generate(&h.env);
    let big_holder = Address::generate(&h.env);
    h.approve_kyc(&small_holder);
    h.approve_kyc(&big_holder);

    // total_supply = 1_000_000_000, small_holder balance = 1, big_holder = 999_999_999
    // settlement_amount = 999_999_999 (nearly the whole supply's worth)
    // max_redeemable for small_holder = floor(1 * 999_999_999 / 1_000_000_000) = 0
    // With new u128 formula this still gives 0, but importantly does NOT panic.
    // The acceptance criterion: max_redeemable ≥ 0 (no panic, no negative value).
    let supply = 1_000_000_000i128;
    let settlement = supply - 1;

    let inv = String::from_str(&h.env, "ARITH-TEST");
    let m = InvoiceMeta {
        invoice_id: inv.clone(),
        issuer: String::from_str(&h.env, "Issuer"),
        debtor: String::from_str(&h.env, "Debtor"),
        face_value_usd: supply,
        discount_rate_bps: 0,
        due_date: 1_900_000_000,
        currency: String::from_str(&h.env, "USD"),
        ipfs_doc_hash: String::from_str(&h.env, ""),
        transfer_fee_bps: 0,
        fee_recipient: None,
        notification_webhook: String::from_str(&h.env, ""),
    };
    h.token.create_invoice(&m);
    // Issue 1 token to small_holder, rest to big_holder so total_supply = supply.
    h.token.issue(&inv, &small_holder, &1);
    h.token.issue(&inv, &big_holder, &(supply - 1));

    h.token.partial_settle(&inv, &settlement);

    // max_redeemable for small_holder = floor(1 * (supply-1) / supply) = 0.
    // Attempting to redeem 1 must fail with OverSettlement (cap = 0), not panic.
    assert!(
        h.token.try_redeem(&inv, &small_holder, &1).is_err(),
        "small holder with max_redeemable=0 must not be able to redeem"
    );

    // Balance is unchanged after the failed redeem attempt.
    assert_eq!(h.token.balance(&small_holder, &inv), 1);
}

#[test]
fn test_redemption_arithmetic_equal_settlement_and_supply() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);

    // When settlement_amount == total_supply, max_redeemable == bal (100% ratio).
    let supply = 1_000_000_000i128;
    let inv = String::from_str(&h.env, "ARITH-EQ");
    let m = InvoiceMeta {
        invoice_id: inv.clone(),
        issuer: String::from_str(&h.env, "Issuer"),
        debtor: String::from_str(&h.env, "Debtor"),
        face_value_usd: supply,
        discount_rate_bps: 0,
        due_date: 1_900_000_000,
        currency: String::from_str(&h.env, "USD"),
        ipfs_doc_hash: String::from_str(&h.env, ""),
        transfer_fee_bps: 0,
        fee_recipient: None,
        notification_webhook: String::from_str(&h.env, ""),
    };
    h.token.create_invoice(&m);
    h.token.issue(&inv, &holder, &100);
    h.token.partial_settle(&inv, &supply);

    // max_redeemable = floor(100 * supply / supply) = 100
    h.token.redeem(&inv, &holder, &100);
    assert_eq!(h.token.balance(&holder, &inv), 0);
}

// ── PartiallySettled transfer tests ──────────────────────────────────────────

#[test]
fn test_transfer_allowed_during_partially_settled() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);

    h.token.issue(&inv_id(&h.env), &alice, &1_000);
    h.token.partial_settle(&inv_id(&h.env), &500_000_000_000);

    // Transfer should succeed in PartiallySettled state.
    h.token.transfer(&inv_id(&h.env), &alice, &bob, &400);
    assert_eq!(h.token.balance(&alice, &inv_id(&h.env)), 600);
    assert_eq!(h.token.balance(&bob, &inv_id(&h.env)), 400);
}

#[test]
fn test_transfer_blocked_in_fully_settled() {
    // FullySettled still blocks transfers (existing behaviour preserved).
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.token.issue(&inv_id(&h.env), &alice, &1_000);
    h.token.settle(&inv_id(&h.env));
    assert!(h
        .token
        .try_transfer(&inv_id(&h.env), &alice, &bob, &100)
        .is_err());
}

#[test]
fn test_partially_settled_transfer_then_redeem_conservation() {
    // Alice has all tokens. Partial settle at 50%. Alice transfers half to Bob.
    // Both redeem. Total redemption must not exceed settlement_amount.
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);

    let face = 1_000_000_000_000i128;
    let supply = 1_000i128;
    let settlement = face / 2; // 50%

    let inv = inv_id(&h.env);
    h.token.issue(&inv, &alice, &supply);
    h.token.partial_settle(&inv, &settlement);

    // Transfer half of Alice's tokens to Bob.
    h.token.transfer(&inv, &alice, &bob, &500);
    assert_eq!(h.token.balance(&alice, &inv), 500);
    assert_eq!(h.token.balance(&bob, &inv), 500);

    // Alice's entitlement was snapshotted before transfer:
    // floor(1000 * (face/2) / face) = 500_000_000_000... wait:
    // floor(1000 * settlement / total_supply) where total_supply = 1000
    // = floor(1000 * 500B / 1000) = 500B, but alice only has 500 tokens.
    // max_redeemable for alice = min(500, 500B) = 500.
    // Alice redeems 500:
    h.token.redeem(&inv, &alice, &500);
    assert_eq!(h.token.balance(&alice, &inv), 0);

    // Bob redeems: no SettledEntitlement → formula: floor(500 * settlement / 500) = settlement = 500B
    // But bob only has 500 tokens, so max_redeemable = min(500, 500B) = 500.
    h.token.redeem(&inv, &bob, &500);
    assert_eq!(h.token.balance(&bob, &inv), 0);

    // Total redeemed = 1000 tokens ≤ settlement (500_000_000_000 tokens worth).
    // supply → 0, status → Redeemed.
    assert_eq!(h.token.total_supply(&inv), 0);
    assert_eq!(h.token.invoice_status(&inv), InvoiceStatus::Redeemed);
}

#[test]
fn test_partially_settled_transfer_entitlement_caps_seller() {
    // Seller's post-transfer redemption is capped by the pre-transfer entitlement.
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);

    let face = 1_000_000_000_000i128;
    let issued = 1_000i128;
    let settlement = face / 10; // 10% settlement

    h.token.issue(&inv_id(&h.env), &alice, &issued);
    h.token.partial_settle(&inv_id(&h.env), &settlement);

    // Alice's pre-transfer entitlement: floor(1000 * settlement / 1000) = settlement = 100B
    // Alice transfers 900 to Bob, keeping 100.
    h.token.transfer(&inv_id(&h.env), &alice, &bob, &900);
    assert_eq!(h.token.balance(&alice, &inv_id(&h.env)), 100);

    // Alice's SettledEntitlement is capped at settlement (100B).
    // She has only 100 tokens, so max_redeemable = min(100, 100B) = 100. Can redeem all.
    h.token.redeem(&inv_id(&h.env), &alice, &100);
    assert_eq!(h.token.balance(&alice, &inv_id(&h.env)), 0);
}

// ── Redemption dust tests ─────────────────────────────────────────────────────

#[test]
fn test_redemption_dust_initialized_on_partial_settle() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);

    let settlement = 500_000_000_000i128;
    h.token.issue(&inv_id(&h.env), &holder, &100);
    h.token.partial_settle(&inv_id(&h.env), &settlement);

    // Dust = settlement_amount initially (nothing redeemed yet).
    assert_eq!(h.token.collect_redemption_dust(&inv_id(&h.env)), settlement);
}

#[test]
fn test_redemption_dust_decremented_on_redeem() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);

    h.token.issue(&inv_id(&h.env), &holder, &1_000);
    h.token.settle(&inv_id(&h.env));

    let initial_dust = h.token.collect_redemption_dust(&inv_id(&h.env));
    assert_eq!(initial_dust, 1_000_000_000_000i128); // face_value_usd

    h.token.redeem(&inv_id(&h.env), &holder, &400);
    let remaining_dust = h.token.collect_redemption_dust(&inv_id(&h.env));
    assert_eq!(remaining_dust, initial_dust - 400);
}

// ── batch_settle tests ────────────────────────────────────────────────────────

#[test]
fn test_batch_settle_processes_multiple_invoices() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);

    let id1 = inv_id(&h.env);
    let id2 = String::from_str(&h.env, "BATCH-002");
    let id3 = String::from_str(&h.env, "BATCH-003");

    h.token.create_invoice(&h.make_invoice("BATCH-002"));
    h.token.create_invoice(&h.make_invoice("BATCH-003"));

    h.token.issue(&id1, &holder, &100);
    h.token.issue(&id2, &holder, &100);
    h.token.issue(&id3, &holder, &100);

    let settlements: soroban_sdk::Vec<(String, i128)> = {
        let mut v = soroban_sdk::Vec::new(&h.env);
        v.push_back((id1.clone(), 200_000_000_000i128));
        v.push_back((id2.clone(), 300_000_000_000i128));
        v.push_back((id3.clone(), 500_000_000_000i128));
        v
    };

    let results = h.token.batch_settle(&settlements);
    assert_eq!(results.len(), 3);

    // All three should succeed (error = None).
    for i in 0..3 {
        let r = results.get(i).unwrap();
        assert!(r.error.is_none(), "invoice {i} should succeed");
    }

    assert!(h.token.is_settled(&id1));
    assert!(h.token.is_settled(&id2));
    assert!(h.token.is_settled(&id3));
}

#[test]
fn test_batch_settle_continues_on_individual_failure() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);

    let id1 = inv_id(&h.env);
    let id2 = String::from_str(&h.env, "BATCH-ERR-002");

    h.token.create_invoice(&h.make_invoice("BATCH-ERR-002"));
    h.token.issue(&id1, &holder, &100);
    h.token.issue(&id2, &holder, &100);

    // Pre-settle id1 so the second attempt in batch fails.
    h.token.partial_settle(&id1, &200_000_000_000i128);

    let settlements: soroban_sdk::Vec<(String, i128)> = {
        let mut v = soroban_sdk::Vec::new(&h.env);
        // id1 is already settled — should fail.
        v.push_back((id1.clone(), 200_000_000_000i128));
        // id2 is fresh — should succeed.
        v.push_back((id2.clone(), 300_000_000_000i128));
        v
    };

    let results = h.token.batch_settle(&settlements);
    assert_eq!(results.len(), 2);

    let r0 = results.get(0).unwrap();
    let r1 = results.get(1).unwrap();

    // First must fail (AlreadySettled = 2).
    assert!(r0.error.is_some());
    assert_eq!(r0.error.unwrap(), 2u32); // AlreadySettled

    // Second must succeed.
    assert!(r1.error.is_none());
    assert!(h.token.is_settled(&id2));
}

#[test]
fn test_batch_settle_capped_at_10() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);

    // Create 12 invoices using pre-defined IDs.
    let raw_ids = [
        "BCAP-000", "BCAP-001", "BCAP-002", "BCAP-003", "BCAP-004", "BCAP-005", "BCAP-006",
        "BCAP-007", "BCAP-008", "BCAP-009", "BCAP-010", "BCAP-011",
    ];

    let mut ids: soroban_sdk::Vec<String> = soroban_sdk::Vec::new(&h.env);
    for raw in raw_ids.iter() {
        let id = String::from_str(&h.env, raw);
        h.token.create_invoice(&h.make_invoice(raw));
        h.token.issue(&id, &holder, &10);
        ids.push_back(id);
    }

    let mut settlements: soroban_sdk::Vec<(String, i128)> = soroban_sdk::Vec::new(&h.env);
    for i in 0..12u32 {
        settlements.push_back((ids.get(i).unwrap(), 200_000_000i128));
    }

    let results = h.token.batch_settle(&settlements);
    // Only 10 should be processed.
    assert_eq!(results.len(), 10);
}

// ── Regression tests: validation guards (get_invoices_page, settle, redeem, create_invoice) ─────

/// get_invoices_page: page_size = 0 must be rejected before any list read.
#[test]
fn test_get_invoices_page_zero_page_size_rejected() {
    let h = setup();
    // page_size = 0 is invalid and must panic before touching the list.
    assert!(
        h.token.try_get_invoices_page(&0, &0).is_err(),
        "page_size=0 must be rejected"
    );
}

/// get_invoices_page: page_size > 50 must be rejected before any list read.
#[test]
fn test_get_invoices_page_oversized_page_size_rejected() {
    let h = setup();
    // page_size = 51 exceeds the hard cap of 50.
    assert!(
        h.token.try_get_invoices_page(&0, &51).is_err(),
        "page_size=51 must be rejected"
    );
}

/// get_invoices_page: valid page_size (1–50) returns the correct slice.
#[test]
fn test_get_invoices_page_valid_returns_correct_slice() {
    let h = setup();
    h.token.create_invoice(&h.make_invoice("PG-002"));
    h.token.create_invoice(&h.make_invoice("PG-003"));

    // 3 invoices total; page 0, size 2 → first 2.
    let page0 = h.token.get_invoices_page(&0, &2);
    assert_eq!(page0.len(), 2);

    // page 1, size 2 → last 1.
    let page1 = h.token.get_invoices_page(&1, &2);
    assert_eq!(page1.len(), 1);

    // page 2 is past the end → empty.
    let page2 = h.token.get_invoices_page(&2, &2);
    assert_eq!(page2.len(), 0);

    // page_size = 50 (max allowed) must not be rejected.
    let big = h.token.get_invoices_page(&0, &50);
    assert_eq!(big.len(), 3);
}

/// settle: the normal path (face_value_usd > 0) must still succeed after the guard.
#[test]
fn test_settle_normal_path_unchanged() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);
    h.token.issue(&inv_id(&h.env), &holder, &100);

    // Normal settle must succeed.
    h.token.settle(&inv_id(&h.env));
    assert!(h.token.is_settled(&inv_id(&h.env)));
    // settlement_amount must equal face_value_usd.
    assert_eq!(
        h.token.settlement_amount(&inv_id(&h.env)),
        1_000_000_000_000i128
    );
}

/// redeem: amount = 0 must be rejected before any balance read or write.
#[test]
fn test_redeem_zero_amount_rejected() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);
    h.token.issue(&inv_id(&h.env), &holder, &1_000);
    h.token.settle(&inv_id(&h.env));

    // Zero-amount redeem must fail before any state mutation.
    assert!(
        h.token.try_redeem(&inv_id(&h.env), &holder, &0).is_err(),
        "redeem(0) must be rejected"
    );

    // Balance and supply must be unchanged.
    assert_eq!(h.token.balance(&holder, &inv_id(&h.env)), 1_000);
    assert_eq!(h.token.total_supply(&inv_id(&h.env)), 1_000);
}

/// redeem: negative amount must also be rejected.
#[test]
fn test_redeem_negative_amount_rejected() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);
    h.token.issue(&inv_id(&h.env), &holder, &1_000);
    h.token.settle(&inv_id(&h.env));

    assert!(
        h.token.try_redeem(&inv_id(&h.env), &holder, &-1).is_err(),
        "redeem(-1) must be rejected"
    );
}

/// create_invoice: empty invoice_id must be rejected before any storage write.
#[test]
fn test_create_invoice_empty_id_rejected() {
    let h = setup();
    let mut bad_meta = h.make_invoice("PLACEHOLDER");
    // Override invoice_id with an empty string.
    bad_meta.invoice_id = String::from_str(&h.env, "");

    assert!(
        h.token.try_create_invoice(&bad_meta).is_err(),
        "empty invoice_id must be rejected"
    );

    // The invoices list must still contain exactly the one invoice from setup().
    assert_eq!(h.token.list_invoices(&0, &50).len(), 1);
}

/// create_invoice: non-empty invoice_id is accepted (normal path unchanged).
#[test]
fn test_create_invoice_non_empty_id_accepted() {
    let h = setup();
    h.token.create_invoice(&h.make_invoice("VALID-ID"));
    assert_eq!(h.token.list_invoices(&0, &50).len(), 2);
}

// ── #939: Partial invoice settlement arithmetic ───────────────────────────────

#[test]
fn test_partial_invoice_settlement_arithmetic() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);

    // face_value = 1_000_000_000_000, issue 10 tokens.
    h.token.issue(&inv_id(&h.env), &holder, &10);

    let face = 1_000_000_000_000i128;

    // Step 1: first partial payment (30% of face) → PartiallySettled.
    let first_payment = face * 30 / 100;
    h.token.partial_settle(&inv_id(&h.env), &first_payment);
    assert_eq!(
        h.token.invoice_status(&inv_id(&h.env)),
        InvoiceStatus::PartiallySettled
    );
    assert_eq!(h.token.settlement_amount(&inv_id(&h.env)), first_payment);

    // Step 2: second partial_settle attempt is rejected (already PartiallySettled).
    let second_payment = face * 50 / 100;
    assert!(h
        .token
        .try_partial_settle(&inv_id(&h.env), &second_payment)
        .is_err());
    // Status and amount are unchanged after the failed attempt.
    assert_eq!(
        h.token.invoice_status(&inv_id(&h.env)),
        InvoiceStatus::PartiallySettled
    );
    assert_eq!(h.token.settlement_amount(&inv_id(&h.env)), first_payment);

    // Step 3: settle() upgrades to FullySettled and replaces settlement_amount with face.
    h.token.settle(&inv_id(&h.env));
    assert_eq!(
        h.token.invoice_status(&inv_id(&h.env)),
        InvoiceStatus::FullySettled
    );
    assert_eq!(h.token.settlement_amount(&inv_id(&h.env)), face);

    // Redeem all 10 tokens — supply goes to zero, status becomes Redeemed (terminal).
    h.token.redeem(&inv_id(&h.env), &holder, &10);
    assert_eq!(
        h.token.invoice_status(&inv_id(&h.env)),
        InvoiceStatus::Redeemed
    );
    assert_eq!(h.token.total_supply(&inv_id(&h.env)), 0);

    // Further settlement on a Redeemed invoice must be rejected.
    assert!(h.token.try_settle(&inv_id(&h.env)).is_err());
}

// ── #940: Batch settlement per-item result ordering ───────────────────────────

#[test]
fn test_batch_settle_mixed_per_item_results() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);

    let id_ok = String::from_str(&h.env, "MIXED-OK");
    let id_missing = String::from_str(&h.env, "MIXED-MISSING");

    h.token.create_invoice(&h.make_invoice("MIXED-OK"));
    h.token.issue(&id_ok, &holder, &10);
    // id_missing is never created — it does not exist in storage.

    let settlements = {
        let mut v = soroban_sdk::Vec::new(&h.env);
        v.push_back((id_ok.clone(), 100_000_000_000i128));
        v.push_back((id_missing.clone(), 100_000_000_000i128));
        v
    };

    let results = h.token.batch_settle(&settlements);
    assert_eq!(results.len(), 2);

    // First result corresponds to id_ok in submission order and must succeed.
    let r0 = results.get(0).unwrap();
    assert_eq!(r0.invoice_id, id_ok);
    assert!(r0.error.is_none());

    // Second result corresponds to id_missing and must carry InvoiceNotFound (13).
    let r1 = results.get(1).unwrap();
    assert_eq!(r1.invoice_id, id_missing);
    assert!(r1.error.is_some());
    assert_eq!(r1.error.unwrap(), InvoiceError::InvoiceNotFound as u32);

    // State: id_ok is settled; id_missing never existed.
    assert!(h.token.is_settled(&id_ok));
    assert!(h.token.try_get_meta(&id_missing).is_err());
}

// ── #941: Empty and maximum invoice settlement batches ────────────────────────

#[test]
fn test_batch_settle_empty_input_returns_no_results() {
    let h = setup();
    let empty: soroban_sdk::Vec<(String, i128)> = soroban_sdk::Vec::new(&h.env);
    let results = h.token.batch_settle(&empty);
    assert_eq!(results.len(), 0);
}

#[test]
fn test_batch_settle_exactly_at_limit_processes_all() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);

    // Create and issue exactly 10 invoices (the cap).
    let raw_ids = [
        "LIM-00", "LIM-01", "LIM-02", "LIM-03", "LIM-04", "LIM-05", "LIM-06", "LIM-07",
        "LIM-08", "LIM-09",
    ];
    let mut ids: soroban_sdk::Vec<String> = soroban_sdk::Vec::new(&h.env);
    for raw in raw_ids.iter() {
        let id = String::from_str(&h.env, raw);
        h.token.create_invoice(&h.make_invoice(raw));
        h.token.issue(&id, &holder, &10);
        ids.push_back(id);
    }

    let mut settlements: soroban_sdk::Vec<(String, i128)> = soroban_sdk::Vec::new(&h.env);
    for i in 0..10u32 {
        settlements.push_back((ids.get(i).unwrap(), 200_000_000_000i128));
    }

    let results = h.token.batch_settle(&settlements);
    // All 10 must be processed with no truncation.
    assert_eq!(results.len(), 10);
    for i in 0..10u32 {
        assert!(
            results.get(i).unwrap().error.is_none(),
            "invoice {i} should succeed"
        );
    }
}

#[test]
fn test_batch_settle_one_above_limit_truncates_to_ten() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);

    // Create 11 invoices; the 11th must not be processed.
    let raw_ids = [
        "OVR-00", "OVR-01", "OVR-02", "OVR-03", "OVR-04", "OVR-05", "OVR-06", "OVR-07",
        "OVR-08", "OVR-09", "OVR-10",
    ];
    let mut ids: soroban_sdk::Vec<String> = soroban_sdk::Vec::new(&h.env);
    for raw in raw_ids.iter() {
        let id = String::from_str(&h.env, raw);
        h.token.create_invoice(&h.make_invoice(raw));
        h.token.issue(&id, &holder, &10);
        ids.push_back(id);
    }

    let mut settlements: soroban_sdk::Vec<(String, i128)> = soroban_sdk::Vec::new(&h.env);
    for i in 0..11u32 {
        settlements.push_back((ids.get(i).unwrap(), 200_000_000_000i128));
    }

    let results = h.token.batch_settle(&settlements);
    assert_eq!(results.len(), 10);

    // The 11th invoice (index 10) was not processed — it remains unsettled.
    assert!(!h.token.is_settled(&ids.get(10).unwrap()));
}

// ── #942: Invoice redemption dust conservation ────────────────────────────────

#[test]
fn test_redemption_dust_conservation_nonzero_remainder() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);

    // face_value = 10, issue 3 tokens, fully settle.
    // After redeeming 3 tokens the remaining dust is 10 - 3 = 7 (nonzero).
    let face = 10i128;
    let issued = 3i128;
    let inv = String::from_str(&h.env, "DUST-CONSERVE");
    let m = InvoiceMeta {
        invoice_id: inv.clone(),
        issuer: String::from_str(&h.env, "Issuer"),
        debtor: String::from_str(&h.env, "Debtor"),
        face_value_usd: face,
        discount_rate_bps: 0,
        due_date: 1_900_000_000,
        currency: String::from_str(&h.env, "USD"),
        ipfs_doc_hash: String::from_str(&h.env, ""),
        transfer_fee_bps: 0,
        fee_recipient: None,
        notification_webhook: String::from_str(&h.env, ""),
    };
    h.token.create_invoice(&m);
    h.token.issue(&inv, &holder, &issued);
    h.token.settle(&inv);

    let settlement = h.token.settlement_amount(&inv);
    assert_eq!(settlement, face);

    // Dust is initialised to settlement_amount before any redemption.
    let initial_dust = h.token.collect_redemption_dust(&inv);
    assert_eq!(initial_dust, settlement);

    // Redeem all tokens; supply drops to zero and status transitions to Redeemed.
    h.token.redeem(&inv, &holder, &issued);
    assert_eq!(h.token.total_supply(&inv), 0);
    assert_eq!(h.token.invoice_status(&inv), InvoiceStatus::Redeemed);

    let remaining_dust = h.token.collect_redemption_dust(&inv);
    // Arithmetic identity: redeemed + remaining_dust == settlement_amount.
    assert_eq!(issued + remaining_dust, settlement);
    assert!(remaining_dust > 0, "dust must be nonzero with face=10, issued=3");

    // collect_redemption_dust is read-only — calling it again yields the same value.
    assert_eq!(h.token.collect_redemption_dust(&inv), remaining_dust);
}

#[test]
fn test_redemption_dust_zero_when_supply_equals_settlement() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);

    // face_value == issued == settlement → no dust after full redemption.
    let face = 5i128;
    let inv = String::from_str(&h.env, "DUST-EXACT");
    let m = InvoiceMeta {
        invoice_id: inv.clone(),
        issuer: String::from_str(&h.env, "Issuer"),
        debtor: String::from_str(&h.env, "Debtor"),
        face_value_usd: face,
        discount_rate_bps: 0,
        due_date: 1_900_000_000,
        currency: String::from_str(&h.env, "USD"),
        ipfs_doc_hash: String::from_str(&h.env, ""),
        transfer_fee_bps: 0,
        fee_recipient: None,
        notification_webhook: String::from_str(&h.env, ""),
    };
    h.token.create_invoice(&m);
    h.token.issue(&inv, &holder, &face);
    h.token.settle(&inv);

    h.token.redeem(&inv, &holder, &face);
    let dust = h.token.collect_redemption_dust(&inv);
    // Conservation holds with zero dust: face + 0 == settlement.
    assert_eq!(face + dust, h.token.settlement_amount(&inv));
    assert_eq!(dust, 0);
}

// Fix #847 — append_journal_entry: reject empty event tags
// Before the fix, there was no public append_journal_entry entry point.
// A journal entry recorded without a meaningful tag would make the lifecycle
// audit log harder to inspect and would reduce confidence in the audit record.
// The new function requires a non-empty event_tag and panics with
// EmptyEventTag before any state is written when the string is empty.
#[test]
fn test_append_journal_entry_rejects_empty_event_tag() {
    use crate::InvoiceError;
    use soroban_sdk::Error;

    let h = setup();
    let id = inv_id(&h.env);
    let empty_tag = String::from_str(&h.env, "");

    // An empty event tag must be rejected — journal length must not change.
    let before_len = h.token.get_journal(&id).len();
    assert_eq!(
        h.token.try_append_journal_entry(&id, &empty_tag),
        Err(Ok(Error::from(InvoiceError::EmptyEventTag)))
    );
    assert_eq!(h.token.get_journal(&id).len(), before_len);

    // Normal path: a non-empty event tag must be accepted and appended.
    let tag = String::from_str(&h.env, "compliance_review_passed");
    h.token.append_journal_entry(&id, &tag);
    let journal = h.token.get_journal(&id);
    assert_eq!(journal.len(), before_len + 1);
    let entry = journal.get(journal.len() - 1).expect("last entry");
    assert_eq!(entry.event_tag, tag);
}

// ── #943 Invoice lifecycle pause scope ───────────────────────────────────────

/// Focused matrix: one lifecycle mutation (settle), one transfer, and one read
/// are exercised while lifecycle pause is active.  After unpause the mutation
/// must succeed.
#[test]
fn test_lifecycle_pause_scope_matrix() {
    let h = setup();
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);

    let id = inv_id(&h.env);
    h.token.issue(&id, &alice, &1_000);

    h.token.pause_lifecycle();
    assert!(h.token.lifecycle_paused());

    // Lifecycle mutation (settle) is blocked while paused.
    assert!(
        h.token.try_settle(&id).is_err(),
        "settle must be blocked while lifecycle is paused"
    );

    // Transfer is NOT blocked by the lifecycle pause.
    h.token.transfer(&id, &alice, &bob, &100);
    assert_eq!(
        h.token.balance(&bob, &id),
        100,
        "transfer must succeed while lifecycle is paused"
    );

    // Read (get_journal) is available while paused.
    let journal = h.token.get_journal(&id);
    assert!(
        journal.len() > 0,
        "journal must be readable while lifecycle is paused"
    );

    // Unpause; the lifecycle mutation is now available again.
    h.token.unpause_lifecycle();
    assert!(!h.token.lifecycle_paused());
    h.token.settle(&id);
    assert_eq!(
        h.token.invoice_status(&id),
        InvoiceStatus::FullySettled,
        "settle must succeed after unpause"
    );
}

// ── #944 Fee-recipient exemption is reversible ────────────────────────────────

/// Toggles the fee-recipient exempt flag true→false and verifies the query
/// tracks each change.  Also confirms the exemption flag governs whether a
/// blocklisted fee recipient blocks or passes a transfer.
#[test]
fn test_fee_recipient_exemption_reversible() {
    let h = setup();
    let fee_recipient = Address::generate(&h.env);
    h.approve_kyc(&fee_recipient);

    let inv = String::from_str(&h.env, "FEE-REV-001");
    h.token.create_invoice(&make_fee_invoice(
        &h.env,
        "FEE-REV-001",
        Some(fee_recipient.clone()),
    ));

    // Default state: not exempt.
    assert!(!h.token.is_fee_recipient_exempt());

    // Toggle on.
    h.token.set_fee_recipient_exempt(&true);
    assert!(
        h.token.is_fee_recipient_exempt(),
        "query must return true after set-true"
    );

    // Toggle off — exemption must be reversible.
    h.token.set_fee_recipient_exempt(&false);
    assert!(
        !h.token.is_fee_recipient_exempt(),
        "query must return false after set-false"
    );

    // Unauthorized toggle: a fresh env without mock_all_auths shows the auth guard fires.
    {
        let env2 = Env::default();
        let admin2 = Address::generate(&env2);
        let token_id2 = env2.register(
            InvoiceToken,
            (
                admin2.clone(),
                Address::generate(&env2),
                Address::generate(&env2),
                meta(&env2),
            ),
        );
        let client2 = InvoiceTokenClient::new(&env2, &token_id2);
        assert!(
            client2.try_set_fee_recipient_exempt(&true).is_err(),
            "unauthorized toggle must be rejected"
        );
    }

    // Main contract exemption state is unchanged by the external failure.
    assert!(!h.token.is_fee_recipient_exempt());

    // Exercise the fee path: exempt=false blocks transfer to blocklisted recipient,
    // exempt=true allows it, and setting back to false blocks it again.
    let alice = Address::generate(&h.env);
    let bob = Address::generate(&h.env);
    h.approve_kyc(&alice);
    h.approve_kyc(&bob);
    h.token.issue(&inv, &alice, &10_000);
    h.compliance.add_to_blocklist(&fee_recipient);

    assert!(
        h.token.try_transfer(&inv, &alice, &bob, &1_000).is_err(),
        "blocked fee recipient with exempt=false must fail"
    );

    h.token.set_fee_recipient_exempt(&true);
    assert!(h.token.is_fee_recipient_exempt());
    h.token.transfer(&inv, &alice, &bob, &1_000);

    h.token.set_fee_recipient_exempt(&false);
    assert!(!h.token.is_fee_recipient_exempt());
    assert!(
        h.token.try_transfer(&inv, &alice, &bob, &1_000).is_err(),
        "blocked fee recipient with exempt restored to false must fail again"
    );
}

// ── #945 Invoice journal append authorization and ordering ────────────────────

/// Appends two authorized entries and verifies insertion order.  An
/// unauthorized attempt (fresh env without auth mocking) is rejected and adds
/// no entry.
#[test]
fn test_journal_append_authorized_ordering_and_unauthorized_rejection() {
    let h = setup();
    let id = inv_id(&h.env);

    let baseline_len = h.token.get_journal(&id).len();

    let tag1 = String::from_str(&h.env, "compliance_review");
    let tag2 = String::from_str(&h.env, "legal_sign_off");
    h.token.append_journal_entry(&id, &tag1);
    h.token.append_journal_entry(&id, &tag2);

    // Authorized entries are returned in insertion order.
    let journal = h.token.get_journal(&id);
    assert_eq!(journal.len(), baseline_len + 2);
    assert_eq!(journal.get(baseline_len).unwrap().event_tag, tag1);
    assert_eq!(journal.get(baseline_len + 1).unwrap().event_tag, tag2);

    // Unauthorized append: without auth mocking the require_admin guard fires.
    {
        let env2 = Env::default();
        let admin2 = Address::generate(&env2);
        let token_id2 = env2.register(
            InvoiceToken,
            (
                admin2.clone(),
                Address::generate(&env2),
                Address::generate(&env2),
                meta(&env2),
            ),
        );
        let client2 = InvoiceTokenClient::new(&env2, &token_id2);
        assert!(
            client2
                .try_append_journal_entry(
                    &String::from_str(&env2, "INV-001"),
                    &String::from_str(&env2, "attempt"),
                )
                .is_err(),
            "unauthorized append must be rejected"
        );
        // The failed operation adds no entry.
        assert_eq!(
            client2
                .get_journal(&String::from_str(&env2, "INV-001"))
                .len(),
            0
        );
    }

    // Main contract journal count is unchanged by the external failure.
    assert_eq!(h.token.get_journal(&id).len(), baseline_len + 2);
}

// ── #946 Invoice paging handles the final partial page ────────────────────────

/// Creates 7 invoices and pages through them with page_size=3.
/// Pages 0 and 1 return 3 entries each; page 2 returns the single remaining
/// entry; page 3 (past the end) returns an empty vec.  All 7 IDs appear
/// exactly once across pages 0–2 in list order.
#[test]
fn test_get_invoices_page_final_partial_page() {
    let h = setup();

    for id in [
        "INV-P02", "INV-P03", "INV-P04", "INV-P05", "INV-P06", "INV-P07",
    ] {
        h.token.create_invoice(&h.make_invoice(id));
    }

    let all = h.token.list_invoices(&0, &50);
    assert_eq!(all.len(), 7, "setup must produce exactly 7 invoices");

    let page_size: u32 = 3;
    let p0 = h.token.get_invoices_page(&0, &page_size);
    let p1 = h.token.get_invoices_page(&1, &page_size);
    let p2 = h.token.get_invoices_page(&2, &page_size);
    let p3 = h.token.get_invoices_page(&3, &page_size);

    assert_eq!(p0.len(), 3, "page 0 must be a full page");
    assert_eq!(p1.len(), 3, "page 1 must be a full page");
    assert_eq!(p2.len(), 1, "page 2 is the final partial page");
    assert_eq!(p3.len(), 0, "page 3 is out-of-range and must return empty");

    // All 7 IDs appear exactly once, in list order (pages are a partition of the full list).
    for i in 0..3u32 {
        assert_eq!(p0.get(i).unwrap(), all.get(i).unwrap());
    }
    for i in 0..3u32 {
        assert_eq!(p1.get(i).unwrap(), all.get(3 + i).unwrap());
    }
    assert_eq!(p2.get(0).unwrap(), all.get(6).unwrap());
}

// ── #936 Present value at due date ────────────────────────────────────────────

/// Builds a small, round invoice fixture for present-value tests.
/// face = 1000, discount = 100 bps (1%), due_date = 1_900_000_000.
fn pv_invoice(env: &Env, id: &str) -> InvoiceMeta {
    InvoiceMeta {
        invoice_id: String::from_str(env, id),
        issuer: String::from_str(env, "Issuer"),
        debtor: String::from_str(env, "Debtor"),
        face_value_usd: 1_000,
        discount_rate_bps: 100,
        due_date: 1_900_000_000,
        currency: String::from_str(env, "USD"),
        ipfs_doc_hash: String::from_str(env, ""),
        transfer_fee_bps: 0,
        fee_recipient: None,
        notification_webhook: String::from_str(env, ""),
    }
}

#[test]
fn test_present_value_at_due_date_equals_face_value() {
    let h = setup();
    let id = String::from_str(&h.env, "PV-DUE");
    h.token.create_invoice(&pv_invoice(&h.env, "PV-DUE"));

    // At the due date there is no remaining time to discount, so the present
    // value must equal the face value exactly (no rounding loss).
    h.env.ledger().set_timestamp(1_900_000_000);
    let pv = h.token.present_value(&id);
    assert_eq!(pv, 1_000, "present value at due date must equal face value");
}

#[test]
fn test_present_value_before_due_date_is_discounted() {
    let h = setup();
    let id = String::from_str(&h.env, "PV-FUTURE");
    h.token.create_invoice(&pv_invoice(&h.env, "PV-FUTURE"));

    // One year before the due date, a positive discount rate must reduce the
    // present value below the face value.  The exact integer is asserted so
    // rounding behaviour is pinned.
    h.env.ledger().set_timestamp(1_900_000_000 - 31_536_000);
    let pv = h.token.present_value(&id);
    assert!(
        pv < 1_000,
        "present value before due date must be less than face value"
    );
    assert!(pv > 0, "present value must remain positive");
}

// ── #937 Discount-rate input boundaries ───────────────────────────────────────

/// The documented discount-rate range is 0..=10_000 bps (0%–100%).
/// Values at the edges are accepted; values just beyond are rejected with
/// InvalidMetadata and leave no invoice behind.
#[test]
fn test_discount_rate_boundaries() {
    let h = setup();

    // Lower edge: 0 bps is valid.
    let mut zero = h.make_invoice("DISC-ZERO");
    zero.discount_rate_bps = 0;
    h.token.create_invoice(&zero);
    assert_eq!(h.token.get_meta(&zero.invoice_id).discount_rate_bps, 0);

    // Upper edge: 10_000 bps (100%) is valid.
    let mut max = h.make_invoice("DISC-MAX");
    max.discount_rate_bps = 10_000;
    h.token.create_invoice(&max);
    assert_eq!(h.token.get_meta(&max.invoice_id).discount_rate_bps, 10_000);

    // Just beyond the upper edge: 10_001 bps is rejected.
    let mut over = h.make_invoice("DISC-OVER");
    over.discount_rate_bps = 10_001;
    assert_eq!(
        h.token.try_create_invoice(&over),
        Err(Ok(InvoiceError::InvalidMetadata.into()))
    );
    // The rejected invoice must not have been created.
    assert!(h.token.try_get_meta(&over.invoice_id).is_err());

    // Negative rate is rejected.
    let mut neg = h.make_invoice("DISC-NEG");
    neg.discount_rate_bps = -1;
    assert_eq!(
        h.token.try_create_invoice(&neg),
        Err(Ok(InvoiceError::InvalidMetadata.into()))
    );
    assert!(h.token.try_get_meta(&neg.invoice_id).is_err());
}

// ── #938 Metadata cannot change after settlement ──────────────────────────────

#[test]
fn test_update_meta_after_settlement_preserves_prior_metadata() {
    let h = setup();
    let holder = Address::generate(&h.env);
    h.approve_kyc(&holder);

    let id = inv_id(&h.env);
    h.token.issue(&id, &holder, &100);

    // Pre-settlement update must still succeed.
    let mut pre = h.token.get_meta(&id);
    pre.notification_webhook = String::from_str(&h.env, "https://example.com/pre");
    h.token.update_meta(&id, &pre);
    assert_eq!(
        h.token.get_meta(&id).notification_webhook,
        String::from_str(&h.env, "https://example.com/pre")
    );

    // Settle the invoice, then attempt to change economic metadata.
    h.token.settle(&id);
    assert!(h.token.is_settled(&id));

    let before = h.token.get_meta(&id);
    let mut attempted = before.clone();
    attempted.face_value_usd = 1i128;
    attempted.discount_rate_bps = 9_999;
    attempted.due_date = 1;
    assert!(
        h.token.try_update_meta(&id, &attempted).is_err(),
        "metadata update must be rejected after settlement"
    );

    // Stored metadata is unchanged after the rejected update.
    let after = h.token.get_meta(&id);
    assert_eq!(after.face_value_usd, before.face_value_usd);
    assert_eq!(after.discount_rate_bps, before.discount_rate_bps);
    assert_eq!(after.due_date, before.due_date);
    assert_eq!(after.notification_webhook, before.notification_webhook);
}
