use crate::domain::{App, Ecosystem, PackageManager};

pub const PATH: &str = ".github/workflows/deployer.yml";
pub const ARTIFACT: &str = "deployer-app";

pub fn render(app: &App) -> String {
    let branch = serde_json::to_string(&app.branch).expect("string");
    let output_directory = serde_json::to_string(&app.output_directory).expect("string");
    if app.build_config.ecosystem == Ecosystem::None {
        return format!(
            include_str!("templates/static.yml"),
            branch = branch,
            output_directory = output_directory,
            artifact = ARTIFACT,
        );
    }
    // Setup comes only from the explicit package-manager selection, never repository detection.
    let setup = match app.build_config.package_manager {
        Some(PackageManager::Npm) => include_str!("templates/setup/npm.yml"),
        Some(PackageManager::Pnpm) => include_str!("templates/setup/pnpm.yml"),
        Some(PackageManager::Yarn) => include_str!("templates/setup/yarn.yml"),
        Some(PackageManager::Bun) => include_str!("templates/setup/bun.yml"),
        None => unreachable!("JavaScript build configuration must be validated before rendering"),
    };
    let command = |value: &str| {
        value
            .lines()
            .map(|s| format!("          {s}\n"))
            .collect::<String>()
    };
    format!(
        include_str!("templates/vite.yml"),
        branch = branch,
        setup = setup,
        install = command(&app.install_command),
        build = command(&app.build_command),
        artifact = ARTIFACT,
        output_directory = output_directory,
        output_path = serde_json::to_string(&format!("{}/", app.output_directory)).expect("string"),
    )
}
