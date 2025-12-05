CREATE TABLE refresh_tokens (
                                token_hash VARCHAR(255) PRIMARY KEY NOT NULL,
                                user_id TEXT NOT NULL,
                                tenant_id TEXT NOT NULL,
                                family_id UUID NOT NULL,        -- Tracks the chain of rotated tokens
                                generation_id INTEGER NOT NULL, -- 1, 2, 3... increments on rotation
                                expires_at TIMESTAMPTZ NOT NULL,
                                created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
                                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE INDEX idx_refresh_tokens_family ON refresh_tokens(family_id);