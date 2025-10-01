//! Graph REST API endpoints
//!
//! Provides read-only REST endpoints for querying graph commits and tags.

use crate::graph::query::{get_commit_by_id, get_tag, list_commits, list_tags};
use axum::{
    extract::{Path, Query},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use capsules_graph::GraphScope;
use serde::{Deserialize, Serialize};
use tracing::{debug, error};

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
