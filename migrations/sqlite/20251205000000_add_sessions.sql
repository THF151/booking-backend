ALTER TABLE events ADD COLUMN schedule_type TEXT NOT NULL DEFAULT 'RECURRING';

ALTER TABLE event_overrides ADD COLUMN override_max_participants INTEGER;

CREATE TABLE event_sessions (
                                id TEXT PRIMARY KEY NOT NULL,
                                event_id TEXT NOT NULL,
                                start_time TIMESTAMPTZ NOT NULL,
                                end_time TIMESTAMPTZ NOT NULL,
                                max_participants INTEGER NOT NULL,
                                location TEXT,
                                host_name TEXT,
                                created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
                                FOREIGN KEY (event_id) REFERENCES events(id) ON DELETE CASCADE
);

CREATE INDEX idx_event_sessions_lookup ON event_sessions(event_id, start_time);