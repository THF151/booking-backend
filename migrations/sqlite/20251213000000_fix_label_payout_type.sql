-- SQLite does not support altering column type easily.
-- We drop and re-add. Data in this column will be reset to 0.
ALTER TABLE booking_labels DROP COLUMN payout;
ALTER TABLE booking_labels ADD COLUMN payout INTEGER NOT NULL DEFAULT 0;