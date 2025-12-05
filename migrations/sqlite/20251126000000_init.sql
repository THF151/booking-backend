-- Clean start
DROP TABLE IF EXISTS bookings;
DROP TABLE IF EXISTS invitees;
DROP TABLE IF EXISTS events;
DROP TABLE IF EXISTS users;
DROP TABLE IF EXISTS tenants;

CREATE TABLE tenants (
                         id TEXT PRIMARY KEY NOT NULL,
                         name TEXT NOT NULL,
                         slug TEXT NOT NULL UNIQUE,
                         created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE users (
                       id TEXT PRIMARY KEY NOT NULL,
                       tenant_id TEXT NOT NULL,
                       username TEXT NOT NULL,
                       password_hash TEXT NOT NULL,
                       role TEXT NOT NULL,
                       created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
                       FOREIGN KEY (tenant_id) REFERENCES tenants(id),
                       UNIQUE(tenant_id, username)
);

CREATE TABLE events (
                        id TEXT PRIMARY KEY NOT NULL,
                        tenant_id TEXT NOT NULL,
                        slug TEXT NOT NULL,
                        title_en TEXT NOT NULL,
                        title_de TEXT NOT NULL,
                        desc_en TEXT NOT NULL,
                        desc_de TEXT NOT NULL,
                        location TEXT NOT NULL,
                        payout TEXT NOT NULL,
                        active_start TIMESTAMPTZ NOT NULL,
                        active_end TIMESTAMPTZ NOT NULL,
                        duration_min INTEGER NOT NULL,
                        interval_min INTEGER NOT NULL,
                        max_participants INTEGER NOT NULL,
                        image_url TEXT NOT NULL,
                        config_json TEXT NOT NULL,
                        access_mode TEXT NOT NULL DEFAULT 'OPEN', -- OPEN, RESTRICTED, CLOSED
                        created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
                        FOREIGN KEY (tenant_id) REFERENCES tenants(id),
                        UNIQUE(tenant_id, slug)
);

CREATE TABLE invitees (
                          id TEXT PRIMARY KEY NOT NULL,
                          tenant_id TEXT NOT NULL,
                          event_id TEXT NOT NULL,
                          token TEXT NOT NULL UNIQUE,
                          email TEXT, -- Optional, for admin reference
                          status TEXT NOT NULL DEFAULT 'ACTIVE', -- ACTIVE, USED
                          created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
                          FOREIGN KEY (event_id) REFERENCES events(id),
                          FOREIGN KEY (tenant_id) REFERENCES tenants(id)
);

CREATE TABLE bookings (
                          id TEXT PRIMARY KEY NOT NULL,
                          tenant_id TEXT NOT NULL,
                          event_id TEXT NOT NULL,
                          invitee_id TEXT, -- Nullable (if public booking)
                          start_time TIMESTAMPTZ NOT NULL,
                          end_time TIMESTAMPTZ NOT NULL,
                          customer_name TEXT NOT NULL,
                          customer_email TEXT NOT NULL,
                          created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
                          FOREIGN KEY (event_id) REFERENCES events(id),
                          FOREIGN KEY (invitee_id) REFERENCES invitees(id)
);

CREATE INDEX idx_bookings_lookup ON bookings(event_id, start_time);
CREATE INDEX idx_invitees_token ON invitees(token);