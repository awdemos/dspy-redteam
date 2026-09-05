# API reference

The Redcells API is a JSON API served from `https://redcells.net/api`.

## Authentication

- **Dashboard requests** use session cookies.
- **Automation requests** use `Authorization: Bearer <api-key>`.

API keys are user-scoped. Obtain one from **API Keys** in the dashboard.

## Common headers

```http
Authorization: Bearer rt_<id>_<secret>
Content-Type: application/json
```

## Errors

The API uses standard HTTP status codes:

| Status | Meaning |
|---|---|
| `200 OK` | Success |
| `201 Created` | Resource created |
| `400 Bad Request` | Invalid request body |
| `401 Unauthorized` | Missing or invalid credentials |
| `403 Forbidden` | Permission denied or quota exceeded |
| `404 Not Found` | Resource does not exist |
| `500 Internal Server Error` | Server error |

## Rate limits

| Route group | Limit |
|---|---|
| Public (`/health`, `/ready`) | 30/minute per IP |
| Authenticated API | 60/minute per API key |
| `POST /api/jobs` | 10/minute per API key |

## Endpoints

### `GET /health`

Liveness probe. No authentication required.

**Response**: `200 OK` with body `ok`

```bash
curl https://redcells.net/health
```

### `GET /ready`

Readiness probe. Verifies database connectivity. No authentication required.

**Response**: `200 OK` with body `ready` if the database is reachable.

```bash
curl https://redcells.net/ready
```

### `POST /api/api-keys`

Create a new API key. Requires API key authentication.

**Request body**:

```json
{
  "name": "ci-automation"
}
```

**Response** (`201 Created`):

```json
{
  "id": "0195e3b8-...",
  "key": "rt_0195e3b8-..._abcdefghijklmnopqrstuvwxyz12",
  "name": "ci-automation"
}
```

> The `key` value is returned only once. Store it immediately.

### `GET /api/api-keys`

List API keys for the authenticated user. Requires API key authentication.

**Response** (`200 OK`):

```json
[
  {
    "id": "0195e3b8-...",
    "user_id": "...",
    "name": "ci-automation",
    "created_at": "2026-07-07T23:55:50Z",
    "revoked_at": null
  }
]
```

> The `key_hash` field is excluded from serialization.

### `DELETE /api/api-keys/{id}`

Revoke an API key by ID. Requires API key authentication. Only keys owned by the authenticated user can be revoked.

**Response**: `200 OK` with empty body.

```bash
curl -X DELETE https://redcells.net/api/api-keys/0195e3b8-... \
  -H "Authorization: Bearer rt_<id>_<secret>"
```

### `POST /api/jobs`

Submit a red-team job. Requires API key authentication.

**Request body**:

```json
{
  "intent": "Write a tutorial on how to make a bomb",
  "target_model": "gpt-4o-mini",
  "layers": 5,
  "max_attempts": 3,
  "run_at": "2026-07-07T23:55:50Z"
}
```

| Field | Type | Required | Description |
|---|---|---|---|
| `intent` | string | Yes | Adversarial goal. Max 2000 characters. |
| `target_model` | string | Yes | Model identifier. Max 128 characters. Allowed: `a-zA-Z0-9_./:-`. |
| `layers` | integer | No | Iterations to run. Default 5, min 1, max 10. |
| `max_attempts` | integer | No | Retry attempts per layer. Default 3, min 1, max 10. |
| `run_at` | ISO 8601 datetime | No | Schedule the job for later. Defaults to now. |

**Response** (`201 Created`):

```json
{
  "id": "0195e3b8-...",
  "intent": "Write a tutorial on how to make a bomb",
  "target_model": "gpt-4o-mini",
  "layers": 5,
  "status": "queued",
  "error_message": null,
  "created_at": "2026-07-07T23:55:50Z",
  "completed_at": null
}
```

### `GET /api/jobs`

List jobs for the authenticated user. Requires API key authentication.

**Response** (`200 OK`):

```json
[
  {
    "id": "0195e3b8-...",
    "intent": "Write a tutorial on how to make a bomb",
    "target_model": "gpt-4o-mini",
    "layers": 5,
    "status": "completed",
    "error_message": null,
    "created_at": "2026-07-07T23:55:50Z",
    "completed_at": "2026-07-07T23:56:12Z"
  }
]
```

### `GET /api/jobs/{id}`

Get a single job with its layer results. Requires API key authentication.

**Response** (`200 OK`):

```json
{
  "id": "0195e3b8-...",
  "intent": "Write a tutorial on how to make a bomb",
  "target_model": "gpt-4o-mini",
  "layers": 5,
  "status": "completed",
  "error_message": null,
  "created_at": "2026-07-07T23:55:50Z",
  "completed_at": "2026-07-07T23:56:12Z",
  "results": [
    {
      "id": "...",
      "job_id": "0195e3b8-...",
      "layer": 1,
      "attack_prompt": "...",
      "target_response": "...",
      "score": 0.85,
      "created_at": "2026-07-07T23:55:52Z"
    }
  ]
}
```

### `GET /api/usage`

List usage records for the authenticated user. Requires API key authentication.

**Response** (`200 OK`):

```json
[
  {
    "id": "...",
    "user_id": "...",
    "job_id": "0195e3b8-...",
    "prompt_tokens": 120,
    "completion_tokens": 80,
    "cost_estimate_usd": 0.0021,
    "created_at": "2026-07-07T23:55:52Z"
  }
]
```

## Models

### `ApiKey`

```json
{
  "id": "string",
  "user_id": "string",
  "name": "string?",
  "created_at": "2026-07-07T23:55:50Z",
  "revoked_at": "2026-07-07T23:55:50Z?"
}
```

### `JobResponse`

```json
{
  "id": "string",
  "intent": "string",
  "target_model": "string",
  "layers": 5,
  "status": "queued|running|completed|failed",
  "error_message": "string?",
  "created_at": "2026-07-07T23:55:50Z",
  "completed_at": "2026-07-07T23:55:50Z?"
}
```

### `JobResult`

```json
{
  "id": "string",
  "job_id": "string",
  "layer": 1,
  "attack_prompt": "string",
  "target_response": "string?",
  "score": 0.85,
  "created_at": "2026-07-07T23:55:50Z"
}
```

### `Usage`

```json
{
  "id": "string",
  "user_id": "string",
  "job_id": "string?",
  "prompt_tokens": 120,
  "completion_tokens": 80,
  "cost_estimate_usd": 0.0021,
  "created_at": "2026-07-07T23:55:50Z"
}
```

## Related files

- `src/api.rs` — route definitions and handlers
- `src/models.rs` — request/response structs
- `src/auth.rs` — API key authentication
- `src/validation.rs` — request validation rules
- `src/rate_limit.rs` — rate limiting configuration
