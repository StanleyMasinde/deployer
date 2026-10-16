#[derive(toasty::Model)]
pub struct User {
    #[key]
    pub id: u64,
    #[column(type = varchar(128))]
    pub name: String,
    #[unique]
    #[column(type = varchar(254))]
    pub email: String,
    pub password_hash: String,
}

#[derive(toasty::Model)]
pub struct App {
    #[key]
    #[auto]
    pub id: u64,
    pub provider: String,
    #[unique]
    pub repository: String,
    pub branch: String,
    #[unique]
    #[column(type = varchar(253))]
    pub domain: String,
    #[column(type = text)]
    pub install_command: String,
    #[column(type = text)]
    pub build_config: Option<String>,
    #[column(type = text)]
    pub build_command: String,
    pub output_directory: String,
    pub spa: bool,
    #[column(type = text)]
    pub workflow_pr: Option<String>,
    pub workflow_pr_state: Option<String>,
    pub workflow_pr_updated_at: Option<String>,
    pub deleted_at: Option<i64>,
    pub https_enabled: Option<bool>,
    #[column(type = varchar(253))]
    pub https_domain: Option<String>,
    pub https_expires_at: Option<i64>,
    #[column(type = text)]
    pub https_error: Option<String>,
}

#[derive(toasty::Model)]
pub struct Deployment {
    #[key]
    #[auto]
    pub id: u64,
    #[unique]
    pub event_key: String,
    #[index]
    pub app_id: u64,
    pub run_id: u64,
    pub attempt: u32,
    pub commit_sha: String,
    #[index]
    pub status: String,
    #[column(type = text)]
    pub error: Option<String>,
    pub created_at: i64,
    pub finished_at: Option<i64>,
}

#[derive(toasty::Model)]
pub struct Job {
    #[key]
    #[auto]
    pub id: u64,
    #[unique]
    pub deployment_id: Option<u64>,
    pub kind: String,
    #[column(type = text)]
    pub payload: Option<String>,
    #[column(type = text)]
    pub error: Option<String>,
    #[index]
    pub status: String,
    pub attempts: u32,
    pub available_at: i64,
    pub created_at: i64,
    pub finished_at: Option<i64>,
}

#[derive(toasty::Model)]
pub struct PasswordReset {
    #[key]
    pub id: u64,
    pub token_hash: String,
    pub created_at: i64,
    pub expires_at: i64,
    pub used: bool,
}
