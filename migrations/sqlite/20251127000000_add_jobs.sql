CREATE TABLE jobs (
                      id TEXT PRIMARY KEY NOT NULL,
                      job_type TEXT NOT NULL, -- 'CONFIRMATION', 'REMINDER'
                      payload JSONB NOT NULL,
                      execute_at TIMESTAMPTZ NOT NULL,
                      status TEXT NOT NULL DEFAULT 'PENDING', -- 'PENDING', 'COMPLETED', 'FAILED'
                      created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_jobs_status_execute ON jobs(status, execute_at);