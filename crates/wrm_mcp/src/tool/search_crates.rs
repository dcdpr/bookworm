use garde::Validate;
use mcp_core::{Content, ResourceContents};
use schemars::JsonSchema;
use serde_json::Value;

use super::format_xml;
use crate::error::Error;

/// # crates_search
///
/// Search for crates matching the given query.
#[derive(Debug, Clone, PartialEq, JsonSchema, Validate)]
pub struct SearchCrates {
    /// Search query.
    #[garde(length(min = 1))]
    query: String,
}

impl SearchCrates {
    pub async fn run(&self) -> Result<Vec<Content>, Error> {
        let crates = wrm_query::search_crates(&self.query).await?;

        if crates.is_empty() {
            return Ok(vec![Content::text(
                "No crates found matching the query. Try partial words.",
            )]);
        }

        crates
            .into_iter()
            .map(|info| {
                Ok(ResourceContents::TextResourceContents {
                    uri: format!("crate://{}/{}/", info.name, info.version),
                    mime_type: None,
                    text: format_xml(&info, None)?,
                })
            })
            .map(|result| result.map(Content::resource))
            .collect::<Result<_, _>>()
    }
}

impl TryFrom<Value> for SearchCrates {
    type Error = Error;

    fn try_from(args: Value) -> Result<Self, Self::Error> {
        let query = args
            .get("query")
            .and_then(Value::as_str)
            .ok_or_else(|| Error::MissingParameter("query"))?;

        let this = Self {
            query: query.to_string(),
        };

        this.validate()?;

        Ok(this)
    }
}
