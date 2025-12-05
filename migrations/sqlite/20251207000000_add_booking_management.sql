ALTER TABLE bookings ADD COLUMN status TEXT NOT NULL DEFAULT 'CONFIRMED';
ALTER TABLE bookings ADD COLUMN management_token TEXT;

UPDATE bookings
SET management_token = lower(hex(randomblob(16)))
WHERE management_token IS NULL;

CREATE INDEX idx_bookings_management_token ON bookings(management_token);
CREATE INDEX idx_bookings_status ON bookings(status);

ALTER TABLE events ADD COLUMN allow_customer_cancel BOOLEAN NOT NULL DEFAULT 1;
ALTER TABLE events ADD COLUMN allow_customer_reschedule BOOLEAN NOT NULL DEFAULT 1;
