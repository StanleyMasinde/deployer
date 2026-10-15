use super::Email;
use crate::config::{Mail, SmtpSecurity};
use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
    message::{Mailbox, MultiPart, SinglePart},
    transport::smtp::{
        authentication::Credentials,
        client::{Tls, TlsParameters},
    },
};
use std::{future::Future, time::Duration};

#[derive(Debug)]
pub struct SendFailure {
    pub retryable: bool,
    pub code: &'static str,
}

pub trait EmailSender: Send + Sync {
    fn deliver(
        &self,
        id: u64,
        email: &Email,
    ) -> impl Future<Output = Result<(), SendFailure>> + Send;
}

pub struct SmtpSender {
    transport: AsyncSmtpTransport<Tokio1Executor>,
    from: Mailbox,
}

impl SmtpSender {
    pub fn new(config: &Mail) -> anyhow::Result<Self> {
        config.validate()?;
        let smtp = &config.smtp;
        let tls = match smtp.security {
            SmtpSecurity::Starttls => Tls::Required(TlsParameters::new(smtp.host.clone())?),
            SmtpSecurity::Tls => Tls::Wrapper(TlsParameters::new(smtp.host.clone())?),
            SmtpSecurity::None => Tls::None,
        };
        let mut builder = AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&smtp.host)
            .port(smtp.port)
            .tls(tls)
            .timeout(Some(Duration::from_secs(10)));

        if let (Some(username), Some(password)) = (&smtp.username, &smtp.password) {
            builder = builder.credentials(Credentials::new(username.clone(), password.clone()));
        }

        Ok(Self {
            transport: builder.build(),
            from: Mailbox::new(Some(config.from_name.clone()), config.from_email.parse()?),
        })
    }

    fn message(&self, id: u64, email: &Email) -> anyhow::Result<Message> {
        email.validate()?;
        let domain = self.from.email.domain();

        Ok(Message::builder()
            .from(self.from.clone())
            .to(email.recipient.parse()?)
            .message_id(Some(format!("<deployer-job-{id}@{domain}>")))
            .subject(&email.subject)
            .multipart(
                MultiPart::alternative()
                    .singlepart(SinglePart::plain(email.text.clone()))
                    .singlepart(SinglePart::html(email.html.clone())),
            )?)
    }
}

impl EmailSender for SmtpSender {
    async fn deliver(&self, id: u64, email: &Email) -> Result<(), SendFailure> {
        let message = self.message(id, email).map_err(|_| SendFailure {
            retryable: false,
            code: "invalid_email",
        })?;

        self.transport
            .send(message)
            .await
            .map_err(|error| SendFailure {
                retryable: !error.is_permanent(),
                code: if error.is_permanent() {
                    "smtp_rejected"
                } else {
                    "smtp_unavailable"
                },
            })?;

        Ok(())
    }
}

pub enum Transport {
    Disabled,
    Smtp(Box<SmtpSender>),
}

impl Transport {
    pub fn new(config: Option<&Mail>) -> anyhow::Result<Self> {
        match config {
            Some(config) => Ok(Self::Smtp(Box::new(SmtpSender::new(config)?))),
            None => Ok(Self::Disabled),
        }
    }
}

impl EmailSender for Transport {
    async fn deliver(&self, id: u64, email: &Email) -> Result<(), SendFailure> {
        match self {
            Self::Smtp(sender) => sender.deliver(id, email).await,
            Self::Disabled => Err(SendFailure {
                retryable: false,
                code: "mail_not_configured",
            }),
        }
    }
}
