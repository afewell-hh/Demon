//! Graph REST API endpoints
//!
//! Provides read-only REST endpoints for querying graph commits and tags.

use crate::graph::query::{get_commit_by_id, get_tag, list_commits, list_tags};
use axum::{
    body::{Body, Bytes},
    extract::{Path, Query},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use capsules_graph::GraphScope;
use futures_util::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio_stream::wrappers::IntervalStream;
use tracing::{debug, error, info, warn};

/// Query parameters for listing commits
#[derive(Debug, Deserialize)]
pub struct ListCommitsQuery {
    #[serde(rename = "tenantId")]
    pub tenant_id: String,
    #[serde(rename = "projectId")]
    pub project_id: String,
    pub namespace: String,
    #[serde(rename = "graphId")]
    pub graph_id: String,
    pub limit: Option<usize>,
}

/// Query parameters for getting a single commit
#[derive(Debug, Deserialize)]
pub struct GetCommitQuery {
    #[serde(rename = "tenantId")]
    pub tenant_id: String,
    #[serde(rename = "projectId")]
    pub project_id: String,
    pub namespace: String,
    #[serde(rename = "graphId")]
    pub graph_id: String,
}

/// Query parameters for tag operations
#[derive(Debug, Deserialize)]
pub struct TagQuery {
    #[serde(rename = "tenantId")]
    pub tenant_id: String,
    #[serde(rename = "projectId")]
    pub project_id: String,
    pub namespace: String,
    #[serde(rename = "graphId")]
    pub graph_id: String,
}

/// Error response format
#[derive(Serialize)]
struct ErrorResponse {
    error: String,
    code: String,
}

/// Create graph API router
pub fn routes() -> Router {
    Router::new()
        .route("/commits/:commitId", get(get_commit))
        .route("/commits", get(list_commits_handler))
        .route("/commits/stream", get(stream_commits_sse))
        .route("/tags/:tag", get(get_tag_handler))
        .route("/tags", get(list_tags_handler))
}

/// GET /api/graph/commits/:commitId
///
/// Retrieve a single commit by ID. Requires query params for scope.
///
/// Query params:
/// - tenantId (required)
/// - projectId (required)
/// - namespace (required)
/// - graphId (required)
///
/// Example: GET /api/graph/commits/abc123?tenantId=t1&projectId=p1&namespace=ns1&graphId=g1
async fn get_commit(
    Path(commit_id): Path<String>,
    Query(query): Query<GetCommitQuery>,
) -> Response {
    debug!(
        "GET /api/graph/commits/{} with scope {:?}",
        commit_id, query
    );

    let scope = GraphScope {
        tenant_id: query.tenant_id,
        project_id: query.project_id,
        namespace: query.namespace,
        graph_id: query.graph_id,
    };

    match get_commit_by_id(&scope, &commit_id).await {
        Ok(Some(commit)) => {
            // Generate ETag from commit ID
            let mut headers = HeaderMap::new();
            if let Ok(etag) = format!("\"{}\"", commit.commit_id).parse() {
                headers.insert(header::ETAG, etag);
            }

            let mut response_data = serde_json::to_value(&commit).unwrap_or_default();
            if let Some(obj) = response_data.as_object_mut() {
                // Add commit result metadata
                if let Ok(commit_result) = commit.to_commit_result() {
                    obj.insert(
                        "mutationsCount".to_string(),
                        serde_json::json!(commit_result.mutations_count),
                    );
                }
            }

            (StatusCode::OK, headers, Json(response_data)).into_response()
        }
        Ok(None) => {
            error!("Commit not found: {}", commit_id);
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("Commit '{}' not found", commit_id),
                    code: "COMMIT_NOT_FOUND".to_string(),
                }),
            )
                .into_response()
        }
        Err(e) => {
            error!("Failed to retrieve commit {}: {}", commit_id, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to retrieve commit: {}", e),
                    code: "INTERNAL_ERROR".to_string(),
                }),
            )
                .into_response()
        }
    }
}

