use application_core::AutonomyMode;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum PermissionTier {
    P0ReadOnly = 0,
    P1ReversibleEdit = 1,
    P2ProcessExecution = 2,
    P3NetworkEgress = 3,
    P4Destructive = 4,
    P5CredentialSensitive = 5,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActionType {
    ReadFile { path: String },
    WriteFile { path: String },
    DeleteFile { path: String },
    ExecuteProcess { binary: String, args: Vec<String> },
    NetworkRequest { url: String },
    AccessCredential { key_name: String },
}

impl ActionType {
    pub fn tier(&self) -> PermissionTier {
        match self {
            ActionType::ReadFile { .. } => PermissionTier::P0ReadOnly,
            ActionType::WriteFile { .. } => PermissionTier::P1ReversibleEdit,
            ActionType::ExecuteProcess { .. } => PermissionTier::P2ProcessExecution,
            ActionType::NetworkRequest { .. } => PermissionTier::P3NetworkEgress,
            ActionType::DeleteFile { .. } => PermissionTier::P4Destructive,
            ActionType::AccessCredential { .. } => PermissionTier::P5CredentialSensitive,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthorizationDecision {
    Allow,
    RequiresUserApproval,
    Deny(Reason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Reason {
    ObserveModeDisallowsModifications,
    HighRiskDestructiveRequiresExplicitPrompt,
    CredentialAccessRequiresBiometricsOrPrompt,
    DisallowedBinary,
}

pub struct PermissionEngine;

impl PermissionEngine {
    pub fn evaluate(mode: AutonomyMode, action: &ActionType) -> AuthorizationDecision {
        let tier = action.tier();

        match mode {
            AutonomyMode::Observe => {
                if tier == PermissionTier::P0ReadOnly {
                    AuthorizationDecision::Allow
                } else {
                    AuthorizationDecision::Deny(Reason::ObserveModeDisallowsModifications)
                }
            }
            AutonomyMode::Assist => {
                if tier == PermissionTier::P0ReadOnly {
                    AuthorizationDecision::Allow
                } else {
                    AuthorizationDecision::RequiresUserApproval
                }
            }
            AutonomyMode::Autopilot => {
                match tier {
                    PermissionTier::P0ReadOnly | PermissionTier::P1ReversibleEdit => {
                        AuthorizationDecision::Allow
                    }
                    PermissionTier::P2ProcessExecution => {
                        // Whitelist check
                        if let ActionType::ExecuteProcess { binary, .. } = action {
                            let safe_binaries = [
                                "cargo", "git", "npm", "node", "npx", "pytest", "python", "tsc",
                            ];
                            if safe_binaries
                                .iter()
                                .any(|b| binary.ends_with(b) || binary == b)
                            {
                                AuthorizationDecision::Allow
                            } else {
                                AuthorizationDecision::RequiresUserApproval
                            }
                        } else {
                            AuthorizationDecision::RequiresUserApproval
                        }
                    }
                    PermissionTier::P3NetworkEgress => AuthorizationDecision::Allow,
                    PermissionTier::P4Destructive => AuthorizationDecision::RequiresUserApproval,
                    PermissionTier::P5CredentialSensitive => {
                        AuthorizationDecision::RequiresUserApproval
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_observe_mode_blocks_writes() {
        let action = ActionType::WriteFile {
            path: "src/main.rs".into(),
        };
        let decision = PermissionEngine::evaluate(AutonomyMode::Observe, &action);
        assert_eq!(
            decision,
            AuthorizationDecision::Deny(Reason::ObserveModeDisallowsModifications)
        );
    }

    #[test]
    fn test_autopilot_allows_safe_compilation() {
        let action = ActionType::ExecuteProcess {
            binary: "cargo".into(),
            args: vec!["test".into()],
        };
        let decision = PermissionEngine::evaluate(AutonomyMode::Autopilot, &action);
        assert_eq!(decision, AuthorizationDecision::Allow);
    }
}
