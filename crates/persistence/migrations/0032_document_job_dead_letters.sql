ALTER TABLE document_jobs
    ADD COLUMN dead_lettered_at TIMESTAMPTZ;

CREATE INDEX document_jobs_dead_letter_idx
    ON document_jobs (dead_lettered_at, completed_at)
    WHERE dead_lettered_at IS NOT NULL;
