CREATE TABLE human_saved_view_pins (
    account_id UUID NOT NULL REFERENCES human_accounts(id) ON DELETE CASCADE,
    view_id UUID NOT NULL REFERENCES human_saved_views(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (account_id, view_id)
);

CREATE INDEX human_saved_view_pins_account_idx
    ON human_saved_view_pins (account_id, created_at DESC);
