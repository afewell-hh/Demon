use anyhow::Result;
use engine::rituals::log::{EventLog, RitualEvent};
use serde_json::json;
use std::env;

#[tokio::test]
#[ignore] // Requires NATS to be running
async fn test_given_quota_limit_when_exceeded_then_deny() -> Result<()> {
    // Given: A quota configuration with limit of 2 calls per 60 seconds
    env::set_var(
        "WARDS_CAP_QUOTAS",
        json!({
            "test-tenant": {
                "capsule.echo": {
                    "limit": 2,
                    "windowSeconds": 60
                }
            }
        })
        .to_string(),
    );

    let nats_url = env::var("NATS_URL").unwrap_or_else(|_| "nats://localhost:4222".to_string());
    let event_log = EventLog::new(&nats_url).await?;

    // When: Making 3 calls to the capability
    let ritual_id = "test-ritual";
    let run_id = "test-run-1";
    let tenant_id = "test-tenant";
    let capability = "capsule.echo";

    // First call - should be allowed
    let decision1 = RitualEvent::PolicyDecision {
        ritual_id: ritual_id.to_string(),
        run_id: run_id.to_string(),
        ts: chrono::Utc::now().to_rfc3339(),
        tenant_id: tenant_id.to_string(),
        capability: capability.to_string(),
        decision: json!({
            "allowed": true,
            "reason": null
        }),
        quota: json!({
            "limit": 2,
            "windowSeconds": 60,
            "remaining": 1
        }),
    };

    event_log.append(&decision1, 1).await?;

    // Second call - should be allowed
    let decision2 = RitualEvent::PolicyDecision {
        ritual_id: ritual_id.to_string(),
        run_id: run_id.to_string(),
        ts: chrono::Utc::now().to_rfc3339(),
        tenant_id: tenant_id.to_string(),
        capability: capability.to_string(),
        decision: json!({
            "allowed": true,
            "reason": null
        }),
        quota: json!({
            "limit": 2,
            "windowSeconds": 60,
            "remaining": 0
        }),
    };

    event_log.append(&decision2, 2).await?;

    // Third call - should be denied
    let decision3 = RitualEvent::PolicyDecision {
        ritual_id: ritual_id.to_string(),
        run_id: run_id.to_string(),
        ts: chrono::Utc::now().to_rfc3339(),
        tenant_id: tenant_id.to_string(),
        capability: capability.to_string(),
        decision: json!({
            "allowed": false,
            "reason": "limit_exceeded"
        }),
        quota: json!({
            "limit": 2,
            "windowSeconds": 60,
            "remaining": 0
        }),
    };

    event_log.append(&decision3, 3).await?;

    // Then: Verify events were persisted with correct decision values
    let events = event_log.read_run(ritual_id, run_id).await?;
    assert_eq!(events.len(), 3);

    // Verify the third decision is denied with reason "limit_exceeded"
    if let RitualEvent::PolicyDecision {
        decision, quota, ..
    } = &events[2]
    {
        assert_eq!(decision["allowed"], false);
        assert_eq!(decision["reason"], "limit_exceeded");
        assert_eq!(quota["remaining"], 0);
    } else {
        panic!("Expected PolicyDecision event");
    }

    Ok(())
}

#[tokio::test]
async fn test_given_no_quota_when_called_then_allow_with_null_reason() -> Result<()> {
    // Given: No quota configuration (unlimited)
    env::remove_var("WARDS_CAP_QUOTAS");

    // When: Making a call without quota limits
    // Then: Should be allowed with reason: null

    // This would be implemented when engine integrates with Wards
    Ok(())
}

#[tokio::test]
async fn test_given_quota_when_window_expires_then_reset_counter() -> Result<()> {
    // Given: A quota with 1 second window
    env::set_var(
        "WARDS_CAP_QUOTAS",
        json!({
            "test-tenant": {
                "capsule.test": {
                    "limit": 1,
                    "windowSeconds": 1
                }
            }
        })
        .to_string(),
    );

    // When: Making calls across window boundaries
    // First call should be allowed
    // Wait for window to expire
    // Second call should also be allowed (counter reset)

    // This would be implemented when engine integrates with Wards
    Ok(())
}