# create-task

Create a new task in the ClawDE task queue.

## Usage
/create-task <title> [priority: low|medium|high]

## Steps
1. Call MCP tool `create_task` with title, priority, and repo path
2. Record returned task_id
3. Confirm to user: "Task T-XXXXXX created."
