ALTER TABLE issues ADD COLUMN due_date DATE;
ALTER TABLE issues ADD COLUMN snoozed_until DATE;
ALTER TABLE human_issue_sync ADD COLUMN due_date DATE;
ALTER TABLE human_issue_sync ADD COLUMN snoozed_until DATE;

CREATE OR REPLACE FUNCTION refresh_human_issue_sync_dates()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    UPDATE human_issue_sync
    SET due_date = NEW.due_date,
        snoozed_until = NEW.snoozed_until
    WHERE issue_id = NEW.id;
    RETURN NULL;
END;
$$;

CREATE TRIGGER human_issue_sync_z_dates_trigger
AFTER INSERT OR UPDATE ON issues
FOR EACH ROW EXECUTE FUNCTION refresh_human_issue_sync_dates();

UPDATE human_issue_sync sync
SET due_date = issues.due_date
  , snoozed_until = issues.snoozed_until
FROM issues
WHERE issues.id = sync.issue_id;
