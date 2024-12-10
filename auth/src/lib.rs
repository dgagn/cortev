use std::time::Duration;

use sqlx::FromRow;
pub use sqlx::MySqlPool;
use timebox::Timebox;

pub mod timebox;

pub struct AuthLayer {
    pool: MySqlPool,
}

#[derive(Debug, FromRow)]
pub struct GenericUser {
    username: String,
    password: String,
}

impl AuthLayer {
    async fn retrieve_by_credentials(&self, username: &str) -> Option<GenericUser> {
        let query = "select username, password from users where username = ?";

        sqlx::query_as::<_, GenericUser>(query)
            .bind(username)
            .fetch_optional(&self.pool)
            .await
            .unwrap()
    }

    async fn validate_credentials(&self, user: &GenericUser, password: String) -> bool {
        if password.is_empty() {
            return false;
        }

        let user_password = user.password.clone();
        tokio::task::spawn_blocking(move || bcrypt::verify(&password, &user_password).unwrap())
            .await
            .unwrap()
    }

    async fn has_valid_credentials(
        &self,
        user: GenericUser,
        password: String,
    ) -> Option<GenericUser> {
        let timebox = Timebox::new(Duration::from_millis(200));
        let valid = self.validate_credentials(&user, password).await;

        if valid {
            return Some(user);
        }
        timebox.complete().await;
        None
    }

    pub async fn attempt(&self, username: &str, password: String) {
        let credentials = self.retrieve_by_credentials(username).await;
        if let Some(user) = credentials {
            let user = self.has_valid_credentials(user, password).await;
            if let Some(user) = user {
                // login
            }
        }
    }
}
