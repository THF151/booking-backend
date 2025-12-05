-- Drop the default value first to avoid casting errors during type change
ALTER TABLE booking_labels ALTER COLUMN payout DROP DEFAULT;

-- Convert payout from TEXT to INTEGER
-- Assumes format like '15€', strips non-numeric chars.
ALTER TABLE booking_labels
ALTER COLUMN payout TYPE INTEGER
    USING (REGEXP_REPLACE(payout, '[^0-9-]', '', 'g')::INTEGER);

-- Set the new default value
ALTER TABLE booking_labels ALTER COLUMN payout SET DEFAULT 0;