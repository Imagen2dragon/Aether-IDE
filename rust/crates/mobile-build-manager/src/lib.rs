use application_core::{AetherError, Result};
use filesystem_engine::FilesystemEngine;
use process_manager::ProcessManager;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MobileAppTemplate {
    pub name: String,
    pub framework: String, // "react-native-expo", "flutter", "native-android", "native-ios"
    pub bundle_id: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CloudBuildJob {
    pub build_id: String,
    pub platform: String, // "android", "ios", "all"
    pub profile: String,  // "development", "preview", "production"
    pub status: String,   // "queued", "building", "completed", "failed"
    pub artifact_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EasCliStatus {
    pub is_installed: bool,
    pub is_authenticated: bool,
    pub logged_in_as: Option<String>,
}

pub struct MobileBuildManager;

impl MobileBuildManager {
    pub fn scaffold_expo_app<P: AsRef<Path>>(
        target_dir: P,
        app_name: &str,
        bundle_id: &str,
    ) -> Result<PathBuf> {
        let root = target_dir.as_ref().join(app_name);
        std::fs::create_dir_all(&root)?;

        // 1. package.json
        let package_json = serde_json::json!({
            "name": app_name,
            "version": "1.0.0",
            "main": "expo-router/entry",
            "scripts": {
                "start": "expo start",
                "android": "expo start --android",
                "ios": "expo start --ios",
                "web": "expo start --web"
            },
            "dependencies": {
                "expo": "~52.0.0",
                "expo-router": "~4.0.0",
                "expo-status-bar": "~2.0.0",
                "react": "18.3.1",
                "react-native": "0.76.2",
                "react-native-safe-area-context": "4.12.0",
                "react-native-screens": "~4.1.0"
            },
            "devDependencies": {
                "@types/react": "~18.3.12",
                "typescript": "^5.3.3"
            },
            "private": true
        });
        FilesystemEngine::write_file_atomic(
            root.join("package.json"),
            &serde_json::to_string_pretty(&package_json)?,
        )?;

        // 2. app.json
        Self::ensure_app_json(&root, app_name, bundle_id)?;

        // 3. eas.json
        Self::ensure_eas_json(&root)?;

        // 4. app/index.tsx (Expo Router Root)
        let app_dir = root.join("app");
        std::fs::create_dir_all(&app_dir)?;
        let index_code = r#"import React from 'react';
import { StyleSheet, Text, View, TouchableOpacity } from 'react-native';
import { StatusBar } from 'expo-status-bar';

export default function HomeScreen() {
    return (
        <View style={styles.container}>
            <Text style={styles.title}>Welcome to AetherIDE Mobile</Text>
            <Text style={styles.subtitle}>Production-ready React Native + Expo App</Text>
            <TouchableOpacity style={styles.button}>
                <Text style={styles.buttonText}>Get Started</Text>
            </TouchableOpacity>
            <StatusBar style="auto" />
        </View>
    );
}

const styles = StyleSheet.create({
    container: {
        flex: 1,
        backgroundColor: '#0F172A',
        alignItems: 'center',
        justifyContent: 'center',
        padding: 24,
    },
    title: {
        fontSize: 26,
        fontWeight: 'bold',
        color: '#F8FAFC',
        marginBottom: 8,
    },
    subtitle: {
        fontSize: 16,
        color: '#94A3B8',
        marginBottom: 24,
        textAlign: 'center',
    },
    button: {
        backgroundColor: '#3B82F6',
        paddingHorizontal: 24,
        paddingVertical: 12,
        borderRadius: 8,
    },
    buttonText: {
        color: '#FFFFFF',
        fontSize: 16,
        fontWeight: '600',
    },
});
"#;
        FilesystemEngine::write_file_atomic(app_dir.join("index.tsx"), index_code)?;

        Ok(root)
    }

    pub fn ensure_app_json<P: AsRef<Path>>(
        project_dir: P,
        app_name: &str,
        bundle_id: &str,
    ) -> Result<()> {
        let app_json_path = project_dir.as_ref().join("app.json");
        if app_json_path.exists() {
            return Ok(());
        }

        let app_json = serde_json::json!({
            "expo": {
                "name": app_name,
                "slug": app_name.to_lowercase().replace(' ', "-"),
                "version": "1.0.0",
                "orientation": "portrait",
                "icon": "./assets/icon.png",
                "userInterfaceStyle": "automatic",
                "ios": {
                    "supportsTablet": true,
                    "bundleIdentifier": bundle_id
                },
                "android": {
                    "adaptiveIcon": {
                        "foregroundImage": "./assets/adaptive-icon.png",
                        "backgroundColor": "#ffffff"
                    },
                    "package": bundle_id
                },
                "plugins": [
                    "expo-router"
                ]
            }
        });

        FilesystemEngine::write_file_atomic(
            app_json_path,
            &serde_json::to_string_pretty(&app_json)?,
        )
    }

    pub fn ensure_eas_json<P: AsRef<Path>>(project_dir: P) -> Result<()> {
        let eas_json_path = project_dir.as_ref().join("eas.json");
        if eas_json_path.exists() {
            return Ok(());
        }

        let eas_json = serde_json::json!({
            "cli": {
                "version": ">= 12.0.0"
            },
            "build": {
                "development": {
                    "developmentClient": true,
                    "distribution": "internal"
                },
                "preview": {
                    "distribution": "internal",
                    "android": {
                        "buildType": "apk"
                    }
                },
                "production": {
                    "android": {
                        "buildType": "app-bundle"
                    }
                }
            }
        });

        FilesystemEngine::write_file_atomic(
            eas_json_path,
            &serde_json::to_string_pretty(&eas_json)?,
        )
    }

    pub async fn check_eas_status<P: AsRef<Path>>(project_dir: Option<P>) -> Result<EasCliStatus> {
        let cwd = project_dir.map(|p| p.as_ref().to_path_buf());

        // Probe eas whoami
        let whoami_res = ProcessManager::run_command_streaming(
            "eas whoami",
            cwd.clone(),
            Some(std::time::Duration::from_secs(5)),
            None,
            None,
        )
        .await;

        match whoami_res {
            Ok(out) if out.exit_code == 0 => {
                let username = out.stdout.trim().to_string();
                if !username.is_empty() && !username.contains("Not logged in") {
                    return Ok(EasCliStatus {
                        is_installed: true,
                        is_authenticated: true,
                        logged_in_as: Some(username),
                    });
                }
            }
            _ => {}
        }

        // If whoami failed, check if eas CLI or npx eas-cli is installed
        let version_res = ProcessManager::run_command_streaming(
            "eas --version",
            cwd,
            Some(std::time::Duration::from_secs(5)),
            None,
            None,
        )
        .await;

        let is_installed = match version_res {
            Ok(out) => out.exit_code == 0,
            Err(_) => false,
        };

        Ok(EasCliStatus {
            is_installed,
            is_authenticated: false,
            logged_in_as: None,
        })
    }

    pub async fn trigger_eas_build<P: AsRef<Path>>(
        project_dir: P,
        platform: &str,
        profile: &str,
    ) -> Result<CloudBuildJob> {
        let dir = project_dir.as_ref();
        if !dir.exists() {
            return Err(AetherError::MobileBuild(format!(
                "Mobile project directory does not exist: {:?}",
                dir
            )));
        }

        // Ensure app.json and eas.json exist
        Self::ensure_app_json(dir, "MobileApp", "com.aether.mobileapp")?;
        Self::ensure_eas_json(dir)?;

        // Verify EAS CLI installation and authentication
        let eas_status = Self::check_eas_status(Some(dir)).await?;
        if !eas_status.is_installed {
            return Err(AetherError::MobileBuild(
                "EAS CLI is not installed on this machine. Run 'npm install -g eas-cli' or use 'npx eas-cli' to enable cloud builds.".into(),
            ));
        }

        if !eas_status.is_authenticated {
            return Err(AetherError::MobileBuild(
                "EAS CLI is not authenticated. Please run 'npx eas-cli login' or configure EXPO_TOKEN in your environment or Settings before triggering cloud builds.".into(),
            ));
        }

        let cmd = format!(
            "eas build --platform {} --profile {} --non-interactive --json",
            platform, profile
        );

        let out = ProcessManager::run_command_streaming(
            &cmd,
            Some(dir.to_path_buf()),
            Some(std::time::Duration::from_secs(300)),
            None,
            None,
        )
        .await?;

        if out.exit_code != 0 {
            return Err(AetherError::MobileBuild(format!(
                "EAS build failed (exit code {}): {}",
                out.exit_code, out.stderr
            )));
        }

        // Parse real EAS JSON output
        let json_val: serde_json::Value = serde_json::from_str(&out.stdout).map_err(|e| {
            AetherError::MobileBuild(format!(
                "Failed to parse EAS build output JSON: {}. Raw output: {}",
                e, out.stdout
            ))
        })?;

        // EAS returns an array of builds or an object
        let build_node = if json_val.is_array() {
            json_val.get(0).cloned().unwrap_or(json_val.clone())
        } else {
            json_val
        };

        let build_id = build_node["id"]
            .as_str()
            .unwrap_or("unknown-id")
            .to_string();
        let status = build_node["status"]
            .as_str()
            .unwrap_or("in-queue")
            .to_string();
        let artifact_url = build_node
            .pointer("/artifacts/buildUrl")
            .and_then(|v| v.as_str())
            .or_else(|| build_node["dashboardUrl"].as_str())
            .map(|s| s.to_string());

        Ok(CloudBuildJob {
            build_id,
            platform: platform.to_string(),
            profile: profile.to_string(),
            status,
            artifact_url,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expo_scaffold_and_config_generation() {
        let temp_dir = std::env::temp_dir().join(format!(
            "aether_mobile_test_{}",
            uuid::Uuid::new_v4().simple()
        ));
        let app_path =
            MobileBuildManager::scaffold_expo_app(&temp_dir, "MyAetherApp", "com.aether.myapp")
                .expect("scaffolding failed");

        assert!(app_path.join("package.json").exists());
        assert!(app_path.join("app.json").exists());
        assert!(app_path.join("eas.json").exists());
        assert!(app_path.join("app").join("index.tsx").exists());

        // Validate JSON content
        let eas_str = std::fs::read_to_string(app_path.join("eas.json")).unwrap();
        let eas_json: serde_json::Value = serde_json::from_str(&eas_str).unwrap();
        assert!(eas_json["build"]["preview"]["android"]["buildType"] == "apk");

        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[tokio::test]
    async fn test_unauthenticated_eas_build_returns_actionable_error() {
        let temp_dir = std::env::temp_dir().join(format!(
            "aether_mobile_build_{}",
            uuid::Uuid::new_v4().simple()
        ));
        let app_path =
            MobileBuildManager::scaffold_expo_app(&temp_dir, "TestBuildApp", "com.aether.testapp")
                .expect("scaffolding failed");

        // When EAS is not installed or not logged in, trigger_eas_build must return an actionable error
        // and must NOT emit synthetic build IDs or fake artifact URLs.
        let result = MobileBuildManager::trigger_eas_build(&app_path, "android", "preview").await;

        // On developer machines without EAS login, this must fail with clear instructions
        match result {
            Ok(job) => {
                // If the host actually happens to have real EAS authenticated:
                assert!(!job.build_id.starts_with("eas-build-"));
                assert!(!job.artifact_url.as_deref().unwrap_or("").contains("fake"));
            }
            Err(e) => {
                let msg = e.to_string();
                assert!(
                    msg.contains("EAS CLI is not installed")
                        || msg.contains("EAS CLI is not authenticated"),
                    "Expected actionable error message, got: {}",
                    msg
                );
            }
        }

        let _ = std::fs::remove_dir_all(temp_dir);
    }
}
