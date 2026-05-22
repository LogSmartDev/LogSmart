use axum::{
    extract::{ConnectInfo, Request, State},
    middleware::Next,
    response::{IntoResponse, Response},
};
use dashmap::DashMap;
use governor::{
    Quota, RateLimiter,
    clock::DefaultClock,
    state::{InMemoryState, direct::NotKeyed},
};
use std::net::IpAddr;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Instant;

/// Rate limit constants with environment variable overrides
pub fn get_login_ip_limit() -> u32 {
    std::env::var("RATE_LIMIT_LOGIN_IP")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5)
}

pub fn get_register_ip_limit() -> u32 {
    std::env::var("RATE_LIMIT_REGISTER_IP")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10)
}

pub fn get_general_ip_limit() -> u32 {
    std::env::var("RATE_LIMIT_GENERAL_IP")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(60)
}

pub fn get_login_email_limit() -> u32 {
    std::env::var("RATE_LIMIT_LOGIN_EMAIL")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10)
}

pub fn get_register_email_limit() -> u32 {
    std::env::var("RATE_LIMIT_REGISTER_EMAIL")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(20)
}

type IpLimiter = DashMap<
    IpAddr,
    (
        Arc<RateLimiter<NotKeyed, InMemoryState, DefaultClock>>,
        Instant,
    ),
>;
type StringLimiter = DashMap<
    String,
    (
        Arc<RateLimiter<NotKeyed, InMemoryState, DefaultClock>>,
        Instant,
    ),
>;

#[derive(Clone)]
pub struct RateLimitState {
    pub ip_login_limiter: Arc<IpLimiter>,
    pub ip_register_limiter: Arc<IpLimiter>,
    pub ip_general_limiter: Arc<IpLimiter>,
    pub email_login_limiter: Arc<StringLimiter>,
    pub email_register_limiter: Arc<StringLimiter>,
    pub export_limiter: Arc<StringLimiter>,
    pub disabled: bool,
}

impl RateLimitState {
    pub fn new() -> Self {
        let disabled = std::env::var("DISABLE_RATE_LIMIT")
            .map(|v| v.eq_ignore_ascii_case("true") || v == "1")
            .unwrap_or(false);

        if disabled {
            tracing::warn!("Rate limiting is DISABLED via DISABLE_RATE_LIMIT environment variable");
        }

        Self {
            ip_login_limiter: Arc::new(DashMap::new()),
            ip_register_limiter: Arc::new(DashMap::new()),
            ip_general_limiter: Arc::new(DashMap::new()),
            email_login_limiter: Arc::new(DashMap::new()),
            email_register_limiter: Arc::new(DashMap::new()),
            export_limiter: Arc::new(DashMap::new()),
            disabled,
        }
    }

    #[must_use]
    pub fn disabled() -> Self {
        Self {
            ip_login_limiter: Arc::new(DashMap::new()),
            ip_register_limiter: Arc::new(DashMap::new()),
            ip_general_limiter: Arc::new(DashMap::new()),
            email_login_limiter: Arc::new(DashMap::new()),
            email_register_limiter: Arc::new(DashMap::new()),
            export_limiter: Arc::new(DashMap::new()),
            disabled: true,
        }
    }

    fn get_or_create_ip_limiter(
        map: &IpLimiter,
        ip: IpAddr,
        quota: Quota,
    ) -> Arc<RateLimiter<NotKeyed, InMemoryState, DefaultClock>> {
        let now = Instant::now();
        match map.entry(ip) {
            dashmap::mapref::entry::Entry::Occupied(mut entry) => {
                entry.get_mut().1 = now;
                entry.get().0.clone()
            }
            dashmap::mapref::entry::Entry::Vacant(entry) => {
                let limiter = Arc::new(RateLimiter::direct(quota));
                entry.insert((limiter.clone(), now));
                limiter
            }
        }
    }

