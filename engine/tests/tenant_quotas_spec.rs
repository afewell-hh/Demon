use serial_test::serial;
use wards::config::{load_from_env, QuotaCfg, WardsConfig};
use wards::policy::PolicyKernel;

#[test]
#[serial]
fn given_two_tenants_when_enabled_then_counters_are_independent() {
    std::env::set_var("TENANTING_ENABLED", "1");
    std::env::set_var(
        "WARDS_CAP_QUOTAS",
        r#"{"A":{"capsule.echo":{"limit":2,"windowSeconds":60}},"B":{"capsule.echo":{"limit":2,"windowSeconds":60}}}"#,
    );

    let cfg: WardsConfig = load_from_env();
    let mut kernel = PolicyKernel::new(cfg);

    // Tenant A consumes its 2 tokens
    assert!(kernel.allow_and_count("A", "capsule.echo").allowed);
    assert!(kernel.allow_and_count("A", "capsule.echo").allowed);
    assert!(!kernel.allow_and_count("A", "capsule.echo").allowed);

    // Tenant B is unaffected and still has 2 tokens
    assert!(kernel.allow_and_count("B", "capsule.echo").allowed);
    assert!(kernel.allow_and_count("B", "capsule.echo").allowed);
    assert!(!kernel.allow_and_count("B", "capsule.echo").allowed);

    std::env::remove_var("WARDS_CAP_QUOTAS");
    std::env::remove_var("TENANTING_ENABLED");
}

#[test]
#[serial]
fn given_tenancy_disabled_then_global_counter_is_used() {
    std::env::set_var("TENANTING_ENABLED", "0");
    // Global cap quota: applies regardless of tenant when tenancy disabled
    std::env::set_var("WARDS_CAP_QUOTAS", "GLOBAL:capsule.echo=2:60");

    let cfg = load_from_env();
    let mut kernel = PolicyKernel::new(cfg);

    // Calls from different tenants share the same capability counter
    assert!(kernel.allow_and_count("A", "capsule.echo").allowed);
    assert!(kernel.allow_and_count("B", "capsule.echo").allowed);
    // Third call, regardless of tenant, should be denied
    assert!(!kernel.allow_and_count("A", "capsule.echo").allowed);

    std::env::remove_var("WARDS_CAP_QUOTAS");
    std::env::remove_var("TENANTING_ENABLED");
}

#[test]
#[serial]
fn given_compact_overrides_then_acme_uses_override_and_others_use_global() {
    std::env::set_var("TENANTING_ENABLED", "1");
    std::env::set_var(
        "WARDS_CAP_QUOTAS",
        "GLOBAL:capsule.echo=5:60,TENANT:acme:capsule.echo=2:60",
    );

    let cfg = load_from_env();

    let q_acme = cfg.effective_quota("acme", "capsule.echo");
    assert_eq!(
        q_acme,
        QuotaCfg {
            limit: 2,
            window_seconds: 60
        }
    );

    let q_other = cfg.effective_quota("other", "capsule.echo");
    assert_eq!(
        q_other,
        QuotaCfg {
            limit: 5,
            window_seconds: 60
        }
    );

    std::env::remove_var("WARDS_CAP_QUOTAS");
    std::env::remove_var("TENANTING_ENABLED");
}
