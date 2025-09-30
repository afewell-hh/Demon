# Graph REST API

Read-only REST API endpoints for querying graph commits and tags from the Demon runtime.

## Prerequisites

- NATS JetStream must be running with the `GRAPH_COMMITS` stream configured
- Runtime server must be started (`cargo run -p runtime`)
- Default port: `8080` (configurable via `PORT` environment variable)

## Base URL

```
http://localhost:8080/api/graph
```

## Endpoints

### Health Check

**GET** `/health`

Returns server health status.

**Response:**
```
OK
```

---

### Get Commit by ID

**GET** `/api/graph/commits/:commitId`

Retrieve a single commit by ID with full metadata and mutations.

**Path Parameters:**
- `commitId` (string, required): The commit ID to retrieve (64-character SHA256 hex)

**Query Parameters:**
- `tenantId` (string, required): Tenant identifier
- `projectId` (string, required): Project identifier
- `namespace` (string, required): Namespace identifier
- `graphId` (string, required): Graph identifier

**Response Headers:**
- `ETag`: Commit ID as quoted string for caching

**Success Response (200 OK):**
```json
{
  "event": "graph.commit.created:v1",
  "graphId": "graph-1",
  "tenantId": "tenant-1",
  "projectId": "proj-1",
  "namespace": "ns-1",
  "commitId": "abc123...",
  "parentCommitId": "def456...",
  "ts": "2025-09-30T12:34:56.789Z",
  "mutations": [
    {
      "op": "add-node",
      "nodeId": "node-1",
      "labels": ["Person"],
      "properties": {
        "name": "Alice"
      }
    }
  ],
  "mutationsCount": 1
}
```

**Error Responses:**
- `404 Not Found` - Commit does not exist
- `500 Internal Server Error` - JetStream query failure

**Example:**
```bash
curl "http://localhost:8080/api/graph/commits/abc123...?tenantId=t1&projectId=p1&namespace=ns1&graphId=g1"
```

---

### List Commits

**GET** `/api/graph/commits`

List recent commits for a graph scope, sorted by timestamp (most recent first).

**Query Parameters:**
- `tenantId` (string, required): Tenant identifier
- `projectId` (string, required): Project identifier
- `namespace` (string, required): Namespace identifier
- `graphId` (string, required): Graph identifier
- `limit` (integer, optional): Maximum number of commits to return (default: 50, max: 1000)

**Success Response (200 OK):**
```json
[
  {
    "event": "graph.commit.created:v1",
    "graphId": "graph-1",
    "tenantId": "tenant-1",
    "projectId": "proj-1",
    "namespace": "ns-1",
    "commitId": "abc123...",
    "parentCommitId": "def456...",
    "ts": "2025-09-30T12:35:00.000Z",
    "mutations": [...]
  },
  {
    "event": "graph.commit.created:v1",
    "commitId": "def456...",
    "ts": "2025-09-30T12:34:00.000Z",
    "mutations": [...]
  }
]
```

**Error Responses:**
- `500 Internal Server Error` - JetStream query failure

**Example:**
```bash
curl "http://localhost:8080/api/graph/commits?tenantId=t1&projectId=p1&namespace=ns1&graphId=g1&limit=100"
```

---

### Get Tag

**GET** `/api/graph/tags/:tag`

Retrieve a tag by name, returning the commit ID it points to.

**Path Parameters:**
- `tag` (string, required): The tag name to retrieve

**Query Parameters:**
- `tenantId` (string, required): Tenant identifier
- `projectId` (string, required): Project identifier
- `namespace` (string, required): Namespace identifier
- `graphId` (string, required): Graph identifier

**Response Headers:**
- `ETag`: `"tag:commitId"` for caching

**Success Response (200 OK):**
```json
{
  "tag": "v1.0.0",
  "commitId": "abc123...",
  "timestamp": "2025-09-30T12:34:56.789Z"
}
```

**Error Responses:**
- `404 Not Found` - Tag does not exist
- `500 Internal Server Error` - KV bucket query failure

**Example:**
```bash
curl "http://localhost:8080/api/graph/tags/v1.0.0?tenantId=t1&projectId=p1&namespace=ns1&graphId=g1"
```

---

### List Tags

**GET** `/api/graph/tags`

List all tags for a graph scope, sorted alphabetically by tag name.

**Query Parameters:**
- `tenantId` (string, required): Tenant identifier
- `projectId` (string, required): Project identifier
- `namespace` (string, required): Namespace identifier
- `graphId` (string, required): Graph identifier

