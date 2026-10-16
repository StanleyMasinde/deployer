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

    check_session_metadata(&app, &cookie).await;

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
    check_deployment_resources(&app, &cleanup_store, &cookie).await;
    check_workflow_pr_updates(&app, &cleanup_store, config.providers.github().unwrap()).await;
    check_build_lifecycle(&app, &cleanup_store, config.providers.github().unwrap()).await;
    check_live_events(
        &app,
        &cleanup_store,
        &cookie,
        config.providers.github().unwrap(),
    )
    .await;
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
    assert!(!store.find_app(record.id).await.unwrap().https.enabled);
    assert_eq!(
        router
            .clone()
            .oneshot(req("POST", &format!("{uri}/https"), None, false))
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    for _ in 0..2 {
        assert_eq!(
            router
                .clone()
                .oneshot(req("POST", &format!("{uri}/https"), None, true))
                .await
                .unwrap()
                .status(),
            StatusCode::OK
        );
    }
    let https_job = store.next().await.unwrap().unwrap();
    assert!(matches!(https_job.task, JobTask::EnableHttps(id) if id == record.id));
    deployer::storage::models::Job::filter_by_id(https_job.id)
        .update()
        .attempts(3_u32)
        .exec(&mut store.database())
        .await
        .unwrap();
    store.recover().await.unwrap();
    let interrupted = store.find_app(record.id).await.unwrap();
    assert_eq!(
        interrupted
            .https
            .status(&interrupted.domain, deployer::now()),
        "failed"
    );
    store.request_https(record.id).await.unwrap();
    let https_job = store.next().await.unwrap().unwrap();
    assert!(matches!(https_job.task, JobTask::EnableHttps(id) if id == record.id));
    store
        .record_https(record.id, &input.domain, deployer::now() + 86400, None)
        .await
        .unwrap();
    store.finish(https_job.id, "completed", None).await.unwrap();
    let saved = store.find_app(record.id).await.unwrap();
    assert_eq!(saved.https.status(&saved.domain, deployer::now()), "active");
    assert!(store.next().await.unwrap().is_none());
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
    assert_eq!(
        saved.https.status(&saved.domain, deployer::now()),
        "pending"
    );
    store
        .record_https(record.id, &record.domain, deployer::now() + 86400, None)
        .await
        .unwrap();
    assert!(
        store
            .find_app(record.id)
            .await
            .unwrap()
            .https
            .expires_at
            .is_none()
    );
    store
        .https_failed(record.id, &input.domain, "test issuance failed")
        .await
        .unwrap();
    let failed = store.find_app(record.id).await.unwrap();
    assert_eq!(
        failed.https.status(&failed.domain, deployer::now()),
        "failed"
    );
    assert!(saved.spa);
    assert!(saved.workflow_pr.is_some());
    assert_eq!(saved.workflow_pr_state.as_deref(), Some("open"));
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
    assert!(
        store
            .find_app(record.id)
            .await
            .unwrap()
            .workflow_pr_state
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

async fn check_deployment_resources(router: &axum::Router, store: &Store, cookie: &str) {
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    let nonce = rand::random::<u32>();
    let mut apps = Vec::new();
    for suffix in ["history", "other"] {
        let app = store
            .create_app(
                "github",
                &NewApp {
                    repository: format!("test/{suffix}-{nonce}"),
                    branch: "main".into(),
                    domain: format!("{suffix}-{nonce}.example.com"),
                    build_config: Default::default(),
                    install_command: String::new(),
                    build_command: String::new(),
                    output_directory: ".".into(),
                    spa: false,
                },
            )
            .await
            .unwrap();
        apps.push(app);
    }
    let mut ids = Vec::new();
    for index in 0..104_u64 {
        let app_id = apps[usize::from(index == 103)].id;
        let deployment = deployer::storage::models::Deployment::create()
            .event_key(format!("resource-test-{nonce}-{index}"))
            .app_id(app_id)
            .run_id(index + 1)
            .attempt(1_u32)
            .commit_sha("a".repeat(40))
            .status("live")
            .created_at(deployer::now())
            .exec(&mut store.database())
            .await
            .unwrap();
        if index < 103 {
            ids.push(deployment.id);
        }
    }
    ids.reverse();
    let request = |uri: &str, authenticated: bool| {
        let mut builder = Request::builder().uri(uri);
        if authenticated {
            builder = builder.header("cookie", cookie);
        }
        builder.body(Body::empty()).unwrap()
    };
    let uri = format!("/api/apps/{}/deployments", apps[0].id);
    let first = router.clone().oneshot(request(&uri, true)).await.unwrap();
    assert_eq!(first.status(), StatusCode::OK);
    let first: serde_json::Value =
        serde_json::from_slice(&to_bytes(first.into_body(), 1_000_000).await.unwrap()).unwrap();
    let first_ids: Vec<u64> = first["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["id"].as_str().unwrap().parse().unwrap())
        .collect();
    assert_eq!(first_ids, ids[..100]);
    let second = router
        .clone()
        .oneshot(request(&format!("{uri}?before={}", first_ids[99]), true))
        .await
        .unwrap();
    assert_eq!(second.status(), StatusCode::OK);
    let second: serde_json::Value =
        serde_json::from_slice(&to_bytes(second.into_body(), 65536).await.unwrap()).unwrap();
    let second_ids: Vec<u64> = second["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["id"].as_str().unwrap().parse().unwrap())
        .collect();
    assert_eq!(second_ids, ids[100..]);
    let empty = store
        .app_deployments_before(apps[0].id, Some(*ids.last().unwrap()))
        .await
        .unwrap();
    assert!(empty.is_empty());
    let detail = format!("/api/deployments/{}", ids[0]);
    let response = router
        .clone()
        .oneshot(request(&detail, true))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let json: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 65536).await.unwrap()).unwrap();
    assert_eq!(json["data"]["id"], ids[0].to_string());
    assert_eq!(json["data"]["attributes"]["appId"], apps[0].id);
    assert_eq!(json["data"]["attributes"]["commitSha"], "a".repeat(40));
    for path in [&uri, &detail] {
        assert_eq!(
            router
                .clone()
                .oneshot(request(path, false))
                .await
                .unwrap()
                .status(),
            StatusCode::UNAUTHORIZED
        );
    }
    for path in [
        "/api/deployments/18446744073709551615".to_owned(),
        "/api/apps/18446744073709551615/deployments".to_owned(),
    ] {
        assert_eq!(
            router
                .clone()
                .oneshot(request(&path, true))
                .await
                .unwrap()
                .status(),
            StatusCode::NOT_FOUND
        );
    }
    assert_eq!(
        router
            .clone()
            .oneshot(request(&format!("{uri}?before=invalid"), true))
            .await
            .unwrap()
            .status(),
        StatusCode::BAD_REQUEST
    );
    for app in apps {
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
}

async fn check_session_metadata(router: &axum::Router, cookie: &str) {
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    let unauthorized = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/session")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/session")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let document: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 65536).await.unwrap()).unwrap();
    assert_eq!(document["data"]["type"], "sessions");
    assert_eq!(document["data"]["id"], "current");
    let attributes = &document["data"]["attributes"];
    assert_eq!(attributes["authenticated"], true);
    assert_eq!(attributes.as_object().unwrap().len(), 2);
    assert_eq!(
        attributes["user"],
        serde_json::json!({
            "id": 1, "name": "Zoë Masinde", "email": "root@example.com"
        })
    );
}

