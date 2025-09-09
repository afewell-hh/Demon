use std::collections::HashMap;

use wards::config::{load_from_env, QuotaCfg, WardsConfig};

#[test]
fn precedence_cap_overrides_tenant_and_global() {
    std::env::set_var(
        "WARDS_CAP_QUOTAS",
        r#"{
          "tenant-a": {
            "capsule.http": { "limit": 1, "windowSeconds": 60 },
            "capsule.echo": { "limit": 5, "windowSeconds": 60 }
          },
          "tenant-b": {
            "capsule.echo": { "limit": 2, "windowSeconds": 30 }
          }
        }"#,
    );
    std::env::set_var(
        "WARDS_QUOTAS",
        r#"{ "tenant-a": { "limit": 2, "windowSeconds": 60 },
              "tenant-b": { "limit": 10, "windowSeconds": 60 } }"#,
    );
    std::env::set_var(
        "WARDS_GLOBAL_QUOTA",
        r#"{ "limit": 100, "windowSeconds": 300 }"#,
    );

    let cfg = load_from_env();

    let q = cfg.effective_quota("tenant-a", "capsule.http");
    assert_eq!(q, QuotaCfg { limit: 1, window_seconds: 60 });

    let q = cfg.effective_quota("tenant-a", "capsule.unknown");
    assert_eq!(q, QuotaCfg { limit: 2, window_seconds: 60 });

    let q = cfg.effective_quota("unknown", "capsule.any");
    assert_eq!(q, QuotaCfg { limit: 100, window_seconds: 300 });

    std::env::remove_var("WARDS_CAP_QUOTAS");
    std::env::remove_var("WARDS_QUOTAS");
    std::env::remove_var("WARDS_GLOBAL_QUOTA");
    let cfg2 = load_from_env();
    let q = cfg2.effective_quota("t", "c");
    assert_eq!(q, QuotaCfg { limit: 0, window_seconds: 60 });
}

#[test]
fn two_caps_independent_counters_property() {
    let mut cfg = WardsConfig::default();
    let mut tmap: HashMap<String, QuotaCfg> = HashMap::new();
    tmap.insert(
        "capsule.echo".into(),
        QuotaCfg {
            limit: 2,
            window_seconds: 60,
        },
    );
    tmap.insert(
        "capsule.http".into(),
        QuotaCfg {
            limit: 1,
            window_seconds: 60,
        },
    );
    cfg.cap_quotas.insert("tenant-x".into(), tmap);

    let mut kernel = wards::policy::PolicyKernel::new(cfg);

    let mut ok = 0;
    for _ in 0..3 {
        if kernel.allow_and_count("tenant-x", "capsule.echo").allowed {
            ok += 1;
        }
    }
    assert_eq!(ok, 2);

    let mut ok2 = 0;
    for _ in 0..2 {
        if kernel.allow_and_count("tenant-x", "capsule.http").allowed {
            ok2 += 1;
        }
    }
    assert_eq!(ok2, 1);
}

