-- Read-only pilot baseline. Run with psql against the active Riichi database.
SELECT 'open_leases' AS metric, count(*)::text AS value
FROM leases WHERE state = 'active'
UNION ALL
SELECT 'active_holds', count(*)::text
FROM dispatch_holds WHERE released_at IS NULL
  AND (expires_at IS NULL OR expires_at > now())
UNION ALL
SELECT 'pending_outbox', count(*)::text
FROM outbox_messages WHERE delivered_at IS NULL
UNION ALL
SELECT 'dead_letter_outbox', count(*)::text
FROM outbox_messages WHERE dead_lettered_at IS NOT NULL
UNION ALL
SELECT 'pending_document_jobs', count(*)::text
FROM document_jobs
WHERE completed_at IS NULL AND dead_lettered_at IS NULL
UNION ALL
SELECT 'dead_letter_document_jobs', count(*)::text
FROM document_jobs
WHERE dead_lettered_at IS NOT NULL
UNION ALL
SELECT 'retried_document_jobs', count(*)::text
FROM document_jobs
WHERE attempt_count > 0
UNION ALL
SELECT 'unread_notifications', count(*)::text
FROM notifications WHERE read_at IS NULL
UNION ALL
SELECT 'quarantined_attempts_24h', count(*)::text
FROM quarantined_attempts WHERE created_at >= now() - interval '24 hours'
UNION ALL
SELECT 'claim_operations_24h', count(*)::text
FROM audit_records WHERE operation = 'claim' AND created_at >= now() - interval '24 hours'
UNION ALL
SELECT 'stale_report_rejections_24h', count(*)::text
FROM quarantined_attempts WHERE reason IN ('stale_lease', 'lease_not_active')
  AND created_at >= now() - interval '24 hours'
UNION ALL
SELECT 'takeovers_24h', count(*)::text
FROM audit_records WHERE operation = 'takeover_issue' AND created_at >= now() - interval '24 hours'
UNION ALL
SELECT 'approval_supersessions_24h', count(*)::text
FROM approval_requests WHERE state = 'superseded' AND decided_at >= now() - interval '24 hours'
UNION ALL
SELECT 'pending_document_provisioning', count(*)::text
FROM documents WHERE provisioning_state = 'pending' AND deleted_at IS NULL
UNION ALL
SELECT 'failed_document_provisioning', count(*)::text
FROM documents WHERE provisioning_state = 'failed' AND deleted_at IS NULL
UNION ALL
SELECT 'pending_attachment_uploads', count(*)::text
FROM attachment_uploads
WHERE completed_at IS NULL
  AND expires_at > now()
UNION ALL
SELECT 'expired_attachment_uploads', count(*)::text
FROM attachment_uploads
WHERE completed_at IS NULL
  AND expires_at <= now()
UNION ALL
SELECT 'stale_attachment_cleanup_claims', count(*)::text
FROM attachment_uploads
WHERE completed_at IS NULL
  AND expires_at <= now()
  AND cleanup_claimed_at < now() - interval '5 minutes'
UNION ALL
SELECT 'document_activity_24h', count(*)::text
FROM document_activity WHERE created_at >= now() - interval '24 hours'
UNION ALL
SELECT 'accepted_loro_updates_24h', count(*)::text
FROM document_loro_updates WHERE accepted_at >= now() - interval '24 hours'
UNION ALL
SELECT 'issue_metadata_sync_rows', count(*)::text
FROM issue_metadata_sync
UNION ALL
SELECT 'issue_activity_sync_rows', count(*)::text
FROM issue_activity_sync
UNION ALL
SELECT 'human_issue_sync_rows', count(*)::text
FROM human_issue_sync
UNION ALL
SELECT 'human_document_sync_rows', count(*)::text
FROM human_document_sync
UNION ALL
SELECT 'human_agent_sync_rows', count(*)::text
FROM human_agent_sync
UNION ALL
SELECT 'document_projection_lag', count(*)::text
FROM documents d
JOIN document_loro_snapshots s ON s.document_id = d.id
LEFT JOIN document_projections p ON p.document_id = d.id
WHERE d.deleted_at IS NULL
  AND (p.document_id IS NULL OR p.projected_at < s.updated_at)
UNION ALL
SELECT 'orphan_document_bindings', count(*)::text
FROM document_bindings b
LEFT JOIN documents d ON d.id = b.document_id
WHERE d.id IS NULL OR d.deleted_at IS NOT NULL
UNION ALL
SELECT 'orphan_document_projections', count(*)::text
FROM document_projections p
LEFT JOIN documents d ON d.id = p.document_id
WHERE d.id IS NULL OR d.deleted_at IS NOT NULL
ORDER BY metric;
