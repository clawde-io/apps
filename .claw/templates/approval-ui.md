# Approval Request UI Template

This template defines the approval card shown to the developer when an agent calls
`request_approval`. The daemon fills the placeholders from the `tool.approvalRequested`
push event payload.

---

## Approval Card — Display Format

```
Task: {task_title}
Agent: {agent_role} ({model})
Action: {tool_name}
Risk: {risk_level}

Details:
{tool_arguments_summary}

Impact:
{impact_description}

[Approve]  [Deny]  [Request Clarification]
```

---

## Field definitions

### task_title
The title of the task the agent is working on. Pulled from the task record by task_id.
Example: `"Add user logout endpoint"`

### agent_role
The agent type making the request: `Implementer`, `Reviewer`, `QA`.
Include the model name in parentheses to show which provider is making the request.
Example: `"Implementer (claude-sonnet-4-5)"`

### tool_name
The MCP tool the agent wants to call.
Example: `"apply_patch"`, `"run_tests"`

### risk_level
One of: `low`, `medium`, `high`, `critical`.
Display `high` in orange, `critical` in red.

### tool_arguments_summary
Human-readable summary of the arguments. Do not dump raw JSON.
For `apply_patch`: list the files being changed and the number of lines modified.
For `run_tests`: show the command and the working directory.
For other tools: show key fields only.

Example for apply_patch:
```
Files to be modified:
  daemon/src/session.rs (+12, -3)
  daemon/src/ipc/handlers/session.rs (+5, -0)
Patch idempotency key: 7f3a-9b2c-...
```

### impact_description
One to three sentences describing what happens if this action proceeds.
Written in plain English for a developer who may not have context.
Example: `"Modifies the session creation handler. If the patch has a bug, new sessions will fail to create."`

---

## Mobile push notification (compact format)

When showing on mobile (limited space):

```
[ClawDE] Approval needed
{agent_role}: {tool_name} on "{task_title}"
Risk: {risk_level}
Tap to review.
```

---

## Desktop approval card (expanded format)

The desktop shows the full card above with syntax-highlighted diff preview
(for `apply_patch`) or the test command (for `run_tests`).

Buttons:
- **Approve** — sends `tool.approve` with the `approval_id`.
- **Deny** — sends `tool.reject` with an optional reason (prompt the developer).
- **Request Clarification** — opens a text field to send a message back to the agent.
  The agent receives this as a `log_event` of type `"approval_clarification"`.

---

## Timeout behaviour

If the developer does not respond within 5 minutes:
- The request is automatically denied.
- The agent receives a rejection event with reason `"approval_timeout"`.
- The task transitions to `blocked` with `block_reason: "Approval timed out after 5 minutes"`.
