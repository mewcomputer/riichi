ALTER TABLE issues
    ADD COLUMN importance TEXT NOT NULL DEFAULT 'none'
    CHECK (importance IN ('none', 'low', 'medium', 'high', 'urgent'));
