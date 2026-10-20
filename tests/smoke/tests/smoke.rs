//! Deployment smoke tests — one per contract target.
//!
//! Each test instantiates a contract in the Soroban test environment and
//! exercises the minimum set of core entry points to confirm the contract
//! initialises correctly and that its primary read/write paths respond as
//! expected. These are intentionally lightweight; comprehensive behaviour
//! coverage lives in the per-contract unit test suites.
//!
//! Expected outcomes
//! -----------------
//! | Contract             | Failure signature                             |
//! |----------------------|-----------------------------------------------|
//! | kyc-registry         | `is_approved` returns false after approve, or  |
//! |                      | `verifier_count` is not 1 after add_verifier   |
//! | compliance-engine    | `can_transfer` returns wrong bool, or rules    |
//! |                      | struct has unexpected defaults                 |
//! | rwa-token            | name/symbol/decimals/total_supply mismatch     |
//! | property-token       | name/symbol/decimals/total_shares/meta wrong   |
//! | invoice-token        | `list_invoices` does not return the seed entry |
//! | carbon-credit-token  | `total_supply` or `total_retired` not zero     |

extern crate alloc;

use soroban_sdk::{testutils::Address as _, Address, Env, String};

use carbon_credit_token::{CarbonCreditToken, CarbonCreditTokenClient, ProjectMeta};
use compliance_engine::{ComplianceEngine, ComplianceEngineClient};
use invoice_token::{InvoiceMeta, InvoiceToken, InvoiceTokenClient};
use kyc_registry::{KycRegistry, KycRegistryClient};
use property_token::{PropertyMeta, PropertyToken, PropertyTokenClient};
use rwa_token::{ComplianceMetadata, RwaToken, RwaTokenClient};

// ── Helpers ───────────────────────────────────────────────────────────────────

fn deploy_kyc(env: &Env, admin: &Address) -> KycRegistryClient<'static> {
    let id = env.register(KycRegistry, ());
    let client = KycRegistryClient::new(env, &id);
    client.initialize(admin);
    client
}

fn deploy_compliance(
    env: &Env,
    admin: &Address,
    kyc_id: &Address,
) -> ComplianceEngineClient<'static> {
    let id = env.register(ComplianceEngine, ());
    let client = ComplianceEngineClient::new(env, &id);
    client.initialize(admin, kyc_id, &0u64);
    client
}

// ── KYC Registry ─────────────────────────────────────────────────────────────

#[test]
fn smoke_kyc_registry() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);

    let kyc = deploy_kyc(&env, &admin);
    assert_eq!(kyc.verifier_count(), 0);

    let verifier = Address::generate(&env);
    kyc.add_verifier(&admin, &verifier);
    assert_eq!(kyc.verifier_count(), 1);

    let subject = Address::generate(&env);
    assert!(!kyc.is_approved(&subject));

    kyc.approve(&verifier, &subject, &0, &0, &String::from_str(&env, "US"));
    assert!(kyc.is_approved(&subject));
    assert_eq!(kyc.get_lifecycle_count(&subject), 1);

    kyc.revoke(&verifier, &subject);
    assert!(!kyc.is_approved(&subject));
    assert_eq!(kyc.get_lifecycle_count(&subject), 2);
}

// ── Compliance Engine ─────────────────────────────────────────────────────────

#[test]
fn smoke_compliance_engine() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);

    let kyc_id = env.register(KycRegistry, ());
    let kyc = KycRegistryClient::new(&env, &kyc_id);
    kyc.initialize(&admin);

    let ce = deploy_compliance(&env, &admin, &kyc_id);

    let rules = ce.get_rules();
    assert!(!rules.paused);
    assert_eq!(rules.max_transfer_amount, 0);
    assert_eq!(rules.max_holders, 0);

    let from = Address::generate(&env);
    let to = Address::generate(&env);
    assert!(ce.can_transfer(&from, &to, &1_000));

    ce.pause();
    assert!(!ce.can_transfer(&from, &to, &1));

    ce.unpause();
    assert!(ce.can_transfer(&from, &to, &1_000));
}

// ── RWA Token ─────────────────────────────────────────────────────────────────

