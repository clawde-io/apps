# request-review

Transition a task from Active to CodeReview and notify reviewer.

## Usage
/request-review <task_id>

## Steps
1. Verify task is in Active state
2. Call MCP tool `transition_task` with task_id, new_state: "code_review"
3. Confirm: "Task T-XXXXXX moved to CodeReview."