async fn check_workflow_pr_updates(
    router: &axum::Router,
    store: &Store,
    github: &deployer::config::Github,
) {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use hmac::{Hmac, KeyInit, Mac};
    use sha2::Sha256;
    use tower::ServiceExt;

    let nonce = rand::random::<u32>();
    let app = store
        .create_app(
            "github",
            &NewApp {
                repository: format!("events/repo-{nonce}"),
                branch: "main".into(),
                domain: format!("events-{nonce}.example.com"),
                build_config: Default::default(),
                install_command: String::new(),
                build_command: String::new(),
                output_directory: ".".into(),
                spa: false,
            },
        )
        .await
        .unwrap();
    let url = format!("https://github.com/{}/pull/1", app.repository);
    assert!(app.workflow_pr_state.is_none());
    store.set_pr(app.id, &url).await.unwrap();
    assert_eq!(
        store
            .app(app.id)
            .await
            .unwrap()
            .workflow_pr_state
            .as_deref(),
        Some("open")
    );
    let event = |action: &str, state: &str, merged: bool, timestamp: &str| {
        serde_json::json!({
            "action": action, "installation": { "id": github.installation_id },
            "repository": { "full_name": app.repository.to_uppercase() },
            "pull_request": { "html_url": url, "state": state, "merged": merged, "updated_at": timestamp }
        })
    };
    let request = |event: &serde_json::Value, valid_signature: bool| {
        let bytes = serde_json::to_vec(event).unwrap();
        let mut mac = Hmac::<Sha256>::new_from_slice(github.webhook_secret.as_bytes()).unwrap();
        mac.update(&bytes);
        let signature = if valid_signature {
            format!("sha256={}", hex::encode(mac.finalize().into_bytes()))
        } else {
            format!("sha256={}", "0".repeat(64))
        };
        Request::builder()
            .method("POST")
            .uri("/api/webhooks/github")
            .header("x-github-event", "pull_request")
            .header("x-hub-signature-256", signature)
            .header("content-type", "application/json")
            .body(Body::from(bytes))
            .unwrap()
    };
    let closed = event("closed", "closed", false, "2026-10-16T09:00:00Z");
    assert_eq!(
        router
            .clone()
            .oneshot(request(&closed, false))
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let mut wrong_installation = closed.clone();
    wrong_installation["installation"]["id"] = serde_json::json!(github.installation_id + 1);
    assert_eq!(
        router
            .clone()
            .oneshot(request(&wrong_installation, true))
            .await
            .unwrap()
            .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        store
            .app(app.id)
            .await
            .unwrap()
            .workflow_pr_state
            .as_deref(),
        Some("open")
    );
    let mut unrelated = closed.clone();
    unrelated["pull_request"]["html_url"] =
        serde_json::json!(format!("https://github.com/{}/pull/99", app.repository));
    assert_eq!(
        router
            .clone()
            .oneshot(request(&unrelated, true))
            .await
            .unwrap()
            .status(),
        StatusCode::ACCEPTED
    );
    unrelated = closed.clone();
    unrelated["repository"]["full_name"] = serde_json::json!("unrelated/repository");
    assert_eq!(
        router
            .clone()
            .oneshot(request(&unrelated, true))
            .await
            .unwrap()
            .status(),
        StatusCode::ACCEPTED
    );
    assert_eq!(
        store
            .app(app.id)
            .await
            .unwrap()
            .workflow_pr_state
            .as_deref(),
        Some("open")
    );
    for (payload, expected) in [
        (closed, "closed"),
        (
            event("reopened", "open", false, "2026-10-16T10:00:00Z"),
            "open",
        ),
        (
            event("closed", "closed", false, "2026-10-16T08:00:00Z"),
            "open",
        ),
        (
            event("closed", "closed", true, "2026-10-16T11:00:00Z"),
            "merged",
        ),
        (
            event("closed", "closed", true, "2026-10-16T11:00:00Z"),
            "merged",
        ),
        (
            event("reopened", "open", false, "2026-10-16T12:00:00Z"),
            "merged",
        ),
    ] {
        assert_eq!(
            router
                .clone()
                .oneshot(request(&payload, true))
                .await
                .unwrap()
                .status(),
            StatusCode::ACCEPTED
        );
        assert_eq!(
            store
                .app(app.id)
                .await
                .unwrap()
                .workflow_pr_state
                .as_deref(),
            Some(expected)
        );
    }
    let new_url = format!("https://github.com/{}/pull/2", app.repository);
    store.set_pr(app.id, &new_url).await.unwrap();
    let row = deployer::storage::models::App::filter_by_id(app.id)
        .first()
        .exec(&mut store.database())
        .await
        .unwrap()
        .unwrap();
    assert!(row.workflow_pr_updated_at.is_none());
    let old_pr = event("closed", "closed", true, "2026-10-16T13:00:00Z");
    assert_eq!(
        router
            .clone()
            .oneshot(request(&old_pr, true))
            .await
            .unwrap()
            .status(),
        StatusCode::ACCEPTED
    );
    assert_eq!(
        store
            .app(app.id)
            .await
            .unwrap()
            .workflow_pr_state
            .as_deref(),
        Some("open")
    );
    deployer::storage::models::App::filter_by_id(app.id)
        .update()
        .deleted_at(Some(deployer::now()))
        .exec(&mut store.database())
        .await
        .unwrap();
    let mut deleted = old_pr;
    deleted["pull_request"]["html_url"] = serde_json::json!(new_url);
    assert_eq!(
        router
            .clone()
            .oneshot(request(&deleted, true))
            .await
            .unwrap()
            .status(),
        StatusCode::ACCEPTED
    );
    let row = deployer::storage::models::App::filter_by_id(app.id)
        .first()
        .exec(&mut store.database())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.workflow_pr_state.as_deref(), Some("open"));
    assert!(store.app_deployments(app.id).await.unwrap().is_empty());
    toasty::sql::statement("DELETE FROM apps WHERE id = ?")
        .bind(app.id)
        .exec(&mut store.database())
        .await
        .unwrap();
}

