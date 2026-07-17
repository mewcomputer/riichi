-- Read-only verification for a restored or migrated pilot database.
\set ON_ERROR_STOP on

DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM issues i LEFT JOIN issue_dispatch d ON d.issue_id = i.id WHERE d.issue_id IS NULL) THEN
        RAISE EXCEPTION 'projection verification failed: issue without dispatch row';
    END IF;
    IF EXISTS (SELECT 1 FROM issue_dispatch d LEFT JOIN issues i ON i.id = d.issue_id WHERE i.id IS NULL) THEN
        RAISE EXCEPTION 'projection verification failed: orphan dispatch row';
    END IF;
    IF EXISTS (
        SELECT 1
        FROM outbox_messages o
        WHERE o.delivered_at IS NOT NULL
          AND NOT EXISTS (SELECT 1 FROM delivery_events d WHERE d.id = o.id)
    ) THEN
        RAISE EXCEPTION 'projection verification failed: delivered outbox message without delivery event';
    END IF;
    IF EXISTS (
        SELECT 1
        FROM leases l
        JOIN issue_dispatch d ON d.issue_id = l.issue_id
        WHERE l.state = 'active' AND d.active_lease_id IS DISTINCT FROM l.id
    ) THEN
        RAISE EXCEPTION 'projection verification failed: active lease mismatch';
    END IF;
    IF EXISTS (
        SELECT 1
        FROM issues i
        LEFT JOIN issue_metadata_sync s ON s.issue_id = i.id
        WHERE s.issue_id IS NULL
    ) THEN
        RAISE EXCEPTION 'projection verification failed: issue without metadata sync row';
    END IF;
    IF EXISTS (
        SELECT 1
        FROM issue_metadata_sync s
        LEFT JOIN issues i ON i.id = s.issue_id
        LEFT JOIN issue_dispatch d ON d.issue_id = s.issue_id
        WHERE i.id IS NULL
           OR s.project_id IS DISTINCT FROM i.project_id
           OR s.title IS DISTINCT FROM i.title
           OR s.status IS DISTINCT FROM i.status
           OR s.importance IS DISTINCT FROM i.importance
           OR s.agent_eligible IS DISTINCT FROM i.agent_eligible
           OR s.spec_complete IS DISTINCT FROM i.spec_complete
           OR s.version IS DISTINCT FROM i.version
           OR s.rank IS DISTINCT FROM COALESCE(d.rank, 0)
           OR s.updated_at < GREATEST(
               i.updated_at,
               COALESCE(d.updated_at, i.updated_at),
               COALESCE(
                   (SELECT max(il.created_at)
                    FROM issue_labels il
                    WHERE il.issue_id = s.issue_id),
                   i.updated_at
               )
           )
           OR s.labels IS DISTINCT FROM COALESCE(
               (SELECT array_agg(il.label ORDER BY il.label)
                FROM issue_labels il
                WHERE il.issue_id = s.issue_id),
               ARRAY[]::text[]
           )
    ) THEN
        RAISE EXCEPTION 'projection verification failed: issue metadata sync row is stale';
    END IF;
    IF EXISTS (
        SELECT 1
        FROM human_issue_sync s
        LEFT JOIN issues i ON i.id = s.issue_id
        LEFT JOIN teams t ON t.id = i.team_id
        LEFT JOIN projects p ON p.id = i.project_id
        LEFT JOIN issue_dispatch d ON d.issue_id = i.id
        WHERE i.id IS NULL
           OR t.id IS NULL
           OR p.id IS NULL
           OR d.issue_id IS NULL
           OR NOT (
               EXISTS (
                   SELECT 1 FROM project_memberships pm
                   WHERE pm.project_id = i.project_id
                     AND pm.account_id = s.account_id
                     AND pm.revoked_at IS NULL
               ) OR EXISTS (
                   SELECT 1 FROM team_memberships tm
                   WHERE tm.team_id = i.team_id
                     AND tm.account_id = s.account_id
                     AND tm.revoked_at IS NULL
               )
           )
           OR s.team_name IS DISTINCT FROM t.name
           OR s.team_key IS DISTINCT FROM t.key
           OR s.project_name IS DISTINCT FROM p.name
           OR s.title IS DISTINCT FROM i.title
           OR s.body IS DISTINCT FROM COALESCE(
               (SELECT dp.plain_text
                FROM document_bindings db
                JOIN document_projections dp ON dp.document_id = db.document_id
                WHERE db.resource_kind = 'issue'
                  AND db.resource_id = i.id
                  AND db.role = 'description'
                LIMIT 1),
               i.body
           )
           OR s.status IS DISTINCT FROM i.status
           OR s.importance IS DISTINCT FROM i.importance
           OR s.agent_eligible IS DISTINCT FROM i.agent_eligible
           OR s.spec_complete IS DISTINCT FROM i.spec_complete
           OR s.rank IS DISTINCT FROM d.rank
           OR s.dispatch_version IS DISTINCT FROM d.dispatch_version
           OR s.assignee_account_id IS DISTINCT FROM i.assignee_account_id
           OR s.labels IS DISTINCT FROM COALESCE(
               (SELECT array_agg(il.label ORDER BY il.label)
                FROM issue_labels il
                WHERE il.issue_id = s.issue_id),
               ARRAY[]::text[]
           )
    ) THEN
        RAISE EXCEPTION 'projection verification failed: human issue sync row is stale or unauthorized';
    END IF;
    IF EXISTS (
        SELECT 1
        FROM issues i
        WHERE (
            EXISTS (
                SELECT 1 FROM project_memberships pm
                WHERE pm.project_id = i.project_id
                  AND pm.revoked_at IS NULL
            ) OR EXISTS (
                SELECT 1 FROM team_memberships tm
                WHERE tm.team_id = i.team_id
                  AND tm.revoked_at IS NULL
            )
        )
        AND NOT EXISTS (
            SELECT 1 FROM human_issue_sync s
            WHERE s.issue_id = i.id
        )
    ) THEN
        RAISE EXCEPTION 'projection verification failed: accessible human issue sync row is missing';
    END IF;
    IF EXISTS (
        SELECT 1
        FROM navigation_sync n
        LEFT JOIN organization_memberships om
          ON om.organization_id = n.organization_id
         AND om.account_id = n.account_id
         AND om.revoked_at IS NULL
        LEFT JOIN team_memberships tm
          ON tm.team_id = n.team_id
         AND tm.account_id = n.account_id
         AND tm.revoked_at IS NULL
        LEFT JOIN project_teams pt
          ON pt.project_id = n.project_id
         AND pt.team_id = n.team_id
        LEFT JOIN organizations o ON o.id = n.organization_id
        LEFT JOIN teams t ON t.id = n.team_id
        LEFT JOIN projects p ON p.id = n.project_id
        WHERE om.account_id IS NULL
           OR tm.account_id IS NULL
           OR pt.project_id IS NULL
           OR o.id IS NULL
           OR t.id IS NULL
           OR p.id IS NULL
           OR n.organization_name IS DISTINCT FROM o.name
           OR n.organization_role IS DISTINCT FROM om.role
           OR n.organization_has_logo IS DISTINCT FROM (o.logo_bytes IS NOT NULL AND o.logo_content_type IS NOT NULL)
           OR n.team_name IS DISTINCT FROM t.name
           OR n.team_key IS DISTINCT FROM t.key
           OR n.team_emoji IS DISTINCT FROM t.emoji
           OR n.project_name IS DISTINCT FROM p.name
           OR n.project_role IS DISTINCT FROM pt.role
    ) THEN
        RAISE EXCEPTION 'projection verification failed: navigation sync row is stale or unauthorized';
    END IF;
    IF EXISTS (
        SELECT 1
        FROM organization_memberships om
        JOIN team_memberships tm
          ON tm.account_id = om.account_id
         AND tm.revoked_at IS NULL
        JOIN teams t ON t.id = tm.team_id AND t.organization_id = om.organization_id
        JOIN project_teams pt ON pt.team_id = t.id
        JOIN projects p ON p.id = pt.project_id AND p.organization_id = om.organization_id
        WHERE om.revoked_at IS NULL
          AND NOT EXISTS (
              SELECT 1
              FROM navigation_sync n
              WHERE n.account_id = om.account_id
                AND n.organization_id = om.organization_id
                AND n.team_id = t.id
                AND n.project_id = p.id
          )
    ) THEN
        RAISE EXCEPTION 'projection verification failed: accessible navigation row is missing';
    END IF;
    IF EXISTS (
        SELECT 1
        FROM document_bindings b
        LEFT JOIN documents d ON d.id = b.document_id
        WHERE d.id IS NULL OR d.deleted_at IS NOT NULL
    ) THEN
            RAISE EXCEPTION 'projection verification failed: document binding without a live document';
    END IF;
    IF EXISTS (
        SELECT 1
        FROM human_document_sync s
        LEFT JOIN documents d ON d.id = s.document_id
        LEFT JOIN document_projections p ON p.document_id = s.document_id
        WHERE d.id IS NULL
           OR d.deleted_at IS NOT NULL
           OR s.organization_id IS DISTINCT FROM d.organization_id
           OR s.kind IS DISTINCT FROM d.kind
           OR s.title IS DISTINCT FROM d.title
           OR s.parent_document_id IS DISTINCT FROM d.parent_document_id
           OR s.position IS DISTINCT FROM d.position
           OR s.owner_team_id IS DISTINCT FROM d.owner_team_id
           OR s.owner_project_id IS DISTINCT FROM d.owner_project_id
           OR s.provisioning_state IS DISTINCT FROM d.provisioning_state
           OR s.created_by IS DISTINCT FROM d.created_by
           OR s.created_at IS DISTINCT FROM d.created_at
           OR s.updated_at IS DISTINCT FROM d.updated_at
           OR s.current_revision IS DISTINCT FROM p.content_revision
           OR s.plain_text IS DISTINCT FROM p.plain_text
           OR s.sanitized_html IS DISTINCT FROM p.sanitized_html
           OR NOT (
               (
                   d.owner_team_id IS NULL
                   AND d.owner_project_id IS NULL
                   AND EXISTS (
                       SELECT 1 FROM organization_memberships om
                       WHERE om.organization_id = d.organization_id
                         AND om.account_id = s.account_id
                         AND om.revoked_at IS NULL
                   )
               ) OR (
                   d.owner_team_id IS NOT NULL
                   AND EXISTS (
                       SELECT 1 FROM team_memberships tm
                       WHERE tm.team_id = d.owner_team_id
                         AND tm.account_id = s.account_id
                         AND tm.revoked_at IS NULL
                   )
               ) OR (
                   d.owner_project_id IS NOT NULL
                   AND EXISTS (
                       SELECT 1 FROM project_memberships pm
                       WHERE pm.project_id = d.owner_project_id
                         AND pm.account_id = s.account_id
                         AND pm.revoked_at IS NULL
                   )
               ) OR EXISTS (
                   SELECT 1
                   FROM document_bindings b
                   JOIN issues i ON b.resource_kind = 'issue' AND b.resource_id = i.id
                   WHERE b.document_id = d.id
                     AND (
                         EXISTS (
                             SELECT 1 FROM team_memberships tm
                             WHERE tm.team_id = i.team_id
                               AND tm.account_id = s.account_id
                               AND tm.revoked_at IS NULL
                         ) OR EXISTS (
                             SELECT 1 FROM project_memberships pm
                             WHERE pm.project_id = i.project_id
                               AND pm.account_id = s.account_id
                               AND pm.revoked_at IS NULL
                         )
                     )
               )
           )
    ) THEN
        RAISE EXCEPTION 'projection verification failed: human document sync row is stale or unauthorized';
    END IF;
    IF EXISTS (
        SELECT 1
        FROM human_accounts account
        JOIN documents d ON d.deleted_at IS NULL
        WHERE (
            (
                d.owner_team_id IS NULL
                AND d.owner_project_id IS NULL
                AND EXISTS (
                    SELECT 1 FROM organization_memberships om
                    WHERE om.organization_id = d.organization_id
                      AND om.account_id = account.id
                      AND om.revoked_at IS NULL
                )
            ) OR (
                d.owner_team_id IS NOT NULL
                AND EXISTS (
                    SELECT 1 FROM team_memberships tm
                    WHERE tm.team_id = d.owner_team_id
                      AND tm.account_id = account.id
                      AND tm.revoked_at IS NULL
                )
            ) OR (
                d.owner_project_id IS NOT NULL
                AND EXISTS (
                    SELECT 1 FROM project_memberships pm
                    WHERE pm.project_id = d.owner_project_id
                      AND pm.account_id = account.id
                      AND pm.revoked_at IS NULL
                )
            ) OR EXISTS (
                SELECT 1
                FROM document_bindings b
                JOIN issues i ON b.resource_kind = 'issue' AND b.resource_id = i.id
                WHERE b.document_id = d.id
                  AND (
                      EXISTS (
                          SELECT 1 FROM team_memberships tm
                          WHERE tm.team_id = i.team_id
                            AND tm.account_id = account.id
                            AND tm.revoked_at IS NULL
                      ) OR EXISTS (
                          SELECT 1 FROM project_memberships pm
                          WHERE pm.project_id = i.project_id
                            AND pm.account_id = account.id
                            AND pm.revoked_at IS NULL
                      )
                  )
            )
        )
        AND NOT EXISTS (
            SELECT 1
            FROM human_document_sync s
            WHERE s.account_id = account.id
              AND s.document_id = d.id
        )
    ) THEN
            RAISE EXCEPTION 'projection verification failed: accessible human document sync row is missing';
    END IF;
    IF EXISTS (
        SELECT 1
        FROM human_agent_sync s
        LEFT JOIN agent_roles r ON r.id = s.agent_role_id
        LEFT JOIN teams t ON t.id = s.team_id
        LEFT JOIN projects p ON p.id = s.project_id
        WHERE r.id IS NULL
           OR t.id IS NULL
           OR p.id IS NULL
           OR s.team_id IS DISTINCT FROM r.team_id
           OR s.project_id IS DISTINCT FROM r.project_id
           OR s.display_name IS DISTINCT FROM r.display_name
           OR s.owner_account_id IS DISTINCT FROM r.owner_account_id
           OR s.capabilities IS DISTINCT FROM r.capabilities
           OR s.revoked_at IS DISTINCT FROM r.revoked_at
           OR s.active_session_count IS DISTINCT FROM (
               SELECT count(*)
               FROM sessions agent_session
               WHERE agent_session.agent_role_id = r.id
                 AND agent_session.state = 'active'
           )
           OR s.sessions IS DISTINCT FROM COALESCE(
               (
                   SELECT jsonb_agg(
                       jsonb_build_object(
                           'id', agent_session.id,
                           'project_id', agent_session.project_id,
                           'team_id', agent_session.team_id,
                           'agent_role_id', agent_session.agent_role_id,
                           'state', agent_session.state,
                           'max_lifetime_ends_at', agent_session.max_lifetime_ends_at,
                           'heartbeat_at', agent_session.heartbeat_at,
                           'last_action_at', agent_session.last_action_at,
                           'revoked_at', agent_session.revoked_at
                       ) ORDER BY agent_session.created_at DESC, agent_session.id DESC
                   )
                   FROM sessions agent_session
                   WHERE agent_session.agent_role_id = r.id
               ),
               '[]'::jsonb
           )
           OR NOT EXISTS (
               SELECT 1
               FROM team_memberships tm
               WHERE tm.team_id = r.team_id
                 AND tm.account_id = s.account_id
                 AND tm.revoked_at IS NULL
           )
    ) THEN
        RAISE EXCEPTION 'projection verification failed: human agent sync row is stale or unauthorized';
    END IF;
    IF EXISTS (
        SELECT 1
        FROM human_accounts account
        JOIN team_memberships tm
          ON tm.account_id = account.id
         AND tm.revoked_at IS NULL
        JOIN agent_roles r ON r.team_id = tm.team_id
        WHERE NOT EXISTS (
            SELECT 1
            FROM human_agent_sync s
            WHERE s.account_id = account.id
              AND s.agent_role_id = r.id
        )
    ) THEN
        RAISE EXCEPTION 'projection verification failed: accessible human agent sync row is missing';
    END IF;
    IF EXISTS (
        SELECT 1
        FROM approval_sync a
        LEFT JOIN approval_requests r ON r.id = a.id
        LEFT JOIN project_memberships pm
          ON pm.project_id = a.project_id
         AND pm.account_id = a.account_id
         AND pm.revoked_at IS NULL
         AND pm.role IN ('owner', 'admin')
        WHERE r.id IS NULL
           OR r.state <> 'pending'
           OR pm.account_id IS NULL
           OR a.project_id IS DISTINCT FROM r.project_id
           OR a.issue_id IS DISTINCT FROM r.issue_id
           OR a.target_version IS DISTINCT FROM r.target_version
           OR a.proposed_operation IS DISTINCT FROM r.proposed_operation
           OR a.state IS DISTINCT FROM r.state
    ) THEN
        RAISE EXCEPTION 'projection verification failed: approval sync row is stale or unauthorized';
    END IF;
    IF EXISTS (
        SELECT 1
        FROM approval_requests r
        JOIN project_memberships pm
          ON pm.project_id = r.project_id
         AND pm.revoked_at IS NULL
         AND pm.role IN ('owner', 'admin')
        WHERE r.state = 'pending'
          AND NOT EXISTS (
              SELECT 1
              FROM approval_sync a
              WHERE a.account_id = pm.account_id
                AND a.id = r.id
          )
    ) THEN
        RAISE EXCEPTION 'projection verification failed: pending approval sync row is missing';
    END IF;
    IF EXISTS (
        SELECT 1
        FROM document_projections p
        LEFT JOIN documents d ON d.id = p.document_id
        WHERE d.id IS NULL OR d.deleted_at IS NOT NULL
    ) THEN
        RAISE EXCEPTION 'projection verification failed: document projection without a live document';
    END IF;
    IF EXISTS (
        SELECT 1
        FROM document_activity a
        LEFT JOIN documents d ON d.id = a.document_id
        WHERE d.id IS NULL
    ) THEN
        RAISE EXCEPTION 'projection verification failed: document activity without a document';
    END IF;
    IF EXISTS (
        SELECT 1
        FROM comments c
        LEFT JOIN issue_activity_sync a ON a.id = c.id
        WHERE a.id IS NULL
    ) THEN
        RAISE EXCEPTION 'projection verification failed: comment missing from activity sync';
    END IF;
    IF EXISTS (
        SELECT 1
        FROM audit_records r
        LEFT JOIN issue_activity_sync a ON a.id = r.id
        WHERE r.target_type = 'issue'
          AND r.target_id IS NOT NULL
          AND a.id IS NULL
    ) THEN
        RAISE EXCEPTION 'projection verification failed: issue audit missing from activity sync';
    END IF;
    IF EXISTS (
        SELECT 1
        FROM document_activity d
        JOIN document_bindings b
          ON b.document_id = d.document_id
         AND b.resource_kind = 'issue'
         AND b.role = 'description'
        LEFT JOIN issue_activity_sync a ON a.id = d.id
        WHERE a.id IS NULL
    ) THEN
        RAISE EXCEPTION 'projection verification failed: issue document activity missing from activity sync';
    END IF;
    IF EXISTS (
        SELECT 1
        FROM documents d
        JOIN document_loro_snapshots s ON s.document_id = d.id
        LEFT JOIN document_projections p ON p.document_id = d.id
        WHERE d.deleted_at IS NULL
          AND (p.document_id IS NULL OR p.content_revision < s.source_revision)
    ) THEN
        RAISE EXCEPTION 'projection verification failed: document projection is behind its Loro source revision';
    END IF;
    IF EXISTS (
        SELECT 1
        FROM document_loro_snapshots s
        LEFT JOIN document_versions v
          ON v.document_id = s.document_id
         AND v.revision = s.source_revision
        WHERE v.revision IS NULL
           OR v.frontiers IS DISTINCT FROM s.frontiers
    ) THEN
        RAISE EXCEPTION 'projection verification failed: Loro snapshot frontier has no matching document version';
    END IF;
END $$;

SELECT 'projection_verification' AS check, 'passed' AS result;
