use anyhow::Result;
use engine::rituals::{log::EventLog, Engine};
use std::env;

#[tokio::test]
#[ignore] // Requires NATS to be running
async fn test_tenant_isolation_engine_publishing() -> Result<()> {
    // Setup: Configure tenanting
    env::set_var("TENANTING_ENABLED", "1");
    env::set_var("TENANT_DEFAULT", "default");

    let nats_url = env::var("NATS_URL").unwrap_or_else(|_| "nats://localhost:4222".to_string());

    // Initialize engine with EventLog
    let mut engine = Engine::new().with_event_log(&nats_url).await?;

    // Test 1: Run ritual with tenant "tenant-a"
    let temp_ritual_a = create_test_ritual("test-ritual-a")?;
    engine
        .run_from_file_with_tenant(&temp_ritual_a, Some("tenant-a"))
        .await?;

    // Test 2: Run ritual with tenant "tenant-b"
    let temp_ritual_b = create_test_ritual("test-ritual-b")?;
    engine
        .run_from_file_with_tenant(&temp_ritual_b, Some("tenant-b"))
        .await?;

    // Test 3: Run ritual with default tenant
    let temp_ritual_default = create_test_ritual("test-ritual-default")?;
    engine
        .run_from_file_with_tenant(&temp_ritual_default, None)
        .await?;

    // Verification: Read events for each tenant and ensure isolation
    let event_log = EventLog::new(&nats_url).await?;

    // Check tenant-a events
    let events_a = event_log
        .read_run_with_tenant("test-ritual-a", "test-ritual-a", Some("tenant-a"))
        .await?;
    assert!(!events_a.is_empty(), "Tenant A should have events");

    // Check tenant-b events
    let events_b = event_log
        .read_run_with_tenant("test-ritual-b", "test-ritual-b", Some("tenant-b"))
        .await?;
    assert!(!events_b.is_empty(), "Tenant B should have events");

    // Check default tenant events
    let events_default = event_log
        .read_run_with_tenant(
            "test-ritual-default",
            "test-ritual-default",
            Some("default"),
        )
        .await?;
    assert!(
        !events_default.is_empty(),
        "Default tenant should have events"
    );

    // Cross-tenant isolation: tenant-a should not see tenant-b events
    let cross_check_a_to_b = event_log
        .read_run_with_tenant("test-ritual-b", "test-ritual-b", Some("tenant-a"))
        .await?;
    assert!(
        cross_check_a_to_b.is_empty(),
        "Tenant A should not see tenant B events"
    );

    let cross_check_b_to_a = event_log
        .read_run_with_tenant("test-ritual-a", "test-ritual-a", Some("tenant-b"))
        .await?;
    assert!(
        cross_check_b_to_a.is_empty(),
        "Tenant B should not see tenant A events"
    );

    // Clean up temp files
    std::fs::remove_file(&temp_ritual_a)?;
    std::fs::remove_file(&temp_ritual_b)?;
    std::fs::remove_file(&temp_ritual_default)?;

    Ok(())
}

#[tokio::test]
#[ignore] // Requires NATS to be running
async fn test_tenant_dual_publish_mode() -> Result<()> {
    // Setup: Configure tenanting with dual publish
    env::set_var("TENANTING_ENABLED", "1");
    env::set_var("TENANT_DEFAULT", "default");
    env::set_var("TENANT_DUAL_PUBLISH", "1");

    let nats_url = env::var("NATS_URL").unwrap_or_else(|_| "nats://localhost:4222".to_string());

    // Initialize engine with EventLog
    let mut engine = Engine::new().with_event_log(&nats_url).await?;
    let event_log = EventLog::new(&nats_url).await?;

    // Run ritual with tenant
    let temp_ritual = create_test_ritual("dual-publish-test")?;
    engine
        .run_from_file_with_tenant(&temp_ritual, Some("tenant-dual"))
        .await?;

    // Should be able to read from both tenant-scoped and legacy subjects
    let tenant_events = event_log
        .read_run_with_tenant(
            "dual-publish-test",
            "dual-publish-test",
            Some("tenant-dual"),
        )
        .await?;
    assert!(
        !tenant_events.is_empty(),
        "Should have events in tenant-scoped subject"
    );

    let legacy_events = event_log
        .read_run("dual-publish-test", "dual-publish-test")
        .await?;
    assert!(
        !legacy_events.is_empty(),
        "Should have events in legacy subject"
    );

    // Events should match in content (ignoring subject differences)
    assert_eq!(
        tenant_events.len(),
        legacy_events.len(),
        "Event counts should match between tenant and legacy subjects"
    );

    // Clean up
    std::fs::remove_file(&temp_ritual)?;
    env::remove_var("TENANT_DUAL_PUBLISH");

    Ok(())
}

#[tokio::test]
#[ignore] // Requires NATS to be running
async fn test_tenant_disabled_fallback() -> Result<()> {
    // Setup: Disable tenanting
    env::set_var("TENANTING_ENABLED", "0");

    let nats_url = env::var("NATS_URL").unwrap_or_else(|_| "nats://localhost:4222".to_string());

    // Initialize engine with EventLog
    let mut engine = Engine::new().with_event_log(&nats_url).await?;
    let event_log = EventLog::new(&nats_url).await?;

    // Run ritual with different tenants - should all use legacy subjects
    let temp_ritual_1 = create_test_ritual("disabled-test-1")?;
    engine
        .run_from_file_with_tenant(&temp_ritual_1, Some("any-tenant"))
        .await?;

    let temp_ritual_2 = create_test_ritual("disabled-test-2")?;
    engine
        .run_from_file_with_tenant(&temp_ritual_2, None)
        .await?;

    // All events should be readable via legacy subjects regardless of tenant parameter
    let events_1 = event_log
        .read_run("disabled-test-1", "disabled-test-1")
        .await?;
    assert!(!events_1.is_empty(), "Should have events in legacy subject");

    let events_2 = event_log
        .read_run("disabled-test-2", "disabled-test-2")
        .await?;
    assert!(!events_2.is_empty(), "Should have events in legacy subject");

    // Clean up
    std::fs::remove_file(&temp_ritual_1)?;
    std::fs::remove_file(&temp_ritual_2)?;

    Ok(())
}

fn create_test_ritual(ritual_id: &str) -> Result<String> {
    let ritual_content = format!(
        r#"
id: {}
version: '1.0'
name: Test Ritual for Tenant Isolation
description: A test ritual for verifying tenant isolation
states:
  - name: echo_task
    type: task
    action:
      functionRef:
        refName: echo
        arguments:
          message: "Hello from {}"
    end: true
"#,
        ritual_id, ritual_id
    );

    let temp_file = format!("/tmp/test_ritual_{}.yaml", ritual_id);
    std::fs::write(&temp_file, ritual_content)?;
    Ok(temp_file)
}

#[cfg(test)]
mod setup_teardown {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn setup_test_environment() {
        // This test can be run to setup the environment if needed
        env::set_var("TENANTING_ENABLED", "1");
        env::set_var("TENANT_DEFAULT", "default");
        println!("Test environment configured for tenant isolation tests");
    }

    #[tokio::test]
    #[ignore]
    async fn cleanup_test_environment() {
        // Cleanup any test environment variables
        env::remove_var("TENANTING_ENABLED");
        env::remove_var("TENANT_DEFAULT");
        env::remove_var("TENANT_DUAL_PUBLISH");
        println!("Test environment cleaned up");
    }
}
