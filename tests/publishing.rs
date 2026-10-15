use deployer::{
    config::Worker,
    domain::App,
    publishing::{self, Activator},
};
use std::{fs, io::Write, path::Path};

fn config(path: &Path) -> Worker {
    Worker {
        apps_root: path.into(),
        nginx_helper: "/unused".into(),
        max_artifact_bytes: 1024,
        max_extracted_bytes: 4096,
        poll_seconds: 1,
    }
}

fn app() -> App {
    App {
        id: 1,
        provider: "github".into(),
        repository: "owner/repo".into(),
        branch: "main".into(),
        domain: "app.example.com".into(),
        build_config: Default::default(),
        install_command: "npm ci".into(),
        build_command: "npm run build".into(),
        output_directory: "dist".into(),
        spa: false,
        workflow_pr: None,
        deletion_requested: false,
    }
}

struct Activate(bool);

impl Activator for Activate {
    async fn activate(&self, _: &App) -> anyhow::Result<()> {
        anyhow::ensure!(self.0, "simulated Nginx failure");
        Ok(())
    }
}

#[tokio::test]
async fn swaps_complete_apps_and_restores_previous_on_activation_failure() {
    let temp = tempfile::tempdir().unwrap();
    let config = config(temp.path());
    let directory = publishing::prepare(&config, 1).unwrap();
    let staging = directory.join("staging-test");
    fs::create_dir(&staging).unwrap();
    fs::write(staging.join("index.html"), "first").unwrap();

    publishing::publish_with(&config, &app(), &staging, &Activate(true))
        .await
        .unwrap();
    assert_eq!(
        fs::read_to_string(directory.join("current/index.html")).unwrap(),
        "first"
    );

    fs::create_dir(&staging).unwrap();
    fs::write(staging.join("index.html"), "bad").unwrap();
    assert!(
        publishing::publish_with(&config, &app(), &staging, &Activate(false))
            .await
            .is_err()
    );
    assert_eq!(
        fs::read_to_string(directory.join("current/index.html")).unwrap(),
        "first"
    );

    fs::create_dir(&staging).unwrap();
    fs::write(staging.join("index.html"), "second").unwrap();
    publishing::publish_with(&config, &app(), &staging, &Activate(true))
        .await
        .unwrap();
    assert_eq!(
        fs::read_to_string(directory.join("current/index.html")).unwrap(),
        "second"
    );
    assert_eq!(fs::read_dir(&directory).unwrap().count(), 2);
}

fn archive(path: &Path, name: &str, contents: &[u8]) {
    let mut writer = zip::ZipWriter::new(fs::File::create(path).unwrap());
    writer
        .start_file(name, zip::write::SimpleFileOptions::default())
        .unwrap();
    writer.write_all(contents).unwrap();
    writer.finish().unwrap();
}

#[test]
fn extracts_with_limits_and_rejects_traversal_or_missing_entrypoint() {
    let temp = tempfile::tempdir().unwrap();
    let zip = temp.path().join("app.zip");

    archive(&zip, "index.html", b"hello");
    publishing::archive::extract(&zip, &temp.path().join("valid"), 100).unwrap();
    assert!(publishing::archive::extract(&zip, &temp.path().join("large"), 2).is_err());

    archive(&zip, "../escape.html", b"no");
    assert!(publishing::archive::extract(&zip, &temp.path().join("traversal"), 100).is_err());
    assert!(!temp.path().join("escape.html").exists());

    archive(&zip, "dist/index.html", b"wrong output directory");
    assert!(publishing::archive::extract(&zip, &temp.path().join("nested"), 100).is_err());
}
