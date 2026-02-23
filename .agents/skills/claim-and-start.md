# claim-and-start

Atomically claim a pending task and transition it to Active.

## Usage
/claim-and-start <task_id>

## Steps
1. Call MCP tool `claim_task` with task_id
2. On success, call `transition_task` with new_state: "active"
3. Record task_id in session context
4. Confirm: "Task T-XXXXXX claimed and active."