/// GET /api/graph/commits
///
/// List recent commits for a graph scope.
///
/// Query params:
/// - tenantId (required)
/// - projectId (required)
/// - namespace (required)
/// - graphId (required)
/// - limit (optional, default: 50, max: 1000)
///
/// Example: GET /api/graph/commits?tenantId=t1&projectId=p1&namespace=ns1&graphId=g1&limit=100
async fn list_commits_handler(Query(query): Query<ListCommitsQuery>) -> Response {
    debug!("GET /api/graph/commits with query {:?}", query);

    let scope = GraphScope {
        tenant_id: query.tenant_id,
        project_id: query.project_id,
        namespace: query.namespace,
        graph_id: query.graph_id,
    };

    match list_commits(&scope, query.limit).await {
        Ok(commits) => {
            debug!("Retrieved {} commits", commits.len());
            (StatusCode::OK, Json(commits)).into_response()
        }
        Err(e) => {
            error!("Failed to list commits: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to list commits: {}", e),
                    code: "INTERNAL_ERROR".to_string(),
                }),
            )
                .into_response()
        }
    }
}

/// GET /api/graph/tags/:tag
///
/// Retrieve a tag by name, returning the commit ID it points to.
///
/// Query params:
/// - tenantId (required)
/// - projectId (required)
/// - namespace (required)
/// - graphId (required)
///
/// Returns: { "tag": "v1.0.0", "commitId": "abc123", "timestamp": "2025-01-01T00:00:00Z" }
///
/// Example: GET /api/graph/tags/v1.0.0?tenantId=t1&projectId=p1&namespace=ns1&graphId=g1
async fn get_tag_handler(Path(tag): Path<String>, Query(query): Query<TagQuery>) -> Response {
    debug!("GET /api/graph/tags/{} with scope {:?}", tag, query);

    let scope = GraphScope {
        tenant_id: query.tenant_id,
        project_id: query.project_id,
        namespace: query.namespace,
        graph_id: query.graph_id,
    };

    match get_tag(&scope, &tag).await {
        Ok(Some(tagged_commit)) => {
            // Generate ETag from tag + commit ID hash
            let mut headers = HeaderMap::new();
            if let Ok(etag) =
                format!("\"{}:{}\"", tagged_commit.tag, tagged_commit.commit_id).parse()
            {
                headers.insert(header::ETAG, etag);
            }

            (StatusCode::OK, headers, Json(tagged_commit)).into_response()
        }
        Ok(None) => {
            error!("Tag not found: {}", tag);
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("Tag '{}' not found", tag),
                    code: "TAG_NOT_FOUND".to_string(),
                }),
            )
                .into_response()
        }
        Err(e) => {
            error!("Failed to retrieve tag {}: {}", tag, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to retrieve tag: {}", e),
                    code: "INTERNAL_ERROR".to_string(),
                }),
            )
                .into_response()
        }
    }
}

/// GET /api/graph/tags
///
/// List all tags for a graph scope.
///
/// Query params:
/// - tenantId (required)
/// - projectId (required)
/// - namespace (required)
/// - graphId (required)
///
/// Returns: [{ "tag": "v1.0.0", "commitId": "abc123", "timestamp": "..." }, ...]
///
/// Example: GET /api/graph/tags?tenantId=t1&projectId=p1&namespace=ns1&graphId=g1
async fn list_tags_handler(Query(query): Query<TagQuery>) -> Response {
    debug!("GET /api/graph/tags with query {:?}", query);

    let scope = GraphScope {
        tenant_id: query.tenant_id,
        project_id: query.project_id,
        namespace: query.namespace,
        graph_id: query.graph_id,
    };

    match list_tags(&scope).await {
        Ok(tags) => {
            debug!("Retrieved {} tags", tags.len());
            (StatusCode::OK, Json(tags)).into_response()
        }
        Err(e) => {
            error!("Failed to list tags: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to list tags: {}", e),
                    code: "INTERNAL_ERROR".to_string(),
                }),
            )
                .into_response()
        }
    }
}

