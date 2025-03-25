use std::str::FromStr as _;

use garde::Validate;
use mcp_core::{Content, ResourceContents};
use schemars::JsonSchema;
use serde_json::Value;
use wrm_index::EntryType;

use super::truncate_resources;
use crate::{
    error::Error,
    tool::{format_xml, CRATE_VERSION_RE},
};

/// # crate_search_items
///
/// Search for item definitions within a crate.
#[derive(Debug, Clone, PartialEq, JsonSchema, Validate)]
pub struct SearchCrateItems {
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

    /// # Search query.
    ///
    /// The `query` parameter does partial matching against the full path of the
    /// type.
    ///
    /// In SQL terms, this will execute a similar query to the following:
    ///
    /// ```ignore,sql
    /// SELECT * FROM searchIndex WHERE name LIKE ? AND type IN (?)
    /// ```
    ///
    /// Note that matching is case-insensitive.
    ///
    /// For example, if you search for `Value` in the `serde_json` crate,
    /// assuming the default `types` parameter, then this query will match any
    /// types with `Value` in their path, including methods such as
    /// `Value::is_object`.
    #[garde(length(min = 1))]
    #[schemars(extend("examples" = [
        "Value",
        "map::Values",
        "serde_json::value::Value",
        "value::Value::is_object",
    ]))]
    query: String,

    /// Optional filter to search for specific item types.
    #[garde(skip)]
    #[serde(default = "default_kinds")]
    kinds: Vec<EntryType>,
}

fn default_crate_version() -> Option<String> {
    Some("latest".to_string())
}

fn default_kinds() -> Vec<EntryType> {
    EntryType::all()
}

impl SearchCrateItems {
    pub async fn run(&self) -> Result<Vec<Content>, Error> {
        let definitions = wrm_query::search_crate_type_definitions(
            &self.crate_name,
            self.crate_version.as_deref().unwrap_or("latest"),
            &self.query,
            self.kinds.clone(),
            None,
        )
        .await?;

        if definitions.is_empty() {
            return Ok(vec![Content::text(
                "No crate items found matching the query. Try broadening your search query.",
            )]);
        }

        let content = definitions
            .into_iter()
            .map(|info| {
                Ok(ResourceContents::TextResourceContents {
                    uri: info.docs_resource.clone(),
                    mime_type: None,
                    text: format_xml(&info, Some("Item"))?,
                })
            })
            .map(|result| result.map(Content::resource))
            .collect::<Result<Vec<_>, Error>>()?;

        truncate_resources(content)
    }
}

impl TryFrom<Value> for SearchCrateItems {
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

        let kinds = args
            .get("types")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|v| v.as_str())
            .map(|v| EntryType::from_str(v).map_err(|e| Error::InvalidParameter(e.to_string())))
            .collect::<Result<Vec<_>, _>>()?;

        let this = Self {
            crate_name,
            crate_version,
            query,
            kinds,
        };

        this.validate()?;

        Ok(this)
    }
}