    fn get_or_create_string_limiter(
        map: &StringLimiter,
        key: String,
        quota: Quota,
    ) -> Arc<RateLimiter<NotKeyed, InMemoryState, DefaultClock>> {
        let now = Instant::now();
        match map.entry(key) {
            dashmap::mapref::entry::Entry::Occupied(mut entry) => {
                entry.get_mut().1 = now;
                entry.get().0.clone()
            }
            dashmap::mapref::entry::Entry::Vacant(entry) => {
                let limiter = Arc::new(RateLimiter::direct(quota));
                entry.insert((limiter.clone(), now));
                limiter
            }
        }
    }

    pub fn cleanup_expired_entries(&self, max_age: std::time::Duration) {
        let now = Instant::now();

        self.ip_login_limiter
            .retain(|_, (_, last_access)| now.duration_since(*last_access) < max_age);

        self.ip_register_limiter
            .retain(|_, (_, last_access)| now.duration_since(*last_access) < max_age);

        self.ip_general_limiter
            .retain(|_, (_, last_access)| now.duration_since(*last_access) < max_age);

        self.email_login_limiter
            .retain(|_, (_, last_access)| now.duration_since(*last_access) < max_age);

        self.email_register_limiter
            .retain(|_, (_, last_access)| now.duration_since(*last_access) < max_age);
    }

    pub fn cleanup_export_entries(&self, max_age: std::time::Duration) {
        let now = Instant::now();

        self.export_limiter
            .retain(|_, (_, last_access)| now.duration_since(*last_access) < max_age);
    }

    pub fn spawn_cleanup_task(self) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(300));
            loop {
                interval.tick().await;
                self.cleanup_expired_entries(std::time::Duration::from_secs(3600));
                self.cleanup_export_entries(std::time::Duration::from_secs(8 * 24 * 60 * 60));
                tracing::debug!("Rate limiter cleanup completed");
            }
        });
    }

    /// Checks if a login attempt from a given IP is allowed by rate limits.
    ///
    /// # Panics
    /// Panics if the configured quota is invalid.
    #[must_use]
    pub fn check_login(&self, ip: IpAddr) -> bool {
        if self.disabled {
            return true;
        }
        let quota = Quota::per_minute(NonZeroU32::new(get_login_ip_limit()).unwrap());
        let limiter = Self::get_or_create_ip_limiter(&self.ip_login_limiter, ip, quota);
        limiter.check().is_ok()
    }

    /// Checks if a login attempt for a given email is allowed by rate limits.
    ///
    /// # Panics
    /// Panics if the configured quota is invalid.
    #[must_use]
    pub fn check_login_email(&self, email: &str) -> bool {
        if self.disabled {
            return true;
        }
        let quota = Quota::per_minute(NonZeroU32::new(get_login_email_limit()).unwrap());
        let limiter = Self::get_or_create_string_limiter(
            &self.email_login_limiter,
            email.to_lowercase(),
            quota,
        );
        limiter.check().is_ok()
    }

    /// Checks if a registration attempt from a given IP is allowed by rate limits.
    ///
    /// # Panics
    /// Panics if the configured quota is invalid.
    #[must_use]
    pub fn check_register(&self, ip: IpAddr) -> bool {
        if self.disabled {
            return true;
        }
        let quota = Quota::per_minute(NonZeroU32::new(get_register_ip_limit()).unwrap());
        let limiter = Self::get_or_create_ip_limiter(&self.ip_register_limiter, ip, quota);
        limiter.check().is_ok()
    }

    /// Checks if a registration attempt for a given email is allowed by rate limits.
    ///
    /// # Panics
    /// Panics if the configured quota is invalid.
    #[must_use]
    pub fn check_register_email(&self, email: &str) -> bool {
        if self.disabled {
            return true;
        }
        let quota = Quota::per_minute(NonZeroU32::new(get_register_email_limit()).unwrap());
        let limiter = Self::get_or_create_string_limiter(
            &self.email_register_limiter,
            email.to_lowercase(),
            quota,
        );
        limiter.check().is_ok()
    }

    /// Checks if a general request from a given IP is allowed by rate limits.
    ///
    /// # Panics
    /// Panics if the configured quota is invalid.
    #[must_use]
    pub fn check_general(&self, ip: IpAddr) -> bool {
        if self.disabled {
            return true;
        }
        let quota = Quota::per_minute(NonZeroU32::new(get_general_ip_limit()).unwrap());
        let limiter = Self::get_or_create_ip_limiter(&self.ip_general_limiter, ip, quota);
        limiter.check().is_ok()
    }

    /// Checks if an OAuth request from a given IP is allowed by rate limits.
    ///
    /// # Panics
    /// Panics if the configured quota is invalid.
    #[must_use]
    pub fn check_oauth(&self, ip: IpAddr) -> bool {
        if self.disabled {
            return true;
        }
        let quota = Quota::per_minute(NonZeroU32::new(10).unwrap());
        let limiter = Self::get_or_create_ip_limiter(&self.ip_general_limiter, ip, quota);
        limiter.check().is_ok()
    }

    /// Checks if a company export request is allowed (once per week per company).
    ///
    /// # Panics
    /// Panics if the quota period is invalid.
    #[must_use]
    pub fn check_export(&self, company_id: &str) -> bool {
        if self.disabled {
            return true;
        }
        let quota = Quota::with_period(std::time::Duration::from_secs(7 * 24 * 60 * 60))
            .expect("Invalid export quota period");
        let limiter =
            Self::get_or_create_string_limiter(&self.export_limiter, company_id.to_string(), quota);
        limiter.check().is_ok()
    }
}