#[test]
fn smoke_rwa_token() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);

    let kyc_id = env.register(KycRegistry, ());
    let kyc = KycRegistryClient::new(&env, &kyc_id);
    kyc.initialize(&admin);

    let ce_id = env.register(ComplianceEngine, ());
    let ce = ComplianceEngineClient::new(&env, &ce_id);
    ce.initialize(&admin, &kyc_id, &0u64);

    let token_id = env.register(
        RwaToken,
        (
            admin.clone(),
            7u32,
            String::from_str(&env, "Veritoken RWA"),
            String::from_str(&env, "VTRWA"),
            String::from_str(&env, "property"),
            kyc_id.clone(),
            ce_id.clone(),
            Option::<ComplianceMetadata>::None,
            0i128,
        ),
    );
    let token = RwaTokenClient::new(&env, &token_id);

    assert_eq!(token.name(), String::from_str(&env, "Veritoken RWA"));
    assert_eq!(token.symbol(), String::from_str(&env, "VTRWA"));
    assert_eq!(token.decimals(), 7);
    assert_eq!(token.total_supply(), 0);
    assert_eq!(token.asset_type(), String::from_str(&env, "property"));
    assert_eq!(token.kyc_registry(), kyc_id);
    assert_eq!(token.compliance_engine(), ce_id);
    assert!(!token.version().is_empty());
}

// ── Property Token ────────────────────────────────────────────────────────────

#[test]
fn smoke_property_token() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);

    let kyc_id = env.register(KycRegistry, ());
    let kyc = KycRegistryClient::new(&env, &kyc_id);
    kyc.initialize(&admin);

    let ce_id = env.register(ComplianceEngine, ());
    let ce = ComplianceEngineClient::new(&env, &ce_id);
    ce.initialize(&admin, &kyc_id, &0u64);

    let meta = PropertyMeta {
        property_id: String::from_str(&env, "PROP-SMOKE-001"),
        legal_name: String::from_str(&env, "Smoke Test LLC"),
        jurisdiction: String::from_str(&env, "US-NY"),
        address: String::from_str(&env, "1 Test Ave"),
        total_valuation_usd: 1_000_000_000_000,
        total_shares: 1_000,
        property_type: String::from_str(&env, "commercial"),
        ipfs_title_hash: String::from_str(&env, "QmNLei78zWmzUdbeRB3CiUfAizWUrbeeZh5K1rhAQKCh8L"),
        kyc_tier_required: 0,
    };

    let token_id = env.register(
        PropertyToken,
        (admin.clone(), kyc_id.clone(), ce_id.clone(), meta.clone()),
    );
    let token = PropertyTokenClient::new(&env, &token_id);

    assert_eq!(token.name(), String::from_str(&env, "Veritoken Property"));
    assert_eq!(token.symbol(), String::from_str(&env, "VTPROP"));
    assert_eq!(token.decimals(), 0);
    assert_eq!(token.total_shares(), 1_000);

    let stored = token.get_meta();
    assert_eq!(stored.property_id, String::from_str(&env, "PROP-SMOKE-001"));
    assert_eq!(stored.total_shares, 1_000);
    assert_eq!(stored.property_type, String::from_str(&env, "commercial"));
}

// ── Invoice Token ─────────────────────────────────────────────────────────────

#[test]
fn smoke_invoice_token() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);

    let kyc_id = env.register(KycRegistry, ());
    let kyc = KycRegistryClient::new(&env, &kyc_id);
    kyc.initialize(&admin);

    let ce_id = env.register(ComplianceEngine, ());
    let ce = ComplianceEngineClient::new(&env, &ce_id);
    ce.initialize(&admin, &kyc_id, &0u64);

    let meta = InvoiceMeta {
        invoice_id: String::from_str(&env, "SMOKE-INV-001"),
        issuer: String::from_str(&env, "Smoke Corp"),
        debtor: String::from_str(&env, "Test Ltd"),
        face_value_usd: 500_000_000_000,
        discount_rate_bps: 200,
        due_date: 1_900_000_000,
        currency: String::from_str(&env, "USD"),
        ipfs_doc_hash: String::from_str(&env, "QmNLei78zWmzUdbeRB3CiUfAizWUrbeeZh5K1rhAQKCh8L"),
        transfer_fee_bps: 0,
        fee_recipient: None,
        notification_webhook: String::from_str(&env, ""),
    };

    let token_id = env.register(
        InvoiceToken,
        (admin.clone(), kyc_id.clone(), ce_id.clone(), meta),
    );
    let token = InvoiceTokenClient::new(&env, &token_id);

    let ids = token.list_invoices(&0, &10);
    assert_eq!(ids.len(), 1);

    let stored = token.get_meta(&String::from_str(&env, "SMOKE-INV-001"));
    assert_eq!(stored.issuer, String::from_str(&env, "Smoke Corp"));
    assert_eq!(stored.face_value_usd, 500_000_000_000);
}

