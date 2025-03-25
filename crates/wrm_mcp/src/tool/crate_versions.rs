use std::path::PathBuf;

use garde::Validate;
use mcp_core::Content;
use schemars::JsonSchema;
use serde_json::Value;

use super::{CrateResource, CrateUri};
use crate::error::Error;

/// # crate_versions
///
/// Get a list of most recent versions of a crate.
#[derive(Debug, Clone, PartialEq, JsonSchema, Validate)]
pub struct CrateVersions {
    /// # Crate name.
    ///
    /// The exact name of the crate.
    #[garde(length(min = 1))]
    crate_name: String,
}

impl CrateVersions {
    pub async fn run(&self) -> Result<Vec<Content>, Error> {
        let uri = CrateUri {
            name: self.crate_name.clone(),
            version: None,
            root: None,
            path: PathBuf::new(),
            fragment: None,
        };

        CrateResource::new(&uri).run().await
    }
}

impl TryFrom<Value> for CrateVersions {
    type Error = Error;

    fn try_from(args: Value) -> Result<Self, Self::Error> {
        let crate_name = args
            .get("crate_name")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .ok_or_else(|| Error::MissingParameter("name"))?;

        let this = Self { crate_name };

        this.validate()?;

        Ok(this)
    }
}
