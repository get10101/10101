use axum::async_trait;
use axum::routing::get;
use axum::routing::post;
use axum::Router;
use axum_login::AuthUser;
use axum_login::AuthnBackend;
use axum_login::UserId;
use serde::Deserialize;
use sha2::digest::FixedOutput;
use sha2::Digest;
use sha2::Sha256;
use std::error::Error;
use std::fmt::Display;
use std::fmt::Formatter;
use utoipa::ToSchema;

#[derive(Clone)]
pub struct Backend {
    pub(crate) hashed_password: String,
}

#[derive(Clone, Debug)]
pub struct User {
    password: String,
}

#[derive(Clone, Deserialize, ToSchema)]
pub struct Credentials {
    pub password: String,
}

#[derive(std::fmt::Debug)]
pub struct BackendError(String);

impl Display for BackendError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.to_string().fmt(f)
    }
}

impl Error for BackendError {}

#[async_trait]
impl AuthnBackend for Backend {
    type User = User;
    type Credentials = Credentials;
    type Error = BackendError;

    async fn authenticate(
        &self,
        creds: Self::Credentials,
    ) -> Result<Option<Self::User>, Self::Error> {
        let mut hasher = Sha256::new();
        hasher.update(creds.password.as_bytes());
        let hashed_password = hex::encode(hasher.finalize_fixed());

        let user = match hashed_password == self.hashed_password {
            true => Some(User {
                password: self.hashed_password.clone(),
            }),
            false => None,
        };

        Ok(user)
    }

    async fn get_user(&self, _user_id: &UserId<Self>) -> Result<Option<Self::User>, Self::Error> {
        Ok(Some(User {
            password: self.hashed_password.clone(),
        }))
    }
}

impl AuthUser for User {
    type Id = u64;

    fn id(&self) -> Self::Id {
        0
    }

    fn session_auth_hash(&self) -> &[u8] {
        self.password.as_bytes()
    }
}

pub fn router() -> Router {
    Router::new()
        .route("/api/login", post(post::login))
        .route("/api/logout", get(get::logout))
}

pub mod post {
    use super::*;
    use axum::http::StatusCode;
    use axum::response::IntoResponse;
    use axum::Json;
    use axum_login::AuthSession;

    #[utoipa::path(
        post,
        path = "/api/login",
        request_body = Credentials,
        responses(
        (status = 200, description = "If login was successful", body = ())
        )
    )]
    pub async fn login(
        mut auth_session: AuthSession<Backend>,
        creds: Json<Credentials>,
    ) -> impl IntoResponse {
        let user = match auth_session.authenticate(creds.0).await {
            Ok(Some(user)) => user,
            Ok(None) => {
                return StatusCode::UNAUTHORIZED.into_response();
            }
            Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        };

        if auth_session.login(&user).await.is_err() {
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }

        StatusCode::OK.into_response()
    }
}

pub mod get {
    use crate::api::AppError;
    use crate::auth::Backend;
    use axum_login::AuthSession;

    #[utoipa::path(
        get,
        path = "/api/logout",
        responses(
        (status = 200, description = "If logout was successful", body = ())
        )
    )]
    pub async fn logout(mut auth_session: AuthSession<Backend>) -> Result<(), AppError> {
        auth_session.logout().await?;
        Ok(())
    }
}
