use deployer::domain::{NewApp, validate_domain, validate_repository};

#[test]
fn rejects_paths_and_nginx_injection() {
    for domain in [
        "example.com;",
        "example.com\nroot /etc;",
        "../example.com",
        "*.example.com",
        "Example.com",
        "127.0.0.1",
        "-bad.example.com",
    ] {
        assert!(validate_domain(domain).is_err(), "accepted {domain}");
    }

    for domain in ["example.com", "www.example.com", "xn--bcher-kva.example"] {
        validate_domain(domain).unwrap();
    }

    for repo in [
        "../repo",
        "owner/..",
        "owner/repo/extra",
        "owner/repo?token=x",
    ] {
        assert!(validate_repository(repo).is_err());
    }
}

#[test]
fn validates_static_build_input() {
    let mut input = NewApp {
        repository: "owner/repo".into(),
        branch: "main".into(),
        domain: "app.example.com".into(),
        build_config: Default::default(),
        install_command: "npm ci".into(),
        build_command: "npm run build".into(),
        output_directory: "dist".into(),
        spa: false,
    };

    input.validate().unwrap();
    input.output_directory = "../secrets".into();
    assert!(input.validate().is_err());
}

#[test]
fn validates_explicit_build_choices_and_static_root() {
    use deployer::domain::{BuildConfig, DeploymentType, Ecosystem, Framework, PackageManager};
    let mut input = NewApp {
        repository: "owner/plain".into(),
        branch: "main".into(),
        domain: "plain.example.com".into(),
        build_config: BuildConfig {
            deployment_type: DeploymentType::Static,
            ecosystem: Ecosystem::None,
            package_manager: None,
            framework: Framework::PlainHtml,
        },
        install_command: String::new(),
        build_command: String::new(),
        output_directory: ".".into(),
        spa: false,
    };
    input.validate().unwrap();
    for path in [
        "../private",
        "public/../private",
        "/etc",
        "public/./site",
        "public//site",
    ] {
        input.output_directory = path.into();
        assert!(input.validate().is_err(), "accepted {path}");
    }
    input.output_directory = ".".into();
    input.build_config.package_manager = Some(PackageManager::Npm);
    assert!(input.validate().is_err());
    input.build_config.package_manager = None;
    input.install_command = "npm ci".into();
    assert!(input.validate().is_err());
    input.build_config = BuildConfig::default();
    input.build_command = "npm run build".into();
    input.validate().unwrap();
    input.build_config.package_manager = None;
    assert!(input.validate().is_err());
    input.build_config.package_manager = Some(PackageManager::Bun);
    input.build_config.framework = Framework::PlainHtml;
    assert!(input.validate().is_err());
}

#[test]
fn legacy_api_input_keeps_npm_defaults_and_rejects_unknown_choices() {
    let mut value = serde_json::json!({"repository":"owner/repo","branch":"main","domain":"app.example.com","installCommand":"npm ci","buildCommand":"npm run build","outputDirectory":"dist"});
    let input: NewApp = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(input.build_config, Default::default());
    input.validate().unwrap();
    value["buildConfig"] = serde_json::json!({"deploymentType":"server","ecosystem":"javascript","packageManager":"npm","framework":"vite"});
    assert!(serde_json::from_value::<NewApp>(value).is_err());
}

#[test]
fn plain_static_api_input_needs_no_commands() {
    let input: NewApp = serde_json::from_value(serde_json::json!({
        "repository":"owner/plain", "branch":"main", "domain":"plain.example.com", "outputDirectory":".",
        "buildConfig":{"deploymentType":"static","ecosystem":"none","packageManager":null,"framework":"plainHtml"}
    })).unwrap();
    input.validate().unwrap();
    assert!(input.install_command.is_empty());
    assert!(input.build_command.is_empty());
}

#[test]
fn https_status_requires_a_live_certificate_for_the_current_hostname() {
    use deployer::domain::Https;
    let mut cert = Https::default();
    assert_eq!(cert.status("site.example.com", 100), "disabled");
    cert.enabled = true;
    assert_eq!(cert.status("site.example.com", 100), "pending");
    cert.error = Some("issuance failed".into());
    assert_eq!(cert.status("site.example.com", 100), "failed");
    cert.domain = Some("site.example.com".into());
    cert.expires_at = Some(200);
    assert_eq!(cert.status("site.example.com", 100), "active");
    assert_eq!(cert.status("site.example.com", 200), "expired");
    assert_eq!(cert.status("other.example.com", 100), "failed");
}
