#[cfg(unix)]
use dialog::DialogBox;

pub struct Password(pub String);

impl Password {
    pub fn from_user_input(prompt: &str, _title: &str) -> Self {
        let mut password = "".to_string();
        while password.trim().is_empty() {
            #[cfg(unix)]
            {
                password = dialog::Password::new(prompt)
                    .title(_title)
                    .show()
                    .unwrap_or(None)
                    .unwrap_or("".to_string())
                    .trim()
                    .to_string();
            }
            #[cfg(not(unix))]
            {
                password = rpassword::prompt_password(prompt).unwrap_or_else(|_| String::new());
            }
        }
        password.into()
    }
}

impl<T> From<T> for Password
where
    T: AsRef<str>,
{
    fn from(value: T) -> Self {
        Self(value.as_ref().to_string())
    }
}
