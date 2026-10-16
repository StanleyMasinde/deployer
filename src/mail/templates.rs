use super::Email;
use askama::Template;

pub enum SystemEmail<'a> {
    PasswordReset { name: &'a str, link: &'a str },
    PasswordChanged { name: &'a str, link: &'a str },
    Welcome { name: &'a str, link: &'a str },
}

impl SystemEmail<'_> {
    pub fn render(self, recipient: &str) -> anyhow::Result<Email> {
        let email = match self {
            Self::PasswordReset { name, link } => Email {
                recipient: recipient.into(),
                subject: "Reset your Deployer password".into(),
                text: ResetText { name, link }.render()?,
                html: ResetHtml { name, link }.render()?,
            },
            Self::PasswordChanged { name, link } => Email {
                recipient: recipient.into(),
                subject: "Your Deployer password was changed".into(),
                text: ChangedText { name, link }.render()?,
                html: ChangedHtml { name, link }.render()?,
            },
            Self::Welcome { name, link } => Email {
                recipient: recipient.into(),
                subject: "Welcome to Deployer".into(),
                text: WelcomeText { name, link }.render()?,
                html: WelcomeHtml { name, link }.render()?,
            },
        };

        email.validate()?;
        Ok(email)
    }
}

#[derive(Template)]
#[template(path = "emails/welcome.txt")]
struct WelcomeText<'a> {
    name: &'a str,
    link: &'a str,
}

#[derive(Template)]
#[template(path = "emails/welcome.html")]
struct WelcomeHtml<'a> {
    name: &'a str,
    link: &'a str,
}

#[derive(Template)]
#[template(path = "emails/password-reset.txt")]
struct ResetText<'a> {
    name: &'a str,
    link: &'a str,
}
#[derive(Template)]
#[template(path = "emails/password-reset.html")]
struct ResetHtml<'a> {
    name: &'a str,
    link: &'a str,
}
#[derive(Template)]
#[template(path = "emails/password-changed.txt")]
struct ChangedText<'a> {
    name: &'a str,
    link: &'a str,
}
#[derive(Template)]
#[template(path = "emails/password-changed.html")]
struct ChangedHtml<'a> {
    name: &'a str,
    link: &'a str,
}
