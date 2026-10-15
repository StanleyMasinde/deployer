use deployer::{
    domain::{BuildCompleted, NewApp},
    storage::Store,
};

static MIGRATIONS: toasty::migration::MigrationSet = toasty::embed_migrations!("toasty");

#[tokio::test]
async fn mysql_migrations_deduplication_recovery_and_stale_runs() {
    let url = match std::env::var("DEPLOYER_TEST_DATABASE_URL") {
        Ok(url) => url,
        Err(_) if std::env::var_os("CI").is_some() => {
            panic!("CI must configure DEPLOYER_TEST_DATABASE_URL")
        }
        Err(_) => {
            eprintln!(
                "MySQL test skipped: set DEPLOYER_TEST_DATABASE_URL to a dedicated test database"
            );
            return;
        }
    };
    let store = Store::connect(&url).await.unwrap();

    MIGRATIONS.apply(&store.database()).await.unwrap();
    MIGRATIONS.apply(&store.database()).await.unwrap();

    check_http(store.clone(), &url).await;

    let nonce = deployer::now().to_string() + &rand::random::<u32>().to_string();
    let app = store
        .create_app(
            "github",
            &NewApp {
                repository: format!("test/repo-{nonce}"),
                branch: "main".into(),
                domain: format!("{nonce}.example.com"),
                build_config: Default::default(),
                install_command: "npm ci".into(),
                build_command: "npm run build".into(),
                output_directory: "dist".into(),
                spa: false,
            },
        )
        .await
        .unwrap();
    assert_eq!(
        store.app(app.id).await.unwrap().build_config,
        app.build_config
    );
    // Existing rows have no configuration and must retain the original Node/npm behavior.
    toasty::sql::statement("UPDATE apps SET build_config = NULL WHERE id = ?")
        .bind(app.id)
        .exec(&mut store.database())
        .await
        .unwrap();
    assert_eq!(
        store.app(app.id).await.unwrap().build_config,
        Default::default()
    );

    for manager in [
        deployer::domain::PackageManager::Pnpm,
        deployer::domain::PackageManager::Yarn,
        deployer::domain::PackageManager::Bun,
    ] {
        let mut input = NewApp {
            repository: format!("test/config-{nonce}-{manager:?}"),
            branch: "main".into(),
            domain: format!("config-{nonce}-{manager:?}.example.com").to_lowercase(),
            build_config: Default::default(),
            install_command: "install".into(),
            build_command: "build".into(),
            output_directory: "dist".into(),
            spa: false,
        };
        input.build_config.package_manager = Some(manager);
        input.validate().unwrap();
        let saved = store.create_app("github", &input).await.unwrap();
        assert_eq!(
            store.app(saved.id).await.unwrap().build_config,
            input.build_config
        );
    }
    let plain_input = NewApp {
        repository: format!("test/plain-{nonce}"),
        branch: "main".into(),
        domain: format!("plain-{nonce}.example.com"),
        build_config: deployer::domain::BuildConfig {
            deployment_type: deployer::domain::DeploymentType::Static,
            ecosystem: deployer::domain::Ecosystem::None,
            package_manager: None,
            framework: deployer::domain::Framework::PlainHtml,
        },
        install_command: String::new(),
        build_command: String::new(),
        output_directory: ".".into(),
        spa: false,
    };
    plain_input.validate().unwrap();
    let plain = store.create_app("github", &plain_input).await.unwrap();
    let saved_plain = store.app(plain.id).await.unwrap();
    assert_eq!(saved_plain.build_config, plain_input.build_config);
    assert!(!deployer::providers::github::workflow::render(&saved_plain).contains("setup-node"));
    let build = BuildCompleted {
        repository: app.repository.clone(),
        branch: app.branch.clone(),
        run_id: 100,
        attempt: 1,
        commit_sha: "a".repeat(40),
    };

    let (first, second) = tokio::join!(store.enqueue(&app, &build), store.enqueue(&app, &build));
    first.unwrap();
    second.unwrap();
    let deployments: Vec<_> = store
        .deployments()
        .await
        .unwrap()
        .into_iter()
        .filter(|d| d.app_id == app.id)
        .collect();
    assert_eq!(deployments.len(), 1);

    let jobs = deployer::storage::models::Job::filter_by_deployment_id(deployments[0].id)
        .exec(&mut store.database())
        .await
        .unwrap();
    assert_eq!(jobs.len(), 1);

    let claimed = store.next().await.unwrap().unwrap();
    assert_eq!(
        store
            .deployment(claimed.deployment_id().unwrap())
            .await
            .unwrap()
            .app_id,
        app.id
    );
    assert_eq!(claimed.attempts, 1);
    store.recover().await.unwrap();
    let claimed = store.next().await.unwrap().unwrap();
    assert_eq!(claimed.attempts, 2);
    store.finish(claimed.id, "live", None).await.unwrap();

    let older = deployer::domain::Deployment {
        run_id: 99,
        ..store
            .deployment(claimed.deployment_id().unwrap())
            .await
            .unwrap()
    };
    assert!(store.newer_published(&older).await.unwrap());

    let next_build = BuildCompleted {
        run_id: 101,
        ..build
    };
    store.enqueue(&app, &next_build).await.unwrap();
    let interrupted = store.next().await.unwrap().unwrap();
    toasty::sql::statement("UPDATE jobs SET available_at = ? WHERE id = ?")
        .bind(deployer::now() - 1)
        .bind(interrupted.id)
        .exec(&mut store.database())
        .await
        .unwrap();

    let reclaimed = store.next().await.unwrap().unwrap();
    assert_eq!(reclaimed.id, interrupted.id);
    assert_eq!(reclaimed.attempts, 2);

    store
        .retry(reclaimed.id, "temporary download failure")
        .await
        .unwrap();
    let deployment = store
        .deployment(reclaimed.deployment_id().unwrap())
        .await
        .unwrap();
    assert_eq!(deployment.status, "queued");
    assert_eq!(
        deployment.error.as_deref(),
        Some("temporary download failure")
    );
    let job = deployer::storage::models::Job::filter_by_id(reclaimed.id)
        .first()
        .exec(&mut store.database())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(job.status, "queued");

    toasty::sql::statement("UPDATE jobs SET attempts = 3, available_at = ? WHERE id = ?")
        .bind(deployer::now() - 1)
        .bind(reclaimed.id)
        .exec(&mut store.database())
        .await
        .unwrap();

    assert!(store.next().await.unwrap().is_none());
    let exhausted = store
        .deployments()
        .await
        .unwrap()
        .into_iter()
        .find(|deployment| deployment.id == reclaimed.deployment_id().unwrap())
        .unwrap();
    assert_eq!(exhausted.status, "failed");

    toasty::sql::statement(
        "DELETE j FROM jobs j JOIN deployments d ON d.id = j.deployment_id WHERE d.app_id = ?",
    )
    .bind(app.id)
    .exec(&mut store.database())
    .await
    .unwrap();

    toasty::sql::statement("DELETE FROM deployments WHERE app_id = ?")
        .bind(app.id)
        .exec(&mut store.database())
        .await
        .unwrap();
    toasty::sql::statement("DELETE FROM apps WHERE id = ?")
        .bind(app.id)
        .exec(&mut store.database())
        .await
        .unwrap();
}

