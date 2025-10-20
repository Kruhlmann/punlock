use crate::bw::BitwardenApi;
use crate::bw::BitwardenToken;
use crate::data::BitwardenClientCredentials;
use crate::data::Domain;
use crate::data::Email;

use super::BitwardenUrl;

pub struct BitwardenHttpClient {
    token: BitwardenToken,
    client: reqwest::Client,
    url: BitwardenUrl,
}

impl BitwardenHttpClient {
    pub async fn new(
        email: &Email,
        credentials: &BitwardenClientCredentials,
        domain: Domain,
    ) -> anyhow::Result<BitwardenHttpClient> {
        tracing::debug!(?email, ?domain, "logging in");
        let url = BitwardenUrl(domain);
        let client = reqwest::Client::new();
        let params = [
            ("grant_type", "client_credentials"),
            ("scope", "api"),
            ("client_id", &credentials.id),
            ("client_secret", &credentials.secret),
            ("device_identifier", env!("CARGO_PKG_NAME")),
            ("device_type", env!("CARGO_PKG_NAME")),
            ("device_name", env!("CARGO_PKG_NAME")),
        ];
        let token = client
            .post(url.as_identity_url())
            .form(&params)
            .send()
            .await?
            .error_for_status()
            .inspect_err(|error| tracing::error!(?error, "http error"))?
            .json::<BitwardenToken>()
            .await
            .inspect_err(|error| tracing::error!(?error, "deserialization error"))?;

        tracing::debug!(?email, ?url, "login successful");
        let bitwarden_client = Self { client, token, url };
        Ok(bitwarden_client)
    }
}

#[async_trait::async_trait]
impl BitwardenApi for BitwardenHttpClient {
    async fn get_item(&self, id: &str) -> anyhow::Result<serde_json::Value> {
        tracing::debug!(?id, "request item");
        let response = self
            .client
            .get(self.url.as_cipher_url(id))
            .bearer_auth(&self.token.access_token)
            .send()
            .await?
            .error_for_status()?
            .json::<serde_json::Value>()
            .await?;
        tracing::debug!(?id, ?response, "item retrieved");
        Ok(response)
    }

    fn get_token(&self) -> &BitwardenToken {
        &self.token
    }
}
