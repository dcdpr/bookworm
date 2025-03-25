use std::path::PathBuf;

use garde::Validate;
use mcp_core::Content;
use schemars::JsonSchema;
use serde_json::Value;

use super::{CrateResource, CrateUri, PathRoot};
use crate::{error::Error, tool::CRATE_VERSION_RE};

/// # crate_readme
///
/// Get the README for a specific crate version.
#[derive(Debug, Clone, PartialEq, JsonSchema, Validate)]
pub struct CrateReadme {
    /// # Crate name.
    ///
    /// The exact name of the crate.
    #[garde(length(min = 1))]
    crate_name: String,

    /// # Crate version.
    ///
    /// The version of the crate. Either a semantic version or `latest` for the
    /// latest published crate version.
    #[garde(length(min = 1))]
    #[schemars(regex(pattern = *CRATE_VERSION_RE))]
    #[serde(default = "default_crate_version")]
    crate_version: Option<String>,
}

fn default_crate_version() -> Option<String> {
    Some("latest".to_string())
}

impl CrateReadme {
    pub async fn run(&self) -> Result<Vec<Content>, Error> {
        let uri = CrateUri {
            name: self.crate_name.clone(),
            version: self.crate_version.clone(),
            root: Some(PathRoot::Readme),
            path: PathBuf::new(),
            fragment: None,
        };

        CrateResource::new(&uri).run().await
    }
}

impl TryFrom<Value> for CrateReadme {
    type Error = Error;

    fn try_from(args: Value) -> Result<Self, Self::Error> {
        let crate_name = args
            .get("crate_name")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .ok_or_else(|| Error::MissingParameter("name"))?;

        let crate_version = args
            .get("crate_version")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .to_owned();

        let this = Self {
            crate_name,
            crate_version,
        };

        this.validate()?;

        Ok(this)
    }
}
