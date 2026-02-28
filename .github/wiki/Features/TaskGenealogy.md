# Task Genealogy

Task genealogy tracks parent/child relationships between tasks. When an AI session spawns a sub-task, the relationship is recorded so you can trace the full ancestry tree.

## Spawn a child task

Use the `task.spawn` RPC to create a child task linked to its parent:

```json
{
  "method": "task.spawn",
  "params": {
    "parentId": "parent-task-id",
    "title": "Fix auth edge case",
    "description": "Handle expired tokens in the login flow",
    "relationship": "spawned_from"
  }
}
```

**Relationship types:**

| Type | Meaning |
| --- | --- |
| `spawned_from` | Child was created as a sub-task of parent |
| `blocked_by` | This task cannot proceed until parent is done |
| `related_to` | Informational link — no ordering constraint |

## Query lineage

Use `task.lineage` to get the full ancestor and descendant tree:

```json
{
  "method": "task.lineage",
  "params": { "taskId": "child-task-id" }
}
```

Response:

```json
{
  "taskId": "child-task-id",
  "ancestors": [
    { "taskId": "parent-id", "title": "Original task", "relationship": "spawned_from" }
  ],
  "descendants": [
    { "taskId": "grandchild-id", "title": "Sub-sub-task", "relationship": "spawned_from" }
  ]
}
```

## Flutter UI

The task detail page shows a collapsible genealogy tree:

- Ancestors shown above the task (grey, smaller)
- Descendants shown below (indented, with relationship label)
- Tapping any node navigates to that task's detail page

## Database

Genealogy is stored in the `task_genealogy` table:

```sql
parent_task_id TEXT NOT NULL REFERENCES agent_tasks(id)
child_task_id  TEXT NOT NULL REFERENCES agent_tasks(id)
relationship   TEXT NOT NULL DEFAULT 'spawned_from'
```

Cascades on delete — removing a parent removes all its genealogy links.