async fn check_http(store: Store, database_url: &str) {
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    };
    use std::sync::Arc;
    use tower::ServiceExt;

    let directory = tempfile::tempdir().unwrap();
    let key = directory.path().join("test-app.pem");
    assert!(
        std::process::Command::new("openssl")
            .args(["genrsa", "-out"])
            .arg(&key)
            .arg("2048")
            .status()
            .unwrap()
            .success()
    );

    let state_root = directory.path().join("state");
    let template = include_str!("../config.toml.example")
        .replace("REPLACE_WITH_RANDOM_SECRET", &"a".repeat(64))
        .replace("REPLACE_WITH_INDEPENDENT_RANDOM_SECRET", &"b".repeat(64))
        .replace(
            "mysql://deployer:PASSWORD@127.0.0.1:3306/deployer",
            database_url,
        )
        .replace("/etc/deployer/github-app.pem", key.to_str().unwrap())
        .replace("/var/lib/deployer/state", state_root.to_str().unwrap())
        .replace("https://deployer.example.com", "http://127.0.0.1:8080");
    let mut config: deployer::config::Config = toml::from_str(&template).unwrap();
    config.mail = Some(
        toml::from_str(
            r#"
        from_email = "deployer@example.com"
        [smtp]
        host = "127.0.0.1"
        port = 1025
        security = "none"
    "#,
        )
        .unwrap(),
    );
    config.validate().unwrap();

    let current_provider = deployer::providers::current_provider(&config.providers).unwrap();
    let setup = Arc::new(
        deployer::application::setup::Setup::load(&store, &state_root)
            .await
            .unwrap(),
    );
    let code = std::fs::read_to_string(state_root.join("setup-code")).unwrap();
    let cleanup_store = store.clone();

    let app = deployer::http::router(deployer::http::State {
        store,
        config: Arc::new(config.clone()),
        current_provider: current_provider.clone(),
        setup,
        login_slot: Arc::new(tokio::sync::Semaphore::new(1)),
    });

    let setup_body = serde_json::json!({"data":{"type":"users","attributes":{"code":code,"name":"  Zoë Masinde  ","email":"root@example.com","password":"test-password-long-enough"}}});
    for name in [" ".to_owned(), "A\nB".to_owned(), "x".repeat(129)] {
        let mut invalid_name = setup_body.clone();
        invalid_name["data"]["attributes"]["name"] = serde_json::json!(name);
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/setup")
                    .header("content-type", "application/vnd.api+json")
                    .header("origin", "http://127.0.0.1:8080")
                    .body(Body::from(invalid_name.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert!(cleanup_store.root_account().await.unwrap().is_none());
    }

    let mut invalid_email = setup_body.clone();
    invalid_email["data"]["attributes"]["email"] = serde_json::json!("invalid");
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/setup")
                .header("content-type", "application/vnd.api+json")
                .header("origin", "http://127.0.0.1:8080")
                .body(Body::from(invalid_email.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(cleanup_store.root_account().await.unwrap().is_none());

    let mut invalid_setup = setup_body.clone();
    invalid_setup["data"]["attributes"]["code"] = serde_json::json!("0".repeat(64));
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/setup")
                .header("content-type", "application/vnd.api+json")
                .header("origin", "http://127.0.0.1:8080")
                .body(Body::from(invalid_setup.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(cleanup_store.root_account().await.unwrap().is_none());

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/setup")
                .header("content-type", "application/vnd.api+json")
                .header("origin", "http://127.0.0.1:8080")
                .body(Body::from(setup_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(!state_root.join("setup-code").exists());

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/setup")
                .header("content-type", "application/vnd.api+json")
                .header("origin", "http://127.0.0.1:8080")
                .body(Body::from(setup_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/apps")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        response.headers()["content-type"],
        "application/vnd.api+json"
    );

    let wrong_email = serde_json::json!({"data":{"type":"sessions","attributes":{"email":"other@example.com","password":"test-password-long-enough"}}});
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/session")
                .header("content-type", "application/vnd.api+json")
                .header("origin", "http://127.0.0.1:8080")
                .body(Body::from(wrong_email.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert!(response.headers().get("set-cookie").is_none());

    let response = app.clone().oneshot(Request::builder().method("POST").uri("/api/session")
        .header("content-type", "application/vnd.api+json")
        .header("origin", "http://127.0.0.1:8080")
        .body(Body::from(r#"{"data":{"type":"sessions","attributes":{"email":" ROOT@EXAMPLE.COM ","password":"test-password-long-enough"}}}"#)).unwrap()).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let cookie = response.headers()["set-cookie"]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/apps")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let document: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 65536).await.unwrap()).unwrap();
    assert!(document["data"].is_array());

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/session")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/session")
                .header("cookie", &cookie)
                .header("origin", "http://127.0.0.1:8080")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    assert_eq!(
        cleanup_store.root_account().await.unwrap().unwrap().email,
        "root@example.com"
    );
    assert_eq!(
        cleanup_store.root_account().await.unwrap().unwrap().name,
        "Zoë Masinde"
    );
    check_mail_queue(&cleanup_store, current_provider.as_ref(), &config.worker).await;
    check_app_crud(
        &app,
        &cleanup_store,
        &cookie,
        current_provider.as_ref(),
        &config.worker,
    )
    .await;

    toasty::sql::statement("DELETE FROM users WHERE id = 1")
        .exec(&mut cleanup_store.database())
        .await
        .unwrap();
}

struct TestMailer {
    calls: std::sync::atomic::AtomicUsize,
    failure: Option<bool>,
}

impl deployer::mail::EmailSender for TestMailer {
    async fn deliver(
        &self,
        _id: u64,
        email: &deployer::mail::Email,
    ) -> Result<(), deployer::mail::SendFailure> {
        email.validate().unwrap();
        self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        match self.failure {
            None => Ok(()),
            Some(retryable) => Err(deployer::mail::SendFailure {
                retryable,
                code: "test_failure",
            }),
        }
    }
}

async fn check_mail_queue(
    store: &Store,
    provider: &impl deployer::providers::SourceProvider,
    worker: &deployer::config::Worker,
) {
    use deployer::{domain::JobTask, storage::models::Job};
    use std::sync::atomic::{AtomicUsize, Ordering};

    // Setup and its welcome job commit together.
    let welcome = store.next().await.unwrap().unwrap();
    let JobTask::Email(email) = &welcome.task else {
        panic!("expected welcome email")
    };
    assert_eq!(email.recipient, "root@example.com");
    assert!(email.text.contains("Zoë Masinde"));
    store.recover().await.unwrap();
    let success = TestMailer {
        calls: AtomicUsize::new(0),
        failure: None,
    };
    deployer::worker::process_one(store, provider, worker, &success)
        .await
        .unwrap();
    assert_eq!(success.calls.load(Ordering::SeqCst), 1);
    let completed = Job::filter_by_id(welcome.id)
        .first()
        .exec(&mut store.database())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(completed.status, "completed");
    assert!(completed.payload.is_none());
    assert_eq!(completed.attempts, 2);

    let mailer = deployer::mail::QueuedMailer::new(store.clone());
    let first = mailer
        .send(
            "one@example.com",
            deployer::mail::SystemEmail::Welcome {
                name: "one",
                link: "https://example.com",
            },
        )
        .await
        .unwrap();
    let second = mailer
        .send(
            "two@example.com",
            deployer::mail::SystemEmail::Welcome {
                name: "two",
                link: "https://example.com",
            },
        )
        .await
        .unwrap();
    assert_ne!(first, second); // Multiple email jobs have NULL deployment references.
    let temporary = TestMailer {
        calls: AtomicUsize::new(0),
        failure: Some(true),
    };

    for attempt in 1..=3 {
        deployer::worker::process_one(store, provider, worker, &temporary)
            .await
            .unwrap();
        let row = Job::filter_by_id(first)
            .first()
            .exec(&mut store.database())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.attempts, attempt);

        if attempt < 3 {
            assert_eq!(row.status, "queued");
            assert!(row.payload.is_some());
            assert!(row.available_at > deployer::now());
            Job::filter_by_id(first)
                .update()
                .available_at(0_i64)
                .exec(&mut store.database())
                .await
                .unwrap();
        } else {
            assert_eq!(row.status, "failed");
            assert!(row.payload.is_none());
        }
    }

    let permanent = TestMailer {
        calls: AtomicUsize::new(0),
        failure: Some(false),
    };
    deployer::worker::process_one(store, provider, worker, &permanent)
        .await
        .unwrap();
    let row = Job::filter_by_id(second)
        .first()
        .exec(&mut store.database())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.status, "failed");
    assert_eq!(row.attempts, 1);
    assert!(row.payload.is_none());
    assert_eq!(row.error.as_deref(), Some("test_failure"));

    for id in [welcome.id, first, second] {
        toasty::sql::statement("DELETE FROM jobs WHERE id = ?")
            .bind(id)
            .exec(&mut store.database())
            .await
            .unwrap();
    }
}

async fn check_app_crud(
    router: &axum::Router,
    store: &Store,
    cookie: &str,
    provider: &impl deployer::providers::SourceProvider,
    worker: &deployer::config::Worker,
) {
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    };
    use deployer::domain::JobTask;
    use tower::ServiceExt;
    let nonce = rand::random::<u32>();
    let mut input = NewApp {
        repository: format!("test/crud-{nonce}"),
        branch: "main".into(),
        domain: format!("crud-{nonce}.example.com"),
        build_config: Default::default(),
        install_command: "npm ci".into(),
        build_command: "npm run build".into(),
        output_directory: "dist".into(),
        spa: false,
    };
    let record = store.create_app("github", &input).await.unwrap();
    store
        .set_pr(record.id, "https://github.com/test/site/pull/1")
        .await
        .unwrap();
    let uri = format!("/api/apps/{}", record.id);
    let req = |method: &str, uri: &str, data: Option<serde_json::Value>, origin: bool| {
        let mut builder = Request::builder()
            .method(method)
            .uri(uri)
            .header("cookie", cookie)
            .header("content-type", "application/vnd.api+json");
        if origin {
            builder = builder.header("origin", "http://127.0.0.1:8080");
        }
        builder
            .body(
                data.map(|d| Body::from(d.to_string()))
                    .unwrap_or_else(Body::empty),
            )
            .unwrap()
    };
    let response = router
        .clone()
        .oneshot(req("GET", &uri, None, false))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let json: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 65536).await.unwrap()).unwrap();
    assert_eq!(json["data"]["attributes"]["domain"], input.domain);
    assert_eq!(
        router
            .clone()
            .oneshot(req("GET", "/api/apps/18446744073709551615", None, false))
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );
    let build = BuildCompleted {
        repository: record.repository.clone(),
        branch: "main".into(),
        run_id: u64::from(nonce),
        attempt: 1,
        commit_sha: "a".repeat(40),
    };
    store.enqueue(&record, &build).await.unwrap();
    let deployment_job = store.next().await.unwrap().unwrap();
    assert!(matches!(deployment_job.task, JobTask::Deployment(_)));
    store.finish(deployment_job.id, "live", None).await.unwrap();
    let response = router
        .clone()
        .oneshot(req("GET", &format!("{uri}/deployments"), None, false))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let json: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 65536).await.unwrap()).unwrap();
    assert_eq!(json["data"].as_array().unwrap().len(), 1);
    input.domain = format!("updated-{nonce}.example.com");
    input.spa = true;
    let body = |input: &NewApp| serde_json::json!({"data":{"type":"apps","attributes":{"repository":input.repository,"branch":input.branch,"domain":input.domain,"buildConfig":input.build_config,"installCommand":input.install_command,"buildCommand":input.build_command,"outputDirectory":input.output_directory,"spa":input.spa}}});
    assert_eq!(
        router
            .clone()
            .oneshot(req("PUT", &uri, Some(body(&input)), false))
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        router
            .clone()
            .oneshot(req("PUT", &uri, Some(body(&input)), true))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    let saved = store.find_app(record.id).await.unwrap();
    assert_eq!(saved.domain, input.domain);
    assert!(saved.spa);
    assert!(saved.workflow_pr.is_some());
    input.build_command = "npm run build -- --base=/".into();
    assert_eq!(
        router
            .clone()
            .oneshot(req("PUT", &uri, Some(body(&input)), true))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    assert!(
        store
            .find_app(record.id)
            .await
            .unwrap()
            .workflow_pr
            .is_none()
    );
    for _ in 0..2 {
        let job = store.next().await.unwrap().unwrap();
        assert!(matches!(job.task, JobTask::SyncApp(id) if id == record.id));
        store.finish(job.id, "completed", None).await.unwrap();
    }
    let mut invalid = input.clone();
    invalid.domain = "invalid domain".into();
    assert_eq!(
        router
            .clone()
            .oneshot(req("PUT", &uri, Some(body(&invalid)), true))
            .await
            .unwrap()
            .status(),
        StatusCode::BAD_REQUEST
    );
    let other_input = NewApp {
        repository: format!("test/other-{nonce}"),
        domain: format!("other-{nonce}.example.com"),
        ..input.clone()
    };
    let other = store.create_app("github", &other_input).await.unwrap();
    invalid = input.clone();
    invalid.domain = other.domain;
    assert_eq!(
        router
            .clone()
            .oneshot(req("PUT", &uri, Some(body(&invalid)), true))
            .await
            .unwrap()
            .status(),
        StatusCode::CONFLICT
    );
    assert_eq!(
        router
            .clone()
            .oneshot(req("DELETE", &uri, None, false))
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    for _ in 0..2 {
        assert_eq!(
            router
                .clone()
                .oneshot(req("DELETE", &uri, None, true))
                .await
                .unwrap()
                .status(),
            StatusCode::ACCEPTED
        );
    }
    assert!(store.find_app(record.id).await.unwrap().deletion_requested);
    assert_eq!(
        router
            .clone()
            .oneshot(req("PUT", &uri, Some(body(&input)), true))
            .await
            .unwrap()
            .status(),
        StatusCode::CONFLICT
    );
    let stale = BuildCompleted {
        run_id: build.run_id + 1,
        ..build
    };
    store.enqueue(&record, &stale).await.unwrap();
    assert_eq!(store.app_deployments(record.id).await.unwrap().len(), 1);
    let job = store.next().await.unwrap().unwrap();
    assert!(matches!(job.task, JobTask::RemoveApp(id) if id == record.id));
    store.purge_app(record.id).await.unwrap();
    store.finish(job.id, "completed", None).await.unwrap();
    assert!(store.next().await.unwrap().is_none());
    assert_eq!(
        router
            .clone()
            .oneshot(req("GET", &uri, None, false))
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );
    assert!(store.app_deployments(record.id).await.unwrap().is_empty());
    let recreated = store.create_app("github", &input).await.unwrap();
    let local = tempfile::tempdir().unwrap();
    let mut local_worker = worker.clone();
    local_worker.apps_root = local.path().join("apps");
    store.request_app_deletion(recreated.id).await.unwrap();
    deployer::worker::process_one(
        store,
        provider,
        &local_worker,
        &TestMailer {
            calls: std::sync::atomic::AtomicUsize::new(0),
            failure: None,
        },
    )
    .await
    .unwrap();
    assert!(store.find_app(recreated.id).await.is_err());
    for id in [other.id] {
        toasty::sql::statement("DELETE FROM apps WHERE id = ?")
            .bind(id)
            .exec(&mut store.database())
            .await
            .unwrap();
    }
}
