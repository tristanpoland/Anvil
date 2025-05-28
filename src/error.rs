use thiserror::Error;
use std::fmt;

pub type AnvilResult<T> = Result<T, AnvilError>;

#[derive(Error, Debug)]
pub enum AnvilError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("Configuration error: {message}")]
    Config { message: String },

    #[error("REPL error: {message}")]
    Repl { message: String },

    #[error("Evaluation error: {message}")]
    Eval { message: String },

    #[error("Command error: {message}")]
    Command { message: String },

    #[error("Parse error: {message}")]
    Parse { message: String },

    #[error("Type error: expected {expected}, found {found}")]
    Type { expected: String, found: String },

    #[error("Shell error: {message}")]
    Shell { message: String },

    #[error("Object error: {message}")]
    Object { message: String },

    #[error("Runtime error: {message}")]
    Runtime { message: String },

    #[error("File not found: {path}")]
    FileNotFound { path: String },

    #[error("Permission denied: {path}")]
    PermissionDenied { path: String },

    #[error("Invalid syntax: {message}")]
    InvalidSyntax { message: String },

    #[error("Compilation error: {message}")]
    Compilation { message: String },

    #[error("External command failed: {command} (exit code: {code})")]
    ExternalCommand { command: String, code: i32 },

    #[error("Interrupted")]
    Interrupted,

    #[error("Unsupported operation: {operation}")]
    Unsupported { operation: String },
}

impl AnvilError {
    pub fn config<S: Into<String>>(message: S) -> Self {
        Self::Config {
            message: message.into(),
        }
    }

    pub fn repl<S: Into<String>>(message: S) -> Self {
        Self::Repl {
            message: message.into(),
        }
    }

    pub fn eval<S: Into<String>>(message: S) -> Self {
        Self::Eval {
            message: message.into(),
        }
    }

    pub fn command<S: Into<String>>(message: S) -> Self {
        Self::Command {
            message: message.into(),
        }
    }

    pub fn parse<S: Into<String>>(message: S) -> Self {
        Self::Parse {
            message: message.into(),
        }
    }

    pub fn type_error<S: Into<String>>(expected: S, found: S) -> Self {
        Self::Type {
            expected: expected.into(),
            found: found.into(),
        }
    }

    pub fn shell<S: Into<String>>(message: S) -> Self {
        Self::Shell {
            message: message.into(),
        }
    }

    pub fn object<S: Into<String>>(message: S) -> Self {
        Self::Object {
            message: message.into(),
        }
    }

    pub fn runtime<S: Into<String>>(message: S) -> Self {
        Self::Runtime {
            message: message.into(),
        }
    }

    pub fn file_not_found<S: Into<String>>(path: S) -> Self {
        Self::FileNotFound {
            path: path.into(),
        }
    }

    pub fn permission_denied<S: Into<String>>(path: S) -> Self {
        Self::PermissionDenied {
            path: path.into(),
        }
    }

    pub fn invalid_syntax<S: Into<String>>(message: S) -> Self {
        Self::InvalidSyntax {
            message: message.into(),
        }
    }

    pub fn compilation<S: Into<String>>(message: S) -> Self {
        Self::Compilation {
            message: message.into(),
        }
    }

    pub fn external_command<S: Into<String>>(command: S, code: i32) -> Self {
        Self::ExternalCommand {
            command: command.into(),
            code,
        }
    }

    pub fn unsupported<S: Into<String>>(operation: S) -> Self {
        Self::Unsupported {
            operation: operation.into(),
        }
    }

    /// Returns true if this error is recoverable in REPL mode
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            AnvilError::Parse { .. }
                | AnvilError::InvalidSyntax { .. }
                | AnvilError::Type { .. }
                | AnvilError::Command { .. }
                | AnvilError::ExternalCommand { .. }
        )
    }

    /// Returns true if this error should cause the shell to exit
    pub fn is_fatal(&self) -> bool {
        matches!(
            self,
            AnvilError::Interrupted | AnvilError::Io(_)
        )
    }
}

// Custom display for better error messages in the shell
