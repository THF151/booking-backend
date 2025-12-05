CREATE TABLE event_overrides (
                                 id TEXT PRIMARY KEY NOT NULL,
                                 event_id TEXT NOT NULL,
                                 date DATE NOT NULL,
                                 is_unavailable BOOLEAN NOT NULL DEFAULT 0,
                                 override_config_json TEXT,
                                 location TEXT,
                                 host_name TEXT,
                                 created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
                                 FOREIGN KEY (event_id) REFERENCES events(id),
                                 UNIQUE(event_id, date)
);

ALTER TABLE bookings ADD COLUMN location TEXT;