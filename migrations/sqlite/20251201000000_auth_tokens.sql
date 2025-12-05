CREATE TABLE refresh_tokens (
                                token_hash TEXT PRIMARY KEY NOT NULL,
                                user_id TEXT NOT NULL,
                                tenant_id TEXT NOT NULL,
                                family_id TEXT NOT NULL,
                                generation_id INTEGER NOT NULL,
                                expires_at TIMESTAMPTZ NOT NULL,
                                created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
                                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE INDEX idx_refresh_tokens_family ON refresh_tokens(family_id);