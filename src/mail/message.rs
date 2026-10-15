use anyhow::ensure;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Email {
    pub recipient: String,
    pub subject: String,
    pub text: String,
    pub html: String,
}

pub fn validate_address(address: &str) -> anyhow::Result<()> {
    ensure!(
        address.len() <= 254
            && address.is_ascii()
            && !address.chars().any(char::is_control)
            && email_address::EmailAddress::parse_with_options(
                address,
                email_address::Options::default().without_display_text(),
            )
            .is_ok(),
        "enter a valid email address"
    );

    Ok(())
}

impl Email {
    pub fn validate(&self) -> anyhow::Result<()> {
        validate_address(&self.recipient)?;
        ensure!(
            !self.subject.trim().is_empty()
                && self.subject.len() <= 255
                && !self.subject.chars().any(char::is_control),
            "invalid email subject"
        );
        ensure!(
            !self.text.is_empty() || !self.html.is_empty(),
            "email body is empty"
        );
        ensure!(
            self.text.len() + self.html.len() <= 24 * 1024,
            "email body is too large"
        );

        Ok(())
    }
}
