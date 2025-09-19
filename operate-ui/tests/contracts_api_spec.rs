use axum::http::StatusCode;
use axum_test::TestServer;
use operate_ui::{create_app, AppState};
use serde_json::json;

async fn setup_test_server() -> TestServer {
    let state = AppState::new().await;
    let app = create_app(state);
    TestServer::new(app).unwrap()
}

#[tokio::test]
async fn given_valid_envelope_when_post_validate_then_returns_valid_response() {
    let server = setup_test_server().await;

    let envelope = json!({
        "result": {
            "success": true,
            "data": "test"
        }
    });

    let response = server
        .post("/api/contracts/validate/envelope")
        .json(&envelope)
        .await;

    response.assert_status(StatusCode::OK);
    let body = response.json::<serde_json::Value>();
    assert_eq!(body["valid"], true);
    if let Some(errors) = body["errors"].as_array() {
        assert!(errors.is_empty());
    }
}

#[tokio::test]
async fn given_invalid_envelope_when_post_validate_then_returns_invalid_response() {
    let server = setup_test_server().await;

    let envelope = json!({
        "invalid_field": "test"
    });

    let response = server
        .post("/api/contracts/validate/envelope")
        .json(&envelope)
        .await;

    response.assert_status(StatusCode::OK);
    let body = response.json::<serde_json::Value>();
    assert_eq!(body["valid"], false);
    assert!(!body["errors"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn given_bulk_request_when_post_validate_bulk_then_returns_results_for_each() {
    let server = setup_test_server().await;

    let request = json!({
        "envelopes": [
            {
                "name": "valid",
                "envelope": {
                    "result": {
                        "success": true,
                        "data": "test"
                    }
                }
            },
            {
                "name": "invalid",
                "envelope": {
                    "missing": "result"
                }
            }
        ]
    });

    let response = server
        .post("/api/contracts/validate/envelope/bulk")
        .json(&request)
        .await;

    response.assert_status(StatusCode::OK);
    let body = response.json::<serde_json::Value>();
    let results = body["results"].as_array().unwrap();

    assert_eq!(results.len(), 2);
    assert_eq!(results[0]["name"], "valid");
    assert_eq!(results[0]["valid"], true);
    assert_eq!(results[1]["name"], "invalid");
    assert_eq!(results[1]["valid"], false);
}

#[tokio::test]
async fn given_malformed_json_when_post_validate_then_returns_bad_request() {
    let server = setup_test_server().await;

    let response = server
        .post("/api/contracts/validate/envelope")
        .content_type("application/json")
        .text("{invalid json}")
        .await;

    assert!(response.status_code().is_client_error());
}

#[tokio::test]
async fn given_envelope_with_all_fields_when_validate_then_returns_valid() {
    let server = setup_test_server().await;

    let envelope = json!({
        "result": {
            "success": true,
            "data": {"key": "value"}
        },
        "diagnostics": [
            {
                "level": "info",
                "message": "Processing complete"
            }
        ],
        "suggestions": [
            {
                "type": "optimization",
                "description": "Consider optimizing",
                "patch": [
                    {
                        "op": "add",
                        "path": "/optimization",
                        "value": true
                    }
                ]
            }
        ],
        "metrics": {
            "duration": {
                "total_ms": 100
            },
            "counters": {
                "items": 42
            }
        },
        "provenance": {
            "source": {
                "system": "test_system"
            }
        }
    });

    let response = server
        .post("/api/contracts/validate/envelope")
        .json(&envelope)
        .await;

    response.assert_status(StatusCode::OK);
    let body = response.json::<serde_json::Value>();
    assert_eq!(body["valid"], true);
}