**Success Response (200 OK):**
```json
[
  {
    "tag": "latest",
    "commitId": "abc123...",
    "timestamp": "2025-09-30T12:35:00.000Z"
  },
  {
    "tag": "v1.0.0",
    "commitId": "def456...",
    "timestamp": "2025-09-30T12:34:00.000Z"
  }
]
```

**Error Responses:**
- `500 Internal Server Error` - KV bucket query failure

**Example:**
```bash
curl "http://localhost:8080/api/graph/tags?tenantId=t1&projectId=p1&namespace=ns1&graphId=g1"
```

---

## CLI Usage

The `demonctl` CLI provides commands to interact with the Graph REST API.

### Get Commit

```bash
demonctl graph get-commit \
  --tenant-id t1 \
  --project-id p1 \
  --namespace ns1 \
  --graph-id g1 \
  --commit-id abc123... \
  --api-url http://localhost:8080
```

### Get Tag

```bash
demonctl graph get-tag \
  --tenant-id t1 \
  --project-id p1 \
  --namespace ns1 \
  --graph-id g1 \
  --tag v1.0.0 \
  --api-url http://localhost:8080
```

---

## Caching and ETags

Commit and tag endpoints return `ETag` headers for HTTP caching:

- **Commits**: ETag is the commit ID (immutable, safe for long caching)
- **Tags**: ETag is `"tag:commitId"` (mutable, cache with caution)

Clients can use `If-None-Match` headers to leverage 304 Not Modified responses (future enhancement).

---

## Error Response Format

All error responses follow this envelope structure:

```json
{
  "error": "Human-readable error message",
  "code": "ERROR_CODE"
}
```

**Common Error Codes:**
- `COMMIT_NOT_FOUND` - Requested commit does not exist
- `TAG_NOT_FOUND` - Requested tag does not exist
- `INTERNAL_ERROR` - Server-side failure (check logs)

---

## Implementation Notes

- Commit queries scan the `GRAPH_COMMITS` JetStream stream using filtered consumers
- Tag queries read from the `GRAPH_TAGS` KV bucket
- Pagination for commits list uses stream batch limits (not offset-based)
- Responses use JSON content-type; errors return appropriate HTTP status codes
- Server logs operations via tracing at `debug` level (queries) and `error` level (failures)

---

---

## Graph Query Operations

The graph capsule provides three core query operations for traversing and analyzing the graph structure at a given commit:

### Get Node

Retrieves a node by ID, including its labels, properties, and relationships.

**Mutation:**
```json
{
  "op": "get-node",
  "nodeId": "node-1"
}
```

**CLI Example:**
```bash
demonctl graph commit \
  --tenant-id t1 --project-id p1 --namespace ns1 --graph-id g1 \
  --parent-ref <COMMIT_ID> \
  get-node.json
```

### Find Neighbors

Retrieves all neighbors of a node (nodes connected by edges), optionally filtered by relationship type and direction.

**Mutation:**
```json
{
  "op": "neighbors",
  "nodeId": "node-1",
  "relType": "KNOWS",
  "direction": "outgoing"
}
```

**CLI Example:**
```bash
demonctl graph commit \
  --tenant-id t1 --project-id p1 --namespace ns1 --graph-id g1 \
  --parent-ref <COMMIT_ID> \
  neighbors.json
```

### Path Existence

Checks whether a path exists between two nodes, with optional constraints on relationship types and path length.

**Mutation:**
```json
{
  "op": "path-exists",
  "fromNodeId": "node-1",
  "toNodeId": "node-2",
  "relTypes": ["KNOWS", "WORKS_WITH"],
  "maxDepth": 3
}
```

**CLI Example:**
```bash
demonctl graph commit \
  --tenant-id t1 --project-id p1 --namespace ns1 --graph-id g1 \
  --parent-ref <COMMIT_ID> \
  path-exists.json
```

### Query Limitations and Performance

- **Commit Replay**: Query operations replay all commits from genesis to the target commit to reconstruct graph state. For large graphs (thousands of commits), expect replay latency proportional to history depth.
- **No Caching**: Current implementation does not cache graph state between queries. Each query performs a full replay.
- **Future Optimizations**: Planned enhancements include commit snapshots, incremental state caching, and indexed storage for sub-linear query performance.

---

## Future Enhancements

- Pagination tokens for large commit lists
- Conditional requests (If-None-Match) for cache validation
- GraphQL endpoint for flexible queries
- WebSocket support for real-time commit notifications
- Graph query result caching and snapshot-based replay optimization