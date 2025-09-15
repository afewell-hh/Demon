use wards::config::{load_from_env, QuotaCfg};
use serial_test::serial;

#[test]
#[serial]
fn compact_parser_supports_global_and_tenant_overrides() {
    // GLOBAL fallback + TENANT override for acme
    std::env::set_var(
        "WARDS_CAP_QUOTAS",
        "GLOBAL:capsule.echo=5:60,TENANT:acme:capsule.echo=2:60",
    );

    let cfg = load_from_env();

    // Tenant override applies
    let q_acme = cfg.effective_quota("acme", "capsule.echo");
    assert_eq!(
        q_acme,
        QuotaCfg {
            limit: 2,
            window_seconds: 60
        }
    );

    // Other tenants inherit GLOBAL cap quota
    let q_other = cfg.effective_quota("other", "capsule.echo");
    assert_eq!(
        q_other,
        QuotaCfg {
            limit: 5,
            window_seconds: 60
        }
    );

    // Unknown cap falls back to defaults (0/60) when no WARDS_GLOBAL_QUOTA
    let q_unknown = cfg.effective_quota("other", "capsule.unknown");
    assert_eq!(
        q_unknown,
        QuotaCfg {
            limit: 0,
            window_seconds: 60
        }
    );

    std::env::remove_var("WARDS_CAP_QUOTAS");
}

#[test]
#[serial]
#[should_panic]
fn compact_parser_rejects_malformed_entry() {
    std::env::set_var("WARDS_CAP_QUOTAS", "GLOBAL:capsule.echo=bad");
    // load_from_env should panic due to invalid format
    let _ = load_from_env();
}
