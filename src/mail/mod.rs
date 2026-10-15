mod message;
mod sender;
mod templates;

pub use message::{Email, validate_address};
pub use sender::{EmailSender, SendFailure, SmtpSender, Transport};
pub use templates::SystemEmail;

use crate::storage::Store;

pub struct QueuedMailer {
    store: Store,
}

impl QueuedMailer {
    pub fn new(store: Store) -> Self {
        Self { store }
    }

    pub async fn send(&self, recipient: &str, email: SystemEmail<'_>) -> anyhow::Result<u64> {
        self.store.enqueue_email(&email.render(recipient)?).await
    }
}
