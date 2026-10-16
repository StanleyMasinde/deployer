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
fn vite_workflow_builds_and_uploads_static_output_on_github() {
    let app = App {
        id: 1,
        provider: "github".into(),
        repository: "owner/repo".into(),
        branch: "main".into(),
        domain: "app.example.com".into(),
        build_config: Default::default(),
        install_command: "npm ci\necho ready".into(),
        build_command: "npm run build".into(),
        output_directory: "dist".into(),
        spa: false,
        workflow_pr: None,
        workflow_pr_state: None,
        deletion_requested: false,
        https: Default::default(),
    };
    let yaml = workflow::render(&app);

    assert!(yaml.contains("branches: [\"main\"]"));
    assert!(yaml.contains("          npm ci\n          echo ready\n"));
    assert!(yaml.contains("name: deployer-app"));
    assert!(yaml.contains("uses: actions/setup-node@v6"));
    assert!(yaml.contains("node-version: '24'"));
    assert!(yaml.contains("name: Build Vite app"));
    assert!(yaml.contains("          npm run build\n"));
    assert!(yaml.contains("DEPLOYER_OUTPUT: \"dist\""));
    assert!(yaml.contains("test -f \"$DEPLOYER_OUTPUT/index.html\""));
    assert!(yaml.contains("path: \"dist/\""));
    assert!(yaml.contains("if-no-files-found: error"));
    assert!(yaml.contains("group: deployer-${{ github.ref }}"));
    assert!(yaml.contains("workflow_dispatch:"));
    assert!(!yaml.contains("pull_request:"));
    assert!(!yaml.contains("curl"));
}

fn workflow_app() -> App {
    App {
        id: 1,
        provider: "github".into(),
        repository: "owner/repo".into(),
        branch: "release/site".into(),
        domain: "app.example.com".into(),
        build_config: Default::default(),
        install_command: "install-dependencies".into(),
        build_command: "build-site".into(),
        output_directory: "public/site".into(),
        spa: false,
        workflow_pr: None,
        workflow_pr_state: None,
        deletion_requested: false,
        https: Default::default(),
    }
}

#[test]
fn vite_workflows_install_the_explicit_package_manager() {
    use deployer::domain::PackageManager;
    for (manager, setup) in [
        (PackageManager::Npm, "actions/setup-node@v6"),
        (PackageManager::Pnpm, "pnpm/setup@v3"),
        (PackageManager::Yarn, "corepack prepare yarn@4 --activate"),
        (PackageManager::Bun, "oven-sh/setup-bun@v2"),
    ] {
        let mut app = workflow_app();
        app.build_config.package_manager = Some(manager);
        let yaml = workflow::render(&app);
        assert!(yaml.contains(setup));
        assert!(yaml.contains("          install-dependencies\n"));
        assert!(yaml.contains("          build-site\n"));
        assert!(yaml.contains("path: \"public/site/\""));
        assert!(yaml.contains("branches: [\"release/site\"]"));
        assert!(yaml.contains("name: deployer-app"));
        assert!(!yaml.contains("run: npm ci"));
    }
}

#[test]
fn plain_static_workflow_stages_root_files_without_a_runtime_or_build() {
    use deployer::domain::{BuildConfig, DeploymentType, Ecosystem, Framework};
    let mut app = workflow_app();
    app.build_config = BuildConfig {
        deployment_type: DeploymentType::Static,
        ecosystem: Ecosystem::None,
        package_manager: None,
        framework: Framework::PlainHtml,
    };
    app.install_command.clear();
    app.build_command.clear();
    app.output_directory = ".".into();
    let yaml = workflow::render(&app);
    assert!(yaml.contains("DEPLOYER_OUTPUT: \".\""));
    assert!(yaml.contains("test -f \"$DEPLOYER_OUTPUT/index.html\""));
    assert!(yaml.contains("path: ${{ runner.temp }}/deployer-site/"));
    assert!(yaml.contains("include-hidden-files: false"));
    assert!(yaml.contains("name: deployer-app"));
    assert!(!yaml.contains("setup-node"));
    assert!(!yaml.contains("npm"));
    assert!(!yaml.contains("name: Install dependencies"));
    assert!(!yaml.contains("name: Build Vite app"));
}

#[test]
fn pull_request_events_distinguish_merge_close_and_reopen() {
    let mut event = json!({"action":"closed","installation":{"id":42},"repository":{"full_name":"owner/repo"},"pull_request":{"html_url":"https://github.com/owner/repo/pull/1","state":"closed","merged":true,"updated_at":"2026-10-16T10:00:00Z"}});
    let parse = |value: &serde_json::Value| {
        webhook::parse_pull_request(&serde_json::to_vec(value).unwrap(), 42)
            .unwrap()
            .unwrap()
    };
    assert_eq!(parse(&event).state, "merged");
    event["pull_request"]["merged"] = json!(false);
    assert_eq!(parse(&event).state, "closed");
    event["action"] = json!("reopened");
    event["pull_request"]["state"] = json!("open");
    assert_eq!(parse(&event).state, "open");
    assert!(webhook::parse_pull_request(&serde_json::to_vec(&event).unwrap(), 99).is_err());
}
