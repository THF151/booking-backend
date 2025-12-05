ALTER TABLE bookings ADD COLUMN token TEXT;
CREATE INDEX idx_bookings_token ON bookings(token);