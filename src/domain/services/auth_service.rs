use std::sync::Arc;
use crate::domain::{
    models::{auth::{Claims, RefreshTokenRecord}, user::User},
    ports::AuthRepository
};
use crate::error::AppError;
use crate::config::Config;
use jsonwebtoken::{encode, EncodingKey, Header, Algorithm};
use uuid::Uuid;
use chrono::{Utc, Duration};
use rand::{distributions::Alphanumeric, Rng};
use sha2::{Sha256, Digest};

pub struct AuthService {
    repo: Arc<dyn AuthRepository>,
    config: Config,
    encoding_key: EncodingKey,
}

impl AuthService {
    pub fn new(repo: Arc<dyn AuthRepository>, config: Config) -> Self {
        let encoding_key = EncodingKey::from_ed_pem(config.jwt_secret_key.as_bytes())
            .expect("Invalid JWT Private Key PEM");

        Self { repo, config, encoding_key }
    }

    pub async fn login(&self, user: &User) -> Result<(String, String, String), AppError> {
        let family_id = Uuid::new_v4();
        self.issue_token_pair(user, family_id, 1).await
    }

    pub async fn refresh(&self, raw_refresh_token: &str, user: &User) -> Result<(String, String, String), AppError> {
        let token_hash = self.hash_token(raw_refresh_token);

        let record = self.repo.find_refresh_token(&token_hash).await?
            .ok_or(AppError::Unauthorized)?;

        if record.expires_at < Utc::now() {
            self.repo.delete_refresh_token(&token_hash).await?;
            return Err(AppError::Unauthorized);
        }

        self.repo.delete_refresh_token(&token_hash).await?;
        self.issue_token_pair(user, record.family_id, record.generation_id + 1).await
    }

    pub async fn logout(&self, raw_refresh_token: &str) -> Result<(), AppError> {
        let token_hash = self.hash_token(raw_refresh_token);
        self.repo.delete_refresh_token(&token_hash).await
    }

    async fn issue_token_pair(&self, user: &User, family_id: Uuid, generation_id: i32) -> Result<(String, String, String), AppError> {
        let csrf_token: String = rand::thread_rng().sample_iter(&Alphanumeric).take(32).map(char::from).collect();
        let now = Utc::now();
        let exp = (now + Duration::minutes(15)).timestamp() as usize;

        let claims = Claims {
            iss: self.config.auth_issuer.clone(),
            sub: user.id.clone(),
            aud: "booking-frontend".to_string(),
            exp,
            iat: now.timestamp() as usize,
            jti: Uuid::new_v4().to_string(),
            tenant_id: user.tenant_id.clone(),
            role: user.role.clone(),
            csrf_token: csrf_token.clone(),
        };

        let access_token = encode(&Header::new(Algorithm::EdDSA), &claims, &self.encoding_key)
            .map_err(|e| {
                tracing::error!("JWT encoding failed: {}", e);
                AppError::Internal
            })?;

        let refresh_token: String = rand::thread_rng().sample_iter(&Alphanumeric).take(64).map(char::from).collect();
        let refresh_token_hash = self.hash_token(&refresh_token);

        let refresh_record = RefreshTokenRecord {
            token_hash: refresh_token_hash,
            user_id: user.id.clone(),
            tenant_id: user.tenant_id.clone(),
            family_id,
            generation_id,
            expires_at: now + Duration::days(7),
            created_at: now,
        };

        self.repo.create_refresh_token(&refresh_record).await?;
        Ok((access_token, refresh_token, csrf_token))
    }

    pub fn hash_token(&self, token: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(token.as_bytes());
        hex::encode(hasher.finalize())
    }
}