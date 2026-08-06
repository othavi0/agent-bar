//! Reserved helper exit codes (CLI contract).

/// Request processed and output contract satisfied.
pub const SUCCESS: i32 = 0;
/// Delegated login or generic operation failure.
pub const GENERIC_FAILURE: i32 = 1;
/// CLI grammar or unsupported value.
pub const GRAMMAR: i32 = 2;
/// Settings/input validation.
pub const VALIDATION: i32 = 3;
/// Status/schema/serialization invariant.
pub const SERIALIZATION: i32 = 4;
/// Plugin integration or delegation failure.
pub const PLUGIN: i32 = 5;
/// Unexpected internal failure.
pub const INTERNAL: i32 = 70;

/// Error with a user-facing message and process exit code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliFailure {
    pub message: String,
    pub exit_code: i32,
}

impl CliFailure {
    pub fn grammar(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            exit_code: GRAMMAR,
        }
    }

    pub fn validation(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            exit_code: VALIDATION,
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            exit_code: INTERNAL,
        }
    }

    pub fn plugin(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            exit_code: PLUGIN,
        }
    }
}
