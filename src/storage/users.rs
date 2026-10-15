use super::{Store, models::User};

impl Store {
    pub async fn root_account(&self) -> anyhow::Result<Option<User>> {
        Ok(User::filter_by_id(1)
            .first()
            .exec(&mut self.db.clone())
            .await?)
    }

    pub async fn create_root(
        &self,
        name: &str,
        email: &str,
        password_hash: &str,
        welcome: Option<&crate::mail::Email>,
    ) -> anyhow::Result<()> {
        let mut db = self.db.clone();
        let mut transaction = db.transaction().await?;

        // A fixed primary key makes initial account creation atomic across processes.
        toasty::create!(User {
            id: 1_u64,
            name,
            email,
            password_hash
        })
        .exec(&mut transaction)
        .await?;

        if let Some(email) = welcome {
            super::jobs::insert_email(&mut transaction, email).await?;
        }

        transaction.commit().await?;

        Ok(())
    }
}
