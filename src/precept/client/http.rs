use std::sync::Arc;
use reqwest::cookie::{CookieStore, Jar};
use url::Url;
use keyring::Entry;
use crate::precept::{Identity, PreceptID};
use super::{make_access_token, AccessTokenType, KEYRING_SERVICE_NAME, ARTILECT_INSTANCE_ID};

#[derive(Clone, PartialEq)]
pub enum CookieStorage {
    Precept { id: PreceptID },
    User { base_url: Arc<str> },
}

impl CookieStorage {
    fn get_or_create_token(&self) -> String {
        match self {
            CookieStorage::Precept { id } => {
                // Generate token for precept
                let identity = Identity::Precept {
                    id: id.clone(),
                    on_behalf_of: None,
                };
                make_access_token(identity, AccessTokenType::Precept)
                    .expect("Failed to create precept token")
                    .token
                    .to_string()
            }
            CookieStorage::User { base_url } => {
                // Retrieve token from keyring
                let key = format!("user-token-{}", base_url);
                let entry = Entry::new(
                    &format!("{}.{}", ARTILECT_INSTANCE_ID, KEYRING_SERVICE_NAME),
                    &key,
                )
                    .expect("Invalid keyring entry");

                entry
                    .get_password()
                    .unwrap_or_else(|_| String::new())
            }
        }
    }

    fn set_token(&self, token: &str) {
        if let CookieStorage::User { base_url } = self {
            let key = format!("user-token-{}", base_url);
            let entry = Entry::new(
                &format!("{}.{}", ARTILECT_INSTANCE_ID, KEYRING_SERVICE_NAME),
                &key,
            )
                .expect("Invalid keyring entry");

            let _ = entry.set_password(token);
        }
    }
}

#[derive(Clone)]
pub struct HttpClient {
    client: reqwest::Client,
    cookie_storage: CookieStorage,
}

impl HttpClient {
    pub fn new(cookie_storage: CookieStorage) -> Self {
        let jar = Arc::new(Jar::default());

        // Pre-populate with token if available
        let token = cookie_storage.get_or_create_token();
        if !token.is_empty() {
            // Add cookie for any URL - reqwest will handle the domain matching
            if let Ok(url) = Url::parse("http://localhost") {
                jar.add_cookie_str(&format!("at={}", token), &url);
            }
        }

        let client = reqwest::Client::builder()
            .cookie_provider(jar)
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            cookie_storage,
        }
    }

    pub fn client(&self) -> &reqwest::Client {
        &self.client
    }
}

impl PartialEq for HttpClient {
    fn eq(&self, other: &Self) -> bool {
        self.cookie_storage == other.cookie_storage
    }
}
}