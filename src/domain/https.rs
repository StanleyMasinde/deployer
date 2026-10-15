use serde::Serialize;

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Https {
    pub enabled: bool,
    pub domain: Option<String>,
    pub expires_at: Option<i64>,
    pub error: Option<String>,
}

impl Https {
    pub fn status(&self, domain: &str, now: i64) -> &'static str {
        if !self.enabled {
            return "disabled";
        }
        if self.domain.as_deref() == Some(domain)
            && let Some(expiry) = self.expires_at
        {
            return if expiry > now { "active" } else { "expired" };
        }
        if self.error.is_some() {
            "failed"
        } else {
            "pending"
        }
    }
}