impl Default for RateLimitState {
    fn default() -> Self {
        Self::new()
    }
}

fn extract_email_from_body(body: &[u8]) -> Option<String> {
    serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .and_then(|json| {
            json.get("email")
                .and_then(|e| e.as_str().map(std::string::ToString::to_string))
        })
}

pub async fn rate_limit_middleware(
    State(app_state): State<crate::AppState>,
    ConnectInfo(addr): ConnectInfo<std::net::SocketAddr>,
    req: Request,
    next: Next,
) -> Response {
    let ip = req
        .headers()
        .get("x-forwarded-for")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.split(',').next())
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or_else(|| addr.ip());

    if app_state.rate_limit.disabled {
        tracing::debug!("Rate limiting disabled, allowing request from {}", ip);
        return next.run(req).await;
    }

    let path = req.uri().path().to_string();

    let (parts, body) = req.into_parts();
    let body_bytes = axum::body::to_bytes(body, usize::MAX)
        .await
        .unwrap_or_default();

    let email = extract_email_from_body(&body_bytes);

    let ip_allowed = if path.contains("/auth/login") {
        app_state.rate_limit.check_login(ip)
    } else if path.contains("/auth/register") {
        app_state.rate_limit.check_register(ip)
    } else if path.contains("/auth/google/") || path.contains("/auth/oauth/") {
        app_state.rate_limit.check_oauth(ip)
    } else if path.contains("/export") {
        app_state.rate_limit.check_general(ip)
    } else {
        app_state.rate_limit.check_general(ip)
    };

    if !ip_allowed {
        app_state.metrics.increment_rate_limit_hits();
        tracing::warn!("Rate limit exceeded for IP: {}", ip);
        return crate::error::AppError::TooManyRequests(
            "Rate limit exceeded for your IP address. Please try again later.".to_string(),
        )
        .into_response();
    }

    if let Some(email_str) = email.as_ref() {
        let email_allowed = if path.contains("/auth/login") {
            app_state.rate_limit.check_login_email(email_str)
        } else if path.contains("/auth/register") {
            app_state.rate_limit.check_register_email(email_str)
        } else {
            true
        };

        if !email_allowed {
            app_state.metrics.increment_rate_limit_hits();
            tracing::warn!("Rate limit exceeded for email: {}", email_str);
            return crate::error::AppError::TooManyRequests(
                "Rate limit exceeded for this email address. Please try again later.".to_string(),
            )
            .into_response();
        }
    }

    let new_body = axum::body::Body::from(body_bytes);
    let req = Request::from_parts(parts, new_body);

    next.run(req).await
}
