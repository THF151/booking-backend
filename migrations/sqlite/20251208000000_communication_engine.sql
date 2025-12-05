CREATE TABLE email_templates (
                                 id TEXT PRIMARY KEY NOT NULL,
                                 tenant_id TEXT NOT NULL,
                                 name TEXT NOT NULL,
                                 subject_template TEXT NOT NULL,
                                 body_template TEXT NOT NULL,
                                 template_type TEXT NOT NULL DEFAULT 'mjml',
                                 created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
                                 updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
                                 FOREIGN KEY (tenant_id) REFERENCES tenants(id) ON DELETE CASCADE
);

CREATE TABLE notification_rules (
                                    id TEXT PRIMARY KEY NOT NULL,
                                    tenant_id TEXT NOT NULL,
                                    event_id TEXT,
                                    trigger_type TEXT NOT NULL,
                                    template_id TEXT NOT NULL,
                                    is_active BOOLEAN NOT NULL DEFAULT 1,
                                    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
                                    FOREIGN KEY (tenant_id) REFERENCES tenants(id) ON DELETE CASCADE,
                                    FOREIGN KEY (event_id) REFERENCES events(id) ON DELETE CASCADE,
                                    FOREIGN KEY (template_id) REFERENCES email_templates(id)
);

CREATE TABLE mail_logs (
                           id TEXT PRIMARY KEY NOT NULL,
                           job_id TEXT NOT NULL,
                           recipient TEXT NOT NULL,
                           template_id TEXT NOT NULL,
                           context_hash TEXT NOT NULL,
                           sent_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
                           status TEXT NOT NULL
);

CREATE INDEX idx_mail_logs_dedup ON mail_logs(recipient, template_id, context_hash);