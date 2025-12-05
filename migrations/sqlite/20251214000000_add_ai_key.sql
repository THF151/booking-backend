ALTER TABLE tenants ADD COLUMN ai_api_key TEXT;
ALTER TABLE tenants ADD COLUMN ai_provider TEXT DEFAULT 'gemini';