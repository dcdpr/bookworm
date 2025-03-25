use std::path::PathBuf;

use mcp_core::handler::{ResourceError, ToolError};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("missing parameter: {0}")]
    MissingParameter(&'static str),

    #[error("validation error: {0}")]
    ValidationError(#[from] garde::error::Report),

    #[error("invalid parameter: {0}")]
    InvalidParameter(String),

    #[error("Crate not found: {0}")]
    CrateNotFound(String),

    #[error("query error: {0}")]
    Query(#[from] wrm_query::Error),

    #[error("XML error: {0}")]
    Xml(#[from] quick_xml::SeError),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Version not found for crate {crate_name}: {version}")]
    VersionNotFound { crate_name: String, version: String },

    #[error("Invalid resource URI: {0}")]
    InvalidResourceUri(String),

    #[error("Resource not found: {0}")]
    ResourceNotFound(String),

    #[error("Documentation not found at path: {0}")]
    DocNotFoundAtPath(PathBuf),

    #[error("URI parse error: {0}")]
    UriParse(#[from] url::ParseError),
}

impl From<Error> for ResourceError {
    fn from(err: Error) -> Self {
        match err {
            Error::CrateNotFound(name) => {
                ResourceError::NotFound(format!("Crate not found: {}", name))
            }
            Error::VersionNotFound {
                crate_name,
                version,
            } => ResourceError::NotFound(format!(
                "Version {} not found for crate {}",
                version, crate_name
            )),
            Error::ResourceNotFound(uri) => {
                ResourceError::NotFound(format!("Resource not found: {}", uri))
            }
            Error::InvalidResourceUri(uri) => {
                ResourceError::NotFound(format!("Invalid resource URI: {}", uri))
            }
            Error::DocNotFoundAtPath(path) => ResourceError::NotFound(format!(
                "Documentation not found at path: {}",
                path.display()
            )),
            err => ResourceError::ExecutionError(err.to_string()),
        }
    }
}

impl From<Error> for ToolError {
    fn from(err: Error) -> Self {
        ToolError::ExecutionError(err.to_string())
    }
}
