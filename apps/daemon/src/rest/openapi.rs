// rest/openapi.rs — OpenAPI 3.1 spec generator (Sprint QQ RA.7).
//
// Returns the ClawDE REST API spec as JSON at GET /api/v1/openapi.json.
// Used by the @clawde/rest SDK codegen and API docs.

use axum::{extract::State, Json};
use serde_json::{json, Value};
use std::sync::Arc;

use super::REST_PORT;
use crate::AppContext;

pub async fn openapi_spec(State(_ctx): State<Arc<AppContext>>) -> Json<Value> {
    Json(json!({
        "openapi": "3.1.0",
        "info": {
            "title": "ClawDE REST API",
            "version": "1.0.0",
            "description": "Public REST API for the ClawDE daemon. Bridges JSON-RPC sessions to HTTP/SSE.",
            "contact": { "email": "dev@clawde.io" },
            "license": { "name": "MIT" }
        },
        "servers": [
            { "url": format!("http://localhost:{REST_PORT}/api/v1"), "description": "Local daemon" }
        ],
        "security": [{ "BearerAuth": [] }],
        "components": {
            "securitySchemes": {
                "BearerAuth": {
                    "type": "http",
                    "scheme": "bearer",
                    "description": "API token from `~/.claw/config.toml` [api] section."
                }
            },
            "schemas": {
                "Session": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "string" },
                        "status": { "type": "string", "enum": ["idle", "running", "paused", "completed", "error"] },
                        "provider": { "type": "string" },
                        "repo_path": { "type": "string" },
                        "created_at": { "type": "integer", "description": "Unix timestamp" }
                    }
                },
                "MetricsSummary": {
                    "type": "object",
                    "properties": {
                        "total_tokens_in": { "type": "integer" },
                        "total_tokens_out": { "type": "integer" },
                        "total_tool_calls": { "type": "integer" },
                        "total_cost_usd": { "type": "number" },
                        "session_count": { "type": "integer" }
                    }
                },
                "MemoryEntry": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "string" },
                        "scope": { "type": "string" },
                        "key": { "type": "string" },
                        "value": { "type": "string" },
                        "weight": { "type": "integer", "minimum": 1, "maximum": 10 },
                        "source": { "type": "string" }
                    }
                }
            }
        },
        "paths": {
            "/health": {
                "get": {
                    "operationId": "getHealth",
                    "summary": "Daemon health check",
                    "security": [],
                    "responses": { "200": { "description": "Daemon is healthy" } }
                }
            },
            "/sessions": {
                "get": {
                    "operationId": "listSessions",
                    "summary": "List active sessions",
                    "responses": { "200": { "description": "Session list" } }
                },
                "post": {
                    "operationId": "createSession",
                    "summary": "Create a new session",
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {
                                    "type": "object",
                                    "properties": {
                                        "provider": { "type": "string" },
                                        "repo_path": { "type": "string" }
                                    }
                                }
                            }
                        }
                    },
                    "responses": { "200": { "description": "Created session" } }
                }
            },
            "/sessions/{id}": {
                "get": {
                    "operationId": "getSession",
                    "summary": "Get session status",
                    "parameters": [{ "name": "id", "in": "path", "required": true, "schema": { "type": "string" } }],
                    "responses": { "200": { "description": "Session details" } }
                }
            },
            "/sessions/{id}/tasks": {
                "post": {
                    "operationId": "submitTask",
                    "summary": "Submit a task to a session",
                    "parameters": [{ "name": "id", "in": "path", "required": true, "schema": { "type": "string" } }],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {
                                    "type": "object",
                                    "required": ["content"],
                                    "properties": { "content": { "type": "string" } }
                                }
                            }
                        }
                    },
                    "responses": { "200": { "description": "Task submitted" } }
                }
            },
            "/sessions/{id}/events": {
                "get": {
                    "operationId": "sessionEvents",
                    "summary": "SSE stream of session events",
                    "parameters": [{ "name": "id", "in": "path", "required": true, "schema": { "type": "string" } }],
                    "responses": {
                        "200": {
                            "description": "SSE event stream",
                            "content": { "text/event-stream": { "schema": { "type": "string" } } }
                        }
                    }
                }
            },
            "/metrics": {
                "get": {
                    "operationId": "getMetrics",
                    "summary": "24h metrics summary",
                    "responses": { "200": { "description": "Metrics summary" } }
                }
            },
            "/memory": {
                "get": {
                    "operationId": "listMemory",
                    "summary": "List memory entries",
                    "responses": { "200": { "description": "Memory entries" } }
                }
            }
        }
    }))
}
