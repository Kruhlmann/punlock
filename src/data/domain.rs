#[derive(Clone, Debug, serde::Serialize)]
pub struct Domain(String);

impl<T> From<T> for Domain
where
    T: Into<String>,
{
    fn from(value: T) -> Self {
        Self(value.into())
    }
}

impl std::fmt::Display for Domain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
