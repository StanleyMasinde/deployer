use deployer::{
    domain::App,
    providers::github::{webhook, workflow},
};
use hmac::{Hmac, KeyInit, Mac};
use serde_json::json;
use sha2::Sha256;

#[test]
fn webhook_signature_covers_exact_bytes() {
    let body = b"{\"action\":\"completed\"}";
    let mut mac = Hmac::<Sha256>::new_from_slice(b"secret").unwrap();
    mac.update(body);
    let signature = format!("sha256={}", hex::encode(mac.finalize().into_bytes()));

    webhook::verify("secret", &signature, body).unwrap();
    assert!(webhook::verify("secret", &signature, b"changed").is_err());
    assert!(webhook::verify("secret", "sha1=abc", body).is_err());
}

#[test]
fn only_successful_trusted_workflows_enter_the_queue() {
    let mut event = json!({
        "action":"completed", "installation":{"id":42}, "repository":{"full_name":"owner/repo"},
        "workflow_run": {
            "id":100, "run_attempt":1, "head_branch":"main", "head_sha":"a".repeat(40),
            "event":"push", "conclusion":"success", "path":workflow::PATH,
            "head_repository":{"full_name":"owner/repo"}
        }
    });

    assert!(
        webhook::parse(&serde_json::to_vec(&event).unwrap(), 42)
            .unwrap()
            .is_some()
    );
    assert!(webhook::parse(&serde_json::to_vec(&event).unwrap(), 99).is_err());

    event["workflow_run"]["event"] = json!("pull_request");
    assert!(
        webhook::parse(&serde_json::to_vec(&event).unwrap(), 42)
            .unwrap()
            .is_none()
    );

    event["workflow_run"]["event"] = json!("push");
    event["workflow_run"]["head_repository"]["full_name"] = json!("attacker/repo");
    assert!(
        webhook::parse(&serde_json::to_vec(&event).unwrap(), 42)
            .unwrap()
            .is_none()
    );
}

#[test]
fn generated_workflow_runs_builds_on_github_only() {
    let app = App {
        id: 1,
        provider: "github".into(),
        repository: "owner/repo".into(),
        branch: "main".into(),
        domain: "app.example.com".into(),
        install_command: "npm ci\necho ready".into(),
        build_command: "npm run build".into(),
        output_directory: "dist".into(),
        spa: false,
        workflow_pr: None,
    };
    let yaml = workflow::render(&app);

    assert!(yaml.contains("branches: [\"main\"]"));
    assert!(yaml.contains("          npm ci\n          echo ready\n"));
    assert!(yaml.contains("name: deployer-app"));
    assert!(!yaml.contains("pull_request:"));
    assert!(!yaml.contains("curl"));
}