async fn check_build_lifecycle(
    router: &axum::Router,
    store: &Store,
    github: &deployer::config::Github,
) {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use deployer::storage::models::Job;
    use hmac::{Hmac, KeyInit, Mac};
    use sha2::Sha256;
    use tower::ServiceExt;

    let nonce = rand::random::<u32>();
    let app = store
        .create_app(
            "github",
            &NewApp {
                repository: format!("builds/repo-{nonce}"),
                branch: "main".into(),
                domain: format!("builds-{nonce}.example.com"),
                build_config: Default::default(),
                install_command: String::new(),
                build_command: String::new(),
                output_directory: ".".into(),
                spa: false,
            },
        )
        .await
        .unwrap();
    let event = |run: u64, attempt: u32, action: &str, conclusion: Option<&str>| {
        serde_json::json!({
            "action": action, "installation": { "id": github.installation_id },
            "repository": { "full_name": app.repository },
            "workflow_run": { "id": run, "run_attempt": attempt, "head_branch": "main",
                "head_sha": "a".repeat(40), "event": "push", "conclusion": conclusion,
                "path": deployer::providers::github::workflow::PATH,
                "head_repository": { "full_name": app.repository } }
        })
    };
    let request = |payload: &serde_json::Value| {
        let bytes = serde_json::to_vec(payload).unwrap();
        let mut mac = Hmac::<Sha256>::new_from_slice(github.webhook_secret.as_bytes()).unwrap();
        mac.update(&bytes);
        Request::builder()
            .method("POST")
            .uri("/api/webhooks/github")
            .header("x-github-event", "workflow_run")
            .header(
                "x-hub-signature-256",
                format!("sha256={}", hex::encode(mac.finalize().into_bytes())),
            )
            .body(Body::from(bytes))
            .unwrap()
    };
    for (field, value) in [
        ("path", serde_json::json!(".github/workflows/unrelated.yml")),
        ("event", serde_json::json!("pull_request")),
        (
            "head_repository",
            serde_json::json!({ "full_name": "other/fork" }),
        ),
        ("head_branch", serde_json::json!("other-branch")),
    ] {
        let mut ignored = event(10, 1, "in_progress", None);
        ignored["workflow_run"][field] = value;
        assert_eq!(
            router
                .clone()
                .oneshot(request(&ignored))
                .await
                .unwrap()
                .status(),
            StatusCode::ACCEPTED
        );
    }
    let mut invalid = event(10, 1, "in_progress", None);
    invalid["workflow_run"]["head_sha"] = serde_json::json!("invalid");
    assert_eq!(
        router
            .clone()
            .oneshot(request(&invalid))
            .await
            .unwrap()
            .status(),
        StatusCode::BAD_REQUEST
    );
    invalid = event(10, 1, "in_progress", None);
    invalid["installation"]["id"] = serde_json::json!(github.installation_id + 1);
    assert_eq!(
        router
            .clone()
            .oneshot(request(&invalid))
            .await
            .unwrap()
            .status(),
        StatusCode::BAD_REQUEST
    );
    assert!(store.app_deployments(app.id).await.unwrap().is_empty());
    for action in ["requested", "in_progress", "in_progress"] {
        assert_eq!(
            router
                .clone()
                .oneshot(request(&event(11, 1, action, None)))
                .await
                .unwrap()
                .status(),
            StatusCode::ACCEPTED
        );
    }
    let history = store.app_deployments(app.id).await.unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].status, "building");
    assert!(history[0].error.is_none());
    assert!(history[0].finished_at.is_none());
    assert!(
        Job::filter_by_deployment_id(history[0].id)
            .exec(&mut store.database())
            .await
            .unwrap()
            .is_empty()
    );
    for (action, conclusion) in [
        ("completed", Some("failure")),
        ("completed", Some("failure")),
        ("in_progress", None),
    ] {
        assert_eq!(
            router
                .clone()
                .oneshot(request(&event(11, 1, action, conclusion)))
                .await
                .unwrap()
                .status(),
            StatusCode::ACCEPTED
        );
    }
    let history = store.app_deployments(app.id).await.unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].status, "failed");
    assert_eq!(
        history[0].error.as_deref(),
        Some("GitHub workflow concluded: failure")
    );
    assert!(history[0].finished_at.is_some());
    assert!(
        Job::filter_by_deployment_id(history[0].id)
            .exec(&mut store.database())
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        router
            .clone()
            .oneshot(request(&event(12, 1, "requested", None)))
            .await
            .unwrap()
            .status(),
        StatusCode::ACCEPTED
    );
    let success = event(12, 1, "completed", Some("success"));
    let (first, second) = tokio::join!(
        router.clone().oneshot(request(&success)),
        router.clone().oneshot(request(&success))
    );
    assert_eq!(first.unwrap().status(), StatusCode::ACCEPTED);
    assert_eq!(second.unwrap().status(), StatusCode::ACCEPTED);
    let history = store.app_deployments(app.id).await.unwrap();
    assert_eq!(history.len(), 2);
    let queued = history.iter().find(|d| d.run_id == 12).unwrap();
    assert_eq!(queued.status, "queued");
    assert!(queued.finished_at.is_none());
    assert_eq!(
        Job::filter_by_deployment_id(queued.id)
            .exec(&mut store.database())
            .await
            .unwrap()
            .len(),
        1
    );
    for (action, conclusion) in [("in_progress", None), ("completed", Some("failure"))] {
        assert_eq!(
            router
                .clone()
                .oneshot(request(&event(12, 1, action, conclusion)))
                .await
                .unwrap()
                .status(),
            StatusCode::ACCEPTED
        );
    }
    assert_eq!(store.deployment(queued.id).await.unwrap().status, "queued");
    let job = store.next().await.unwrap().unwrap();
    assert!(matches!(job.task, deployer::domain::JobTask::Deployment(id) if id == queued.id));
    store.finish(job.id, "live", None).await.unwrap();
    assert_eq!(
        router
            .clone()
            .oneshot(request(&event(12, 1, "in_progress", None)))
            .await
            .unwrap()
            .status(),
        StatusCode::ACCEPTED
    );
    assert_eq!(store.deployment(queued.id).await.unwrap().status, "live");
    assert_eq!(
        router
            .clone()
            .oneshot(request(&event(12, 2, "in_progress", None)))
            .await
            .unwrap()
            .status(),
        StatusCode::ACCEPTED
    );
    assert_eq!(
        router
            .clone()
            .oneshot(request(&event(12, 2, "completed", Some("cancelled"))))
            .await
            .unwrap()
            .status(),
        StatusCode::ACCEPTED
    );
    let history = store.app_deployments(app.id).await.unwrap();
    assert_eq!(history.len(), 3);
    let retry = history.iter().find(|d| d.attempt == 2).unwrap();
    assert_eq!(retry.status, "failed");
    assert_ne!(retry.id, queued.id);
    assert!(
        Job::filter_by_deployment_id(retry.id)
            .exec(&mut store.database())
            .await
            .unwrap()
            .is_empty()
    );
    deployer::storage::models::App::filter_by_id(app.id)
        .update()
        .deleted_at(Some(deployer::now()))
        .exec(&mut store.database())
        .await
        .unwrap();
    assert_eq!(
        router
            .clone()
            .oneshot(request(&event(13, 1, "in_progress", None)))
            .await
            .unwrap()
            .status(),
        StatusCode::ACCEPTED
    );
    assert_eq!(store.app_deployments(app.id).await.unwrap().len(), 3);
    toasty::sql::statement(
        "DELETE FROM jobs WHERE deployment_id IN (SELECT id FROM deployments WHERE app_id = ?)",
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

async fn check_live_events(
    router: &axum::Router,
    store: &Store,
    cookie: &str,
    github: &deployer::config::Github,
) {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use hmac::{Hmac, KeyInit, Mac};
    use sha2::Sha256;
    use std::time::Duration;
    use tokio_stream::StreamExt;
    use tower::ServiceExt;

    for cookie in [
        None,
        Some("deployer_session=invalid".to_owned()),
        Some(format!(
            "deployer_session={}",
            deployer::application::sessions::issue(&"a".repeat(64), deployer::now() - 1)
        )),
    ] {
        let mut request = Request::builder().uri("/api/events");
        if let Some(cookie) = cookie {
            request = request.header("cookie", cookie);
        }
        assert_eq!(
            router
                .clone()
                .oneshot(request.body(Body::empty()).unwrap())
                .await
                .unwrap()
                .status(),
            StatusCode::UNAUTHORIZED
        );
    }
    let nonce = rand::random::<u32>();
    let app = store
        .create_app(
            "github",
            &NewApp {
                repository: format!("live/repo-{nonce}"),
                branch: "main".into(),
                domain: format!("live-{nonce}.example.com"),
                build_config: Default::default(),
                install_command: String::new(),
                build_command: String::new(),
                output_directory: ".".into(),
                spa: false,
            },
        )
        .await
        .unwrap();
    let url = format!("https://github.com/{}/pull/1", app.repository);
    store.set_pr(app.id, &url).await.unwrap();
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/events")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "text/event-stream");
    assert_eq!(response.headers()["x-accel-buffering"], "no");
    let mut stream = response.into_body().into_data_stream();
    let decode = |bytes: &[u8]| {
        let frame = std::str::from_utf8(bytes).unwrap();
        assert!(frame.contains("event: snapshot"));
        serde_json::from_str::<serde_json::Value>(
            frame
                .lines()
                .find_map(|line| line.strip_prefix("data: "))
                .unwrap(),
        )
        .unwrap()
    };
    let frame = tokio::time::timeout(Duration::from_secs(2), stream.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let snapshot = decode(&frame);
    assert!(snapshot["deployments"].is_array());
    let row = snapshot["apps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == app.id)
        .unwrap();
    assert_eq!(row["workflowPrState"], "open");
    assert!(snapshot.get("user").is_none());
    let bytes = serde_json::to_vec(&serde_json::json!({
        "action": "closed", "installation": { "id": github.installation_id },
        "repository": { "full_name": app.repository },
        "pull_request": { "html_url": url, "state": "closed", "merged": true, "updated_at": "2026-10-16T14:00:00Z" }
    })).unwrap();
    let mut mac = Hmac::<Sha256>::new_from_slice(github.webhook_secret.as_bytes()).unwrap();
    mac.update(&bytes);
    assert_eq!(
        router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/webhooks/github")
                    .header("x-github-event", "pull_request")
                    .header(
                        "x-hub-signature-256",
                        format!("sha256={}", hex::encode(mac.finalize().into_bytes()))
                    )
                    .body(Body::from(bytes))
                    .unwrap()
            )
            .await
            .unwrap()
            .status(),
        StatusCode::ACCEPTED
    );
    let frame = tokio::time::timeout(Duration::from_secs(1), stream.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let snapshot = decode(&frame);
    let row = snapshot["apps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == app.id)
        .unwrap();
    assert_eq!(row["workflowPrState"], "merged");
    store.notify_changes();
    assert!(
        tokio::time::timeout(Duration::from_millis(150), stream.next())
            .await
            .is_err()
    );
    // A separate worker process cannot share Notify; its database writes must still reach the stream.
    toasty::sql::statement("UPDATE apps SET https_error = 'worker update' WHERE id = ?")
        .bind(app.id)
        .exec(&mut store.database())
        .await
        .unwrap();
    let frame = tokio::time::timeout(Duration::from_secs(3), stream.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let snapshot = decode(&frame);
    let row = snapshot["apps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == app.id)
        .unwrap();
    assert_eq!(row["https"]["error"], "worker update");
    drop(stream);
    toasty::sql::statement("DELETE FROM apps WHERE id = ?")
        .bind(app.id)
        .exec(&mut store.database())
        .await
        .unwrap();
}
