CREATE TABLE booking_labels (
                                id TEXT PRIMARY KEY NOT NULL,
                                tenant_id TEXT NOT NULL,
                                name TEXT NOT NULL,
                                color TEXT NOT NULL DEFAULT '#808080',
                                created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
                                FOREIGN KEY (tenant_id) REFERENCES tenants(id) ON DELETE CASCADE
);

ALTER TABLE bookings ADD COLUMN label_id TEXT REFERENCES booking_labels(id) ON DELETE SET NULL;