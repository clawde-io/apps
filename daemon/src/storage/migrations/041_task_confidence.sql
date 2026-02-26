-- Sprint CC TC.1 — Task Confidence Scores
-- AI assigns a 0-1 confidence score at task completion.

ALTER TABLE agent_tasks ADD COLUMN confidence_score REAL;
ALTER TABLE agent_tasks ADD COLUMN confidence_reasoning TEXT;
