use anyhow::ensure;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DeploymentType {
    #[default]
    Static,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Ecosystem {
    None,
    Javascript,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PackageManager {
    Npm,
    Pnpm,
    Yarn,
    Bun,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Framework {
    PlainHtml,
    Vite,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct BuildConfig {
    pub deployment_type: DeploymentType,
    pub ecosystem: Ecosystem,
    pub package_manager: Option<PackageManager>,
    pub framework: Framework,
}

// Existing apps and API clients retain their original npm build behavior.
impl Default for BuildConfig {
    fn default() -> Self {
        Self {
            deployment_type: DeploymentType::Static,
            ecosystem: Ecosystem::Javascript,
            package_manager: Some(PackageManager::Npm),
            framework: Framework::Vite,
        }
    }
}

impl BuildConfig {
    pub fn validate(&self) -> anyhow::Result<()> {
        ensure!(
            matches!(
                (self.ecosystem, self.package_manager, self.framework),
                (Ecosystem::None, None, Framework::PlainHtml)
                    | (Ecosystem::Javascript, Some(_), Framework::Vite)
            ),
            "choose a compatible ecosystem, package manager, and framework"
        );
        Ok(())
    }
}
