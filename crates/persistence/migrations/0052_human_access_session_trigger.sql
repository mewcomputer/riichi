CREATE OR REPLACE FUNCTION notify_human_access_from_session()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    IF TG_OP = 'UPDATE'
       AND OLD.revoked_at IS NOT DISTINCT FROM NEW.revoked_at
       AND OLD.expires_at IS NOT DISTINCT FROM NEW.expires_at THEN
        RETURN NULL;
    END IF;

    PERFORM notify_human_access_event(
        CASE WHEN TG_OP = 'DELETE' THEN OLD.account_id ELSE NEW.account_id END
    );
    RETURN NULL;
END;
$$;
