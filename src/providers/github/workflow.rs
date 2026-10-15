use crate::domain::App;

pub const PATH: &str = ".github/workflows/deployer.yml";
pub const ARTIFACT: &str = "deployer-app";

pub fn render(app: &App) -> String {
    // JSON strings are also YAML scalars. Commands remain literal block content.
    let branch = serde_json::to_string(&app.branch).expect("string");
    let install = app
        .install_command
        .lines()
        .map(|s| format!("          {s}\n"))
        .collect::<String>();
    let build = app
        .build_command
        .lines()
        .map(|s| format!("          {s}\n"))
        .collect::<String>();
    format!(
        r#"# Managed by Deployer. Review and merge this PR to enable deployments.
name: Deployer
on:
  push:
    branches: [{branch}]
  workflow_dispatch:
permissions:
  contents: read
concurrency:
  group: deployer-${{{{ github.ref }}}}
  cancel-in-progress: true
jobs:
  build:
    runs-on: ubuntu-latest
    timeout-minutes: 15
    steps:
      - uses: actions/checkout@v7
      - uses: actions/setup-node@v6
        with:
          node-version: '24'
      - name: Install dependencies
        run: |
{install}      - name: Build static app
        run: |
{build}      - name: Upload static app
        uses: actions/upload-artifact@v7
        with:
          name: {ARTIFACT}
          path: {output}/
          include-hidden-files: false
          if-no-files-found: error
          retention-days: 3
"#,
        output = app.output_directory
    )
}
