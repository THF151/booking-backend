ALTER TABLE email_templates ADD COLUMN event_id TEXT;
CREATE INDEX idx_email_templates_event_id ON email_templates(event_id);