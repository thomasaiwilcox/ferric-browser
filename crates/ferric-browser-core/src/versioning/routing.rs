//! Typed, redacted requests across reducer/runtime/application boundaries.
//! A request is not authority to execute a native or persistent operation.

use std::fmt;

use crate::{CommandSource, Target, WindowId};

use super::{BranchId, CommitId, OperationId, VersioningCommand};

#[derive(Clone, Eq, PartialEq)]
pub struct VersioningRequest {
    command: VersioningCommand,
    window: WindowId,
    target: Option<Target>,
    source: CommandSource,
}

impl VersioningRequest {
    #[must_use]
    pub(crate) const fn new(
        command: VersioningCommand,
        window: WindowId,
        target: Option<Target>,
        source: CommandSource,
    ) -> Self {
        Self {
            command,
            window,
            target,
            source,
        }
    }

    #[must_use]
    pub const fn command(&self) -> &VersioningCommand {
        &self.command
    }

    #[must_use]
    pub const fn window(&self) -> WindowId {
        self.window
    }

    #[must_use]
    pub const fn target(&self) -> Option<Target> {
        self.target
    }

    #[must_use]
    pub const fn source(&self) -> CommandSource {
        self.source
    }

    #[must_use]
    pub fn into_command(self) -> VersioningCommand {
        self.command
    }
}

impl fmt::Debug for VersioningRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("VersioningRequest([redacted])")
    }
}

#[derive(Clone, Eq, PartialEq)]
pub enum VersioningEngineEffect {
    Request(VersioningRequest),
    Status {
        production_enabled: bool,
        active_operations: u16,
    },
    CommitQueued {
        operation: OperationId,
        commit: CommitId,
    },
    BranchQueued {
        branch: BranchId,
        operation: OperationId,
        commit: CommitId,
    },
    SwitchResolved {
        branch: BranchId,
        window: WindowId,
        child: bool,
    },
    CommitListQueued {
        request_id: u64,
    },
    CommitInfoQueued {
        request_id: u64,
    },
    CommitDeletePreviewQueued {
        request_id: u64,
    },
    PolicyRestorePreviewQueued {
        request_id: u64,
    },
    DiffQueued {
        request_id: u64,
    },
    RestoreQueued {
        request_id: u64,
    },
}

impl fmt::Debug for VersioningEngineEffect {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Request(_) => formatter.write_str("VersioningEngineEffect::Request([redacted])"),
            Self::Status {
                production_enabled,
                active_operations,
            } => formatter
                .debug_struct("VersioningEngineEffect::Status")
                .field("production_enabled", production_enabled)
                .field("active_operations", active_operations)
                .finish(),
            Self::CommitQueued { .. } => {
                formatter.write_str("VersioningEngineEffect::CommitQueued([redacted])")
            }
            Self::BranchQueued { .. } => {
                formatter.write_str("VersioningEngineEffect::BranchQueued([redacted])")
            }
            Self::SwitchResolved { .. } => {
                formatter.write_str("VersioningEngineEffect::SwitchResolved([redacted])")
            }
            Self::CommitListQueued { .. } => {
                formatter.write_str("VersioningEngineEffect::CommitListQueued([redacted])")
            }
            Self::CommitInfoQueued { .. } => {
                formatter.write_str("VersioningEngineEffect::CommitInfoQueued([redacted])")
            }
            Self::CommitDeletePreviewQueued { .. } => {
                formatter.write_str("VersioningEngineEffect::CommitDeletePreviewQueued([redacted])")
            }
            Self::PolicyRestorePreviewQueued { .. } => formatter
                .write_str("VersioningEngineEffect::PolicyRestorePreviewQueued([redacted])"),
            Self::DiffQueued { .. } => {
                formatter.write_str("VersioningEngineEffect::DiffQueued([redacted])")
            }
            Self::RestoreQueued { .. } => {
                formatter.write_str("VersioningEngineEffect::RestoreQueued([redacted])")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versioning_engine_effect_debug_redacts_identifiers() {
        let operation = OperationId::fresh();
        let commit = CommitId::fresh();
        let request_id = 9_283_746_u64;
        let queued = VersioningEngineEffect::CommitQueued { operation, commit };
        let listed = VersioningEngineEffect::CommitListQueued { request_id };
        let queued_debug = format!("{queued:?}");
        let listed_debug = format!("{listed:?}");
        let window = WindowId::from_display("windowid-7").expect("test window");
        let branch = BranchId::fresh();
        let switched_debug = format!(
            "{:?}",
            VersioningEngineEffect::SwitchResolved {
                branch,
                window,
                child: true,
            }
        );

        assert!(!queued_debug.contains(operation.to_string().as_str()));
        assert!(!queued_debug.contains(commit.to_string().as_str()));
        assert!(!listed_debug.contains(request_id.to_string().as_str()));
        assert!(!switched_debug.contains(branch.to_string().as_str()));
        assert!(!switched_debug.contains(window.to_string().as_str()));
    }
}
