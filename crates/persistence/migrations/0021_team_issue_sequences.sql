CREATE TABLE team_issue_sequences (
    team_id UUID PRIMARY KEY REFERENCES teams(id),
    next_number BIGINT NOT NULL CHECK (next_number > 0)
);

INSERT INTO team_issue_sequences (team_id, next_number)
SELECT t.id,
       COALESCE(MAX(NULLIF((regexp_match(i.display_key, '-([0-9]+)$'))[1], '')::BIGINT), 0) + 1
FROM teams t
LEFT JOIN issues i ON i.team_id = t.id
GROUP BY t.id
ON CONFLICT (team_id) DO NOTHING;
