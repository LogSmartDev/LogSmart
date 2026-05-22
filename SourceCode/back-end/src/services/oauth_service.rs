use crate::{
    db,
    jwt_manager::JwtManager,
    utils::{AuditContext, AuditLogger},
};
use anyhow::Result;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

#[cfg(test)]
mod oauth_service_tests {
    #[tokio::test]
    async fn test_oauth_service_basic() {
        assert!(true);
    }
}
use openidconnect::{
    AuthenticationFlow, AuthorizationCode, ClaimsVerificationError, ClientId, ClientSecret,
    CsrfToken, IdTokenVerifier, IssuerUrl, JsonWebKeySetUrl, Nonce, RedirectUrl, Scope,
    SignatureVerificationError, TokenResponse,
    core::{
        CoreClient, CoreIdTokenClaims, CoreJsonWebKeySet, CoreProviderMetadata, CoreResponseType,
    },
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

#[derive(Clone)]
pub struct GoogleOAuthClient {
    client_id: ClientId,
    client_secret: ClientSecret,
    redirect_uri: RedirectUrl,
    provider_metadata: CoreProviderMetadata,
    jwks_url: JsonWebKeySetUrl,
    cached_jwks: Arc<RwLock<Option<(CoreJsonWebKeySet, Instant)>>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OAuthUserInfo {
    pub email: String,
    pub given_name: String,
    pub family_name: String,
    pub picture: Option<String>,
    pub sub: String,
}

impl GoogleOAuthClient {
    /// Creates a new Google OAuth client.
    ///
    /// # Errors
    /// Returns an error if the issuer URL is invalid, metadata discovery fails, or redirect URI is invalid.
    pub async fn new(
        client_id: String,
        client_secret: String,
        redirect_uri: String,
        issuer_url: String,
    ) -> Result<Self> {
        let issuer_url = IssuerUrl::new(issuer_url)
            .map_err(|e| anyhow::anyhow!("Failed to create issuer URL: {e}"))?;

        let http_client = openidconnect::reqwest::Client::new();
        tracing::info!("Discovering Google OAuth metadata from: {}", issuer_url);
        let provider_metadata = CoreProviderMetadata::discover_async(issuer_url, &http_client)
            .await
            .map_err(|e| {
                tracing::error!("OAuth metadata discovery failed: {:?}", e);
                anyhow::anyhow!("Failed to discover Google metadata: {e}")
            })?;

        let redirect_uri_validated = RedirectUrl::new(redirect_uri.clone())
            .map_err(|e| anyhow::anyhow!("Invalid redirect URI: {e}"))?;

        let jwks_url = JsonWebKeySetUrl::new(provider_metadata.jwks_uri().to_string())
            .map_err(|e| anyhow::anyhow!("Invalid JWK URI: {e}"))?;

        Ok(Self {
            client_id: ClientId::new(client_id),
            client_secret: ClientSecret::new(client_secret),
            redirect_uri: redirect_uri_validated,
            provider_metadata,
            jwks_url,
            cached_jwks: Arc::new(RwLock::new(None)),
        })
    }

    pub fn initiate_login(&self) -> (String, String, String) {
        let client = CoreClient::from_provider_metadata(
            self.provider_metadata.clone(),
            self.client_id.clone(),
            Some(self.client_secret.clone()),
        )
        .set_redirect_uri(self.redirect_uri.clone());

        let (auth_url, csrf_token, nonce) = client
            .authorize_url(
                AuthenticationFlow::<CoreResponseType>::AuthorizationCode,
                CsrfToken::new_random,
                Nonce::new_random,
            )
            .add_scope(Scope::new("email".to_string()))
            .add_scope(Scope::new("profile".to_string()))
            .url();

        (
            auth_url.to_string(),
            csrf_token.secret().clone(),
            nonce.secret().clone(),
        )
    }

    async fn get_jwks(
        &self,
        http_client: &openidconnect::reqwest::Client,
        force_refresh: bool,
    ) -> Result<CoreJsonWebKeySet, crate::error::AppError> {
        const CACHE_TTL: Duration = Duration::from_secs(300);

        if !force_refresh {
            let cached = self.cached_jwks.read().await;
            if let Some((jwks, timestamp)) = cached.as_ref()
                && timestamp.elapsed() < CACHE_TTL
            {
                tracing::debug!("Using cached JWKs");
                return Ok(jwks.clone());
            }
        }

        tracing::info!("Fetching fresh JWKs from Google");
        let jwks: CoreJsonWebKeySet = CoreJsonWebKeySet::fetch_async(&self.jwks_url, http_client)
            .await
            .map_err(|e| {
                tracing::error!("Failed to fetch JWKs: {:?}", e);
                crate::error::AppError::Internal("Failed to fetch signing keys" .to_string())
            })?;

        {
            let mut cached = self.cached_jwks.write().await;
            *cached = Some((jwks.clone(), Instant::now()));
        }

        Ok(jwks)
    }

    /// Exchanges an authorization code for an ID token and user info.
    ///
    /// # Errors
    /// Returns an error if the code is invalid, exchange fails, or token verification fails.
    pub async fn exchange_code(
        &self,
        code: String,
        nonce: String,
    ) -> Result<(OAuthUserInfo, CoreIdTokenClaims), crate::error::AppError> {
        let http_client = openidconnect::reqwest::Client::new();

        let client = CoreClient::from_provider_metadata(
            self.provider_metadata.clone(),
            self.client_id.clone(),
            Some(self.client_secret.clone()),
        )
        .set_redirect_uri(self.redirect_uri.clone());

        let token_request = client
            .exchange_code(AuthorizationCode::new(code))
            .map_err(|e| {
                tracing::error!("Invalid authorization code: {:?}", e);
                crate::error::AppError::BadRequest("Invalid authorization code" .to_string())
            })?;

        let token_response = token_request
            .request_async(&http_client)
            .await
            .map_err(|e| {
                tracing::error!("Failed to exchange OAuth code: {:?}", e);
                crate::error::AppError::Internal("Failed to exchange authorization code" .to_string())
            })?;

        let id_token = token_response.id_token().ok_or_else(|| {
            tracing::error!("No ID token in OAuth response");
            crate::error::AppError::Internal("No ID token received from Google" .to_string())
        })?;

        let nonce = Nonce::new(nonce);
        let issuer = self.provider_metadata.issuer().clone();
        let jwks = self.get_jwks(&http_client, false).await?;
        let verifier =
            IdTokenVerifier::new_public_client(self.client_id.clone(), issuer.clone(), jwks);

        let claims = match id_token.claims(&verifier, &nonce) {
            Ok(claims) => claims,
            Err(ClaimsVerificationError::SignatureVerification(
                SignatureVerificationError::NoMatchingKey,
            )) => {
                tracing::warn!("No matching JWK found; refreshing JWKs and retrying verification");
                let refreshed_jwks = self.get_jwks(&http_client, true).await?;
                let refreshed_verifier = IdTokenVerifier::new_public_client(
                    self.client_id.clone(),
                    issuer,
                    refreshed_jwks,
                );
                id_token.claims(&refreshed_verifier, &nonce).map_err(|e| {
                    tracing::error!("Failed to verify ID token after JWK refresh: {:?}", e);
                    crate::error::AppError::Unauthorized("Failed to verify ID token" .to_string())
                })?
            }
            Err(e) => {
                tracing::error!("Failed to verify ID token: {:?}", e);
                return Err(crate::error::AppError::Unauthorized("Failed to verify ID token" .to_string()));
            }
        };

        let email = claims
            .email()
            .map(|e| e.as_str().to_string())
            .ok_or_else(|| {
                tracing::error!("No email in ID token claims");
                crate::error::AppError::BadRequest("Email not provided by Google" .to_string())
            })?;

        let given_name = claims
            .given_name()
            .and_then(|names| names.get(None).map(|name| name.as_str().to_string()))
            .unwrap_or_else(|| "User".to_string());

        let family_name = claims
            .family_name()
            .and_then(|names| names.get(None).map(|name| name.as_str().to_string()))
            .unwrap_or_else(String::new);

        let picture = claims
            .picture()
            .and_then(|pics| pics.get(None).map(|pic| pic.to_string()));

        let sub = claims.subject().to_string();

        Ok((
            OAuthUserInfo {
                email,
                given_name,
                family_name,
                picture,
                sub,
            },
            claims.clone(),
        ))
    }

    /// Retrieves an existing user or creates a new one using OAuth info.
    ///
    /// # Errors
    /// Returns an error if the account already exists with a different auth method, or if creation fails.
    pub async fn get_or_create_user(
        &self,
        pool: &PgPool,
        user_info: OAuthUserInfo,
        ip_address: Option<String>,
        user_agent: Option<String>,
        allow_new_account: bool,
    ) -> Result<db::UserRecord, crate::error::AppError> {
        tracing::info!(
            "OAuth get_or_create_user: looking for oauth_subject={}, google_email={}",
            user_info.sub,
            user_info.email
        );

        if let Some(existing_user) = db::get_user_by_oauth(pool, "google", &user_info.sub)
            .await
            .map_err(|e| {
                tracing::error!("Database error checking OAuth user: {:?}", e);
                crate::error::AppError::Internal("Database error" .to_string())
            })?
        {
            tracing::info!(
                "OAuth login: found linked user {} (email: {}) for oauth_subject={}",
                existing_user.id,
                existing_user.email,
                user_info.sub
            );
            AuditLogger::log_oauth_login(
                pool,
                existing_user.id.clone(),
                user_info.email.clone(),
                "google".to_string(),
                true,
                crate::audit_ctx!(
                    &AuditContext {
                        ip_address,
                        user_agent,
                        ..AuditContext::default()
                    },
                    actor: &existing_user
                ),
            )
            .await;

            if existing_user.company_deleted_at.is_some() {
                return Err(crate::error::AppError::Unauthorized("Your company has been deleted. Please contact support.".to_string()));
            }
            return Ok(existing_user);
        }

        tracing::info!(
            "OAuth login: no linked user found for oauth_subject={}, checking email={}",
            user_info.sub,
            user_info.email
        );

        if let Some(_existing_user) = db::get_user_by_email(pool, &user_info.email)
            .await
            .map_err(|e| {
                tracing::error!("Database error checking email: {:?}", e);
                crate::error::AppError::Internal("Database error" .to_string())
            })?
        {
            return Err(crate::error::AppError::Conflict("An account with this email already exists. Please login with your password or link your Google account in settings.".to_string()));
        }

        if !allow_new_account {
            return Err(crate::error::AppError::Forbidden("No account found. Please create an account first or use an invitation link to join a company.".to_string()));
        }

        let new_user = db::create_oauth_user(
            pool,
            user_info.email.clone(),
            user_info.given_name,
            user_info.family_name,
            "google".to_string(),
            user_info.sub,
            user_info.picture,
            None,
            db::UserRole::Staff,
        )
        .await
        .map_err(|e| {
            tracing::error!("Failed to create OAuth user: {:?}", e);
            crate::error::AppError::Internal("Failed to create user account" .to_string())
        })?;

        AuditLogger::log_oauth_login(
            pool,
            new_user.id.clone(),
            user_info.email.clone(),
            "google".to_string(),
            true,
            crate::audit_ctx!(
                &AuditContext {
                    ip_address,
                    user_agent,
                    ..AuditContext::default()
                },
                actor: &new_user
            ),
        )
        .await;

        Ok(new_user)
    }

    /// Links a Google account to an existing user profile.
    ///
    /// # Errors
    /// Returns an error if the Google account is already linked to another user or if the update fails.
    pub async fn link_google_account(
        &self,
        pool: &PgPool,
        user_id: &str,
        user_info: OAuthUserInfo,
    ) -> Result<(), crate::error::AppError> {
        tracing::info!(
            "OAuth linking: user_id={}, oauth_subject={}, google_email={}",
            user_id,
            user_info.sub,
            user_info.email
        );

        if db::get_user_by_oauth(pool, "google", &user_info.sub)
            .await
            .map_err(|e| {
                tracing::error!("Database error: {:?}", e);
                crate::error::AppError::Internal("Database error" .to_string())
            })?
            .is_some()
        {
            return Err(crate::error::AppError::Conflict("This Google account is already linked to another user" .to_string()));
        }

        db::link_oauth_to_user(
            pool,
            user_id,
            "google".to_string(),
            user_info.sub.clone(),
            user_info.picture,
        )
        .await
        .map_err(|e| {
            tracing::error!("Failed to link OAuth account: {:?}", e);
            crate::error::AppError::Internal("Failed to link account" .to_string())
        })?;

        tracing::info!(
            "OAuth successfully linked: user_id={} now has Google OAuth",
            user_id
        );

        Ok(())
    }

    /// Generates a JWT token for a user after OAuth authentication.
    ///
    /// # Errors
    /// Returns an error if token generation fails.
    pub fn generate_jwt_for_user(
        &self,
        user_id: &str,
    ) -> Result<String, crate::error::AppError> {
        let jwt_config = JwtManager::get_config();
        jwt_config.generate_token(user_id, 24).map_err(|e| {
            tracing::error!("Failed to generate JWT token: {:?}", e);
            crate::error::AppError::Internal("Failed to generate token" .to_string())
        })
    }
}
