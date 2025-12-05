CREATE TABLE email_template_versions (
                                         id TEXT PRIMARY KEY NOT NULL,
                                         template_id TEXT NOT NULL,
                                         subject_template TEXT NOT NULL,
                                         body_template TEXT NOT NULL,
                                         created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
                                         FOREIGN KEY (template_id) REFERENCES email_templates(id) ON DELETE CASCADE
);

CREATE INDEX idx_template_versions_template_id ON email_template_versions(template_id);