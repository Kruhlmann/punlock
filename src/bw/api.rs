use serde::Deserialize;

#[derive(Clone, Deserialize)]
pub struct BitwardenToken {
    #[serde(rename = "Kdf")]
    pub kdf: i64,
    #[serde(rename = "KdfIterations")]
    pub kdf_iterations: u32,
    #[serde(rename = "KdfMemory")]
    pub kdf_memory: Option<i64>,
    #[serde(rename = "KdfParallelism")]
    pub kdf_parallelism: Option<String>,
    #[serde(rename = "Key")]
    pub key: String,
    #[serde(rename = "PrivateKey")]
    pub private_key: String,
    #[serde(rename = "ResetMasterPassword")]
    pub reset_master_password: bool,
    pub access_token: String,
    pub expires_in: usize,
    pub scope: String,
    pub token_type: String,
}

#[async_trait::async_trait]
pub trait BitwardenApi {
    async fn get_item(&self, id: &str) -> anyhow::Result<serde_json::Value>;
    fn get_token(&self) -> &BitwardenToken;
}
