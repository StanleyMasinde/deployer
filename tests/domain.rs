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
        install_command: "npm ci".into(),
        build_command: "npm run build".into(),
        output_directory: "dist".into(),
        spa: false,
    };

    input.validate().unwrap();
    input.output_directory = "../secrets".into();
    assert!(input.validate().is_err());
}
