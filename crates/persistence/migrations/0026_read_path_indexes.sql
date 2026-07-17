CREATE INDEX audit_records_issue_time_idx
    ON audit_records (project_id, target_id, created_at, id);

CREATE INDEX delivery_events_project_id_idx
    ON delivery_events (project_id, id);

CREATE INDEX comments_project_issue_time_idx
    ON comments (project_id, issue_id, created_at, id);

CREATE INDEX issue_edges_project_source_target_idx
    ON issue_edges (project_id, source_issue_id, target_issue_id);
