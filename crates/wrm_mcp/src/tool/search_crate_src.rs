use garde::Validate;
use mcp_core::Content;
use schemars::JsonSchema;
use serde_json::Value;

use crate::error::Error;

/// # crate_search_src
///
/// Search inside the source code of a crate.
#[derive(Debug, Clone, PartialEq, JsonSchema, Validate)]
pub struct SearchCrateSrc {
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
    #[serde(default = "default_crate_version")]
    crate_version: Option<String>,

    /// # Search query.
    ///
    /// The `query` parameter does partial matching against the source code of
    /// the crate.
    #[garde(length(min = 1))]
    #[schemars(extend("examples" = [
        "Value",
        "fn main()",
    ]))]
    query: String,

    /// The before and after context lines to include in the search results.
    ///
    /// Can be a maximum of 20 lines. If more context is required, then you can
    /// fetch the source code file using the attached src URI.
    ///
    /// Defaults to 5 lines in each direction.
    #[garde(range(min = 0, max = 20))]
    #[serde(default = "default_context")]
    context: Option<usize>,
}

fn default_crate_version() -> Option<String> {
    Some("latest".to_string())
}

fn default_context() -> usize {
    5
}

impl SearchCrateSrc {
    #[expect(dead_code)]
    pub async fn run(&self) -> Result<Vec<Content>, Error> {
        Ok(vec![])
    }
}

impl TryFrom<Value> for SearchCrateSrc {
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

        let query = args
            .get("query")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .ok_or_else(|| Error::MissingParameter("query"))?;

        let context = args
            .get("context")
            .and_then(Value::as_u64)
            .map(|v| v as usize);

        let this = Self {
            crate_name,
            crate_version,
            query,
            context,
        };

        this.validate()?;

        Ok(this)
    }
}