/// GET /api/graph/commits/stream
///
/// Server-Sent Events endpoint for streaming graph commit updates.
///
/// Query params:
/// - tenantId (required)
/// - projectId (required)
/// - namespace (required)
/// - graphId (optional, defaults to wildcard)
///
/// SSE Event types:
/// - init: Connection established, snapshot loaded
/// - commit: New graph commit event
/// - heartbeat: Keep-alive signal (every ~25s)
///
/// Example: GET /api/graph/commits/stream?tenantId=t1&projectId=p1&namespace=ns1&graphId=g1
///
/// Reconnection policy (client-side):
/// - Exponential backoff: 1s, 2s, 4s, 8s, 16s, max 30s
/// - Max retries: Continue indefinitely with backoff cap
async fn stream_commits_sse(Query(query): Query<ListCommitsQuery>) -> Response {
    debug!(
        "Starting SSE stream for graph commits with query: {:?}",
        query
    );

    let scope = GraphScope {
        tenant_id: query.tenant_id.clone(),
        project_id: query.project_id.clone(),
        namespace: query.namespace.clone(),
        graph_id: query.graph_id.clone(),
    };

    // Heartbeat interval from env or default to 25s
    let heartbeat_secs: u64 = std::env::var("SSE_HEARTBEAT_SECONDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(25);

    // Fetch initial commits BEFORE creating the stream to avoid async issues within stream
    let initial_commits = match list_commits(&scope, Some(50)).await {
        Ok(commits) => {
            debug!("SSE: Loaded {} commits for init event", commits.len());
            commits
        }
        Err(e) => {
            warn!("Failed to load initial commits: {}", e);
            vec![]
        }
    };

    let stream = create_commit_stream(scope.clone(), initial_commits, heartbeat_secs);

    // Convert String stream to Bytes stream for Body
    let byte_stream = stream.map(|result| result.map(Bytes::from));

    // Set SSE headers
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, "text/event-stream".parse().unwrap());
    headers.insert(header::CACHE_CONTROL, "no-cache".parse().unwrap());
    headers.insert(header::CONNECTION, "keep-alive".parse().unwrap());

    (headers, Body::from_stream(byte_stream)).into_response()
}

/// Create the SSE stream combining commits and heartbeats
fn create_commit_stream(
    scope: GraphScope,
    initial_commits: Vec<crate::graph::query::CommitEvent>,
    heartbeat_secs: u64,
) -> impl Stream<Item = Result<String, std::io::Error>> {
    async_stream::stream! {
        debug!("SSE: Stream started for scope {:?}", scope);

        // Yield init event immediately with pre-fetched commits
        let init_payload = serde_json::json!({
            "type": "init",
            "scope": {
                "tenantId": scope.tenant_id,
                "projectId": scope.project_id,
                "namespace": scope.namespace,
                "graphId": scope.graph_id,
            },
            "commits": initial_commits,
            "timestamp": chrono::Utc::now().to_rfc3339()
        });
        yield Ok::<_, std::io::Error>(format!("event: init\ndata: {}\n\n", init_payload));

        // TODO: Add live commit streaming from NATS
        // For now, just send heartbeats to keep connection alive
        // Creating NATS consumers within async_stream seems to hang, needs investigation

        // Set up heartbeat timer
        let interval = tokio::time::interval(Duration::from_secs(heartbeat_secs.max(1)));
        let mut heartbeat_stream = IntervalStream::new(interval);
        let mut seq = 0u64;

        debug!("SSE: Starting heartbeat loop (every {}s)", heartbeat_secs);

        // Send heartbeats to keep connection alive
        loop {
            tokio::select! {
                // Heartbeats for keep-alive
                Some(_) = heartbeat_stream.next() => {
                    let heartbeat_payload = serde_json::json!({
                        "type": "heartbeat",
                        "seq": seq,
                        "timestamp": chrono::Utc::now().to_rfc3339()
                    });
                    seq += 1;
                    yield Ok::<_, std::io::Error>(format!("event: heartbeat\ndata: {}\n\n", heartbeat_payload));
                }

                // Client disconnected
                else => {
                    info!("SSE client disconnected");
                    break;
                }
            }
        }
    }
}
