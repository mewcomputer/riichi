ALTER TABLE team_memberships
    DROP CONSTRAINT IF EXISTS team_memberships_role_check;

ALTER TABLE team_memberships
    ADD CONSTRAINT team_memberships_role_check
    CHECK (role IN ('viewer', 'member', 'admin', 'owner'));
