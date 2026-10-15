use super::Email;
use askama::Template;

pub enum SystemEmail<'a> {
    Welcome { name: &'a str, link: &'a str },
}

impl SystemEmail<'_> {
    pub fn render(self, recipient: &str) -> anyhow::Result<Email> {
        let email = match self {
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