#[test]
fn smoke_invoice_lifecycle() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);

    let kyc = deploy_kyc(&env, &admin);
    let kyc_id = kyc.address.clone();
    let ce = deploy_compliance(&env, &admin, &kyc_id);
    let ce_id = ce.address.clone();

    let meta = InvoiceMeta {
        invoice_id: String::from_str(&env, "SMOKE-INV-LIFECYCLE"),
        issuer: String::from_str(&env, "Lifecycle Corp"),
        debtor: String::from_str(&env, "Lifecycle Ltd"),
        face_value_usd: 1_000_000_000_000,
        discount_rate_bps: 200,
        due_date: 1_900_000_000,
        currency: String::from_str(&env, "USD"),
        ipfs_doc_hash: String::from_str(&env, "QmNLei78zWmzUdbeRB3CiUfAizWUrbeeZh5K1rhAQKCh8L"),
        transfer_fee_bps: 0,
        fee_recipient: None,
        notification_webhook: String::from_str(&env, ""),
    };

    let token_id = env.register(
        InvoiceToken,
        (admin.clone(), kyc_id.clone(), ce_id.clone(), meta),
    );
    let token = InvoiceTokenClient::new(&env, &token_id);

    // Invoice is created and listed.
    let invoice_id = String::from_str(&env, "SMOKE-INV-LIFECYCLE");
    let ids = token.list_invoices(&0, &10);
    assert_eq!(ids.len(), 1);
    assert_eq!(token.total_supply(), 0);

    // Approve the holder and issue a balance.
    let verifier = Address::generate(&env);
    kyc.add_verifier(&admin, &verifier);
    let holder = Address::generate(&env);
    kyc.approve(&verifier, &holder, &0, &0, &String::from_str(&env, "US"));
    assert!(kyc.is_approved(&holder));

    let issued: i128 = 1_000;
    token.issue(&admin, &holder, &invoice_id, &issued);
    assert_eq!(token.total_supply(), issued);
    assert_eq!(token.balance(&holder), issued);

    // Partially settle the invoice.
    let settled: i128 = 400;
    token.settle(&admin, &invoice_id, &settled);
    assert_eq!(token.total_supply(), issued - settled);

    // Redeem a portion of the remaining balance.
    let redeemed: i128 = 200;
    token.redeem(&holder, &invoice_id, &redeemed);
    assert_eq!(token.balance(&holder), issued - settled - redeemed);
    assert_eq!(
        token.total_supply(),
        issued - settled - redeemed
    );

    // Final reconciliation: supply equals outstanding holder balance.
    assert_eq!(token.total_supply(), token.balance(&holder));
}

// ── Carbon Credit Token ───────────────────────────────────────────────────────

#[test]
fn smoke_carbon_credit_token() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);

    let kyc_id = env.register(KycRegistry, ());
    let kyc = KycRegistryClient::new(&env, &kyc_id);
    kyc.initialize(&admin);

    let ce_id = env.register(ComplianceEngine, ());
    let ce = ComplianceEngineClient::new(&env, &ce_id);
    ce.initialize(&admin, &kyc_id, &0u64);

    let meta = ProjectMeta {
        project_id: String::from_str(&env, "SMOKE-VCS-001"),
        standard: String::from_str(&env, "VCS"),
        vintage_year: 2024,
        project_name: String::from_str(&env, "Smoke Reforestation"),
        project_type: String::from_str(&env, "forestry"),
        country: String::from_str(&env, "BR"),
        verifier: String::from_str(&env, "Verra"),
        ipfs_cert_hash: String::from_str(&env, "QmNLei78zWmzUdbeRB3CiUfAizWUrbeeZh5K1rhAQKCh8L"),
        registry_url: String::from_str(&env, "https://registry.verra.org"),
        registry_project_id: String::from_str(&env, "VCS-001"),
    };

    let token_id = env.register(
        CarbonCreditToken,
        (admin.clone(), kyc_id.clone(), ce_id.clone(), meta),
    );
    let token = CarbonCreditTokenClient::new(&env, &token_id);

    assert_eq!(
        token.name(),
        String::from_str(&env, "Veritoken Carbon Credit")
    );
    assert_eq!(token.total_supply(), 0);
    assert_eq!(token.total_retired(), 0);

    let stored = token.get_meta();
    assert_eq!(stored.project_id, String::from_str(&env, "SMOKE-VCS-001"));
    assert_eq!(stored.standard, String::from_str(&env, "VCS"));
    assert_eq!(stored.vintage_year, 2024);
}
