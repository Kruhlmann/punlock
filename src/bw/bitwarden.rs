pub struct EncryptedBitwardenItem(pub String);

impl<T> From<T> for EncryptedBitwardenItem
where
    T: AsRef<str>,
{
    fn from(value: T) -> Self {
        Self(value.as_ref().to_string())
    }
}

impl Into<String> for EncryptedBitwardenItem {
    fn into(self) -> String {
        self.0.clone()
    }
}

#[async_trait::async_trait]
pub trait Bitwarden {
    async fn get_item(&self, id: &str, query: &str) -> anyhow::Result<EncryptedBitwardenItem>;
}
