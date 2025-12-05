ALTER TABLE bookings ADD COLUMN status TEXT NOT NULL DEFAULT 'CONFIRMED';
ALTER TABLE bookings ADD COLUMN management_token TEXT;
-- Index for secure lookup
CREATE INDEX idx_bookings_management_token ON bookings(management_token);
-- Index for filtering availability
CREATE INDEX idx_bookings_status ON bookings(status);

ALTER TABLE events ADD COLUMN allow_customer_cancel BOOLEAN NOT NULL DEFAULT TRUE;
ALTER TABLE events ADD COLUMN allow_customer_reschedule BOOLEAN NOT NULL DEFAULT TRUE;