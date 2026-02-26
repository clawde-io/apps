# REST API

The `clawd` daemon exposes a typed REST API on port **4301** (localhost only).

## Endpoints

| Method | Path | Auth | Description |
| --- | --- | --- | --- |
| GET | `/api/v1/health` | No | Daemon status and uptime |
| GET | `/api/v1/openapi.json` | No | OpenAPI 3.1 specification |
| GET | `/api/v1/sessions` | Yes | List all sessions |
| POST | `/api/v1/sessions` | Yes | Create a new session |
| GET | `/api/v1/sessions/:id` | Yes | Get session details |
| POST | `/api/v1/sessions/:id/tasks` | Yes | Submit a task to a session |
| GET | `/api/v1/sessions/:id/events` | Yes | SSE push events stream |
| GET | `/api/v1/metrics` | Yes | 24h cost and token summary |
| GET | `/api/v1/memory` | Yes | List AI memory entries |

## Authentication

The REST API uses Bearer token authentication. Configure via `api_token` in `config.toml`
or the `CLAWD_API_TOKEN` environment variable. If no token is configured, all requests
are allowed (loopback-only, not recommended for shared machines).

```bash
curl -H "Authorization: Bearer $CLAWD_API_TOKEN" \
  http://127.0.0.1:4301/api/v1/sessions
```

For SSE streams (EventSource doesn't support headers), pass the token as a query parameter:

```
GET /api/v1/sessions/{id}/events?token=your-token
```

## TypeScript SDK

Install `@clawde/rest` from the ClawDE npm registry:

```bash
pnpm add @clawde/rest
```

### REST Client

```typescript
import { ClawdRestClient } from "@clawde/rest";

const client = new ClawdRestClient({ token: "your-api-token" });

const health = await client.health();
const { sessions } = await client.sessions.list();

const { session_id } = await client.sessions.create({
  repo_path: "/path/to/project",
  provider: "claude",
});

await client.sessions.submitTask(session_id, {
  task: "Implement the user authentication module",
});

const summary = await client.metrics.summary();
const { entries } = await client.memory.list("global");
```

### SSE Streaming

```typescript
import { subscribeSessionEvents } from "@clawde/rest";

const sub = subscribeSessionEvents(session_id, {
  token: "your-api-token",
  onEvent: (event) => {
    switch (event.method) {
      case "session.message":
        console.log("Message:", event.params);
        break;
      case "session.complete":
        console.log("Done!");
        sub.close();
        break;
      case "budget_warning":
        console.warn("Budget warning:", event.params);
        break;
    }
  },
});
```

## Postman Collection

Import `.docs/rest-api/clawd-postman-collection.json` into Postman. Set the `token`
collection variable to your `CLAWD_API_TOKEN` value.

## OpenAPI Spec

The daemon serves a full OpenAPI 3.1 spec at `GET /api/v1/openapi.json`. Import into
Swagger UI, Insomnia, or any OpenAPI-compatible tool.
