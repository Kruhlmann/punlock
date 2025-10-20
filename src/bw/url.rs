use crate::data::Domain;

#[derive(Debug)]
pub struct BitwardenUrl(pub Domain);

impl BitwardenUrl {
    pub fn as_identity_url(&self) -> String {
        format!("https://{}/identity/connect/token", self.0)
    }

    pub fn as_vault_url(&self) -> String {
        format!("https://{}/api/sync", self.0)
    }

    pub fn as_cipher_url(&self, id: &str) -> String {
        format!("https://{}/api/ciphers/{}", self.0, id)
    }
}
