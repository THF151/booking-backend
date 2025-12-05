use std::env;

#[derive(Clone)]
pub struct Config {
    pub database_url: String,
    pub port: u16,
    pub mail_service_url: String,
    pub mail_service_token: String,
    pub jwt_secret_key: String, // Private key (PEM or Base64)
    pub jwt_public_key: String, // Public key (PEM or Base64)
    pub auth_issuer: String,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            database_url: env::var("DATABASE_URL").expect("DATABASE_URL must be set"),
            port: env::var("PORT").unwrap_or_else(|_| "3000".to_string()).parse().expect("PORT must be a number"),
            mail_service_url: env::var("MAIL_SERVICE_URL").unwrap_or_else(|_| "http://localhost:8000/api/v1/send".to_string()),
            mail_service_token: env::var("MAIL_SERVICE_TOKEN").unwrap_or_else(|_| "test-token-1".to_string()),
            jwt_secret_key: env::var("JWT_SECRET_KEY").expect("JWT_SECRET_KEY must be set (Ed25519 Private Key)"),
            jwt_public_key: env::var("JWT_PUBLIC_KEY").expect("JWT_PUBLIC_KEY must be set (Ed25519 Public Key)"),
            auth_issuer: env::var("AUTH_ISSUER").unwrap_or_else(|_| "https://api.booking-system.local".to_string()),
        }
    }
}