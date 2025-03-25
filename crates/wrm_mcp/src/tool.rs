mod crate_readme;
mod crate_resource;
mod crate_versions;
mod search_crate_items;
mod search_crate_src;
mod search_crates;

use std::{fmt, path::PathBuf, str::FromStr, sync::LazyLock};

pub use crate_readme::CrateReadme;
pub use crate_resource::CrateResource;
pub use crate_versions::CrateVersions;
use mcp_core::{Content, Tool};
use quick_xml::se::Serializer;
use regex::Regex;
use schemars::{generate::SchemaSettings, JsonSchema};
pub use search_crate_items::SearchCrateItems;
pub use search_crates::SearchCrates;
use serde::Serialize;
use serde_json::Value;
use url::Url;

use crate::error::Error;

/// Maximum size of the search results response in bytes.
///
/// If the response exceeds this size, it will be truncated to avoid overflowing
/// the client with excessive data. The limit is arbitrary, as there is no limit
/// defined by the protocol, but the `Claude.app` client has shown issues
/// handling larger responses.
const MAX_RESPONSE_SIZE_BYTES: usize = 256 * 1024; // 256KiB limit

static CRATE_VERSION_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^latest|(?:0|[1-9]\d*)(?:\.(?:0|[1-9]\d*))?(?:\.(?:0|[1-9]\d*))?(?:-(?:(?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*)(?:\.(?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*))*))?(?:\+(?:[0-9a-zA-Z-]+(?:\.[0-9a-zA-Z-]+)*))?$").unwrap()
});

pub(crate) trait TryFromSchema {
    type Error: std::error::Error;

    fn try_from_schema<S: JsonSchema>() -> Result<Self, Self::Error>
    where
        Self: Sized;
}

impl TryFromSchema for Tool {
    type Error = Error;

    fn try_from_schema<S: JsonSchema>() -> Result<Self, Self::Error> {
        let settings = SchemaSettings::default().with(|s| {
            s.inline_subschemas = true;

            // somehow the default of `true` causes errors from mcp client(s):
            //
            // ```
            // {
            //   `kinds`: [
            //     `Enum`
            //   ],
            //   `query`: `Value`,
            //   `crate_name`: `serde_json`,
            //   `crate_version`: null
            // }
            //
            // Error executing code: Cannot convert undefined or null to object
            // ```
            s.option_add_null_type = false;
        });
        let generator = settings.into_generator();
        let schema = generator.into_root_schema_for::<S>();

        let name = schema
            .get("title")
            .and_then(Value::as_str)
            .ok_or(Error::MissingParameter("title"))?;

        let description = schema
            .get("description")
            .and_then(Value::as_str)
            .ok_or(Error::MissingParameter("description"))?;

        let input_schema = schema
            .get("properties")
            .cloned()
            .ok_or(Error::MissingParameter("properties"))?;

        Ok(Tool::new(
            name,
            description,
            serde_json::json!({ "type": "object", "properties": input_schema }),
        ))
    }
}

#[derive(Debug, Clone, PartialEq, JsonSchema)]
pub(crate) struct CrateUri {
    pub name: String,
    pub version: Option<String>,
    pub root: Option<PathRoot>,
    pub path: PathBuf,
    pub fragment: Option<String>,
}

impl CrateUri {
    fn versions(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: None,
            root: None,
            path: PathBuf::new(),
            fragment: None,
        }
    }

    fn metadata(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: Some(version.into()),
            root: None,
            path: PathBuf::new(),
            fragment: None,
        }
    }

    fn readme(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: Some(version.into()),
            root: Some(PathRoot::Readme),
            path: PathBuf::new(),
            fragment: None,
        }
    }

    #[expect(dead_code)]
    fn items(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: Some(version.into()),
            root: Some(PathRoot::Items),
            path: PathBuf::new(),
            fragment: None,
        }
    }

    fn src(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: Some(version.into()),
            root: Some(PathRoot::Src),
            path: PathBuf::new(),
            fragment: None,
        }
    }
}

impl From<&CrateUri> for Url {
    fn from(uri: &CrateUri) -> Self {
        let mut url = Url::parse(&format!("crate://{}", uri.name)).expect("valid base URL");

        {
            let mut path = url.path_segments_mut().expect("not cannot-be-a-base");

            if let Some(version) = &uri.version {
                path.push(version);
            }

            if let Some(root) = uri.root {
                path.push(root.as_str());
            }

            for segment in uri.path.iter() {
                path.push(&segment.to_string_lossy());
            }
        }

        if let Some(fragment) = &uri.fragment {
            url.set_fragment(Some(fragment));
        }

        url
    }
}

impl fmt::Display for CrateUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Url::from(self).fmt(f)
    }
}

impl From<CrateUri> for String {
    fn from(uri: CrateUri) -> Self {
        uri.to_string()
    }
}

impl TryFrom<&Url> for CrateUri {
    type Error = Error;

    fn try_from(uri: &Url) -> Result<Self, Self::Error> {
        let mut crate_uri = CrateUri {
            name: String::new(),
            version: None,
            root: None,
            path: PathBuf::new(),
            fragment: None,
        };

        if uri.scheme() != "crate" {
            return Err(Error::InvalidResourceUri(format!(
                "Invalid URI scheme: {:?}, expected `crate`",
                uri.scheme()
            )));
        };

        crate_uri.name = uri
            .host_str()
            .ok_or(Error::InvalidResourceUri(
                "Missing crate name in uri host".to_owned(),
            ))?
            .to_owned();

        let Some(mut segments) = uri.path_segments() else {
            return Ok(crate_uri);
        };

        crate_uri.version = segments.next().map(str::to_owned);
        crate_uri.root = segments.next().map(PathRoot::from_str).transpose()?;
        crate_uri.path = PathBuf::from(segments.collect::<Vec<_>>().join("/"));
        crate_uri.fragment = uri.fragment().map(ToOwned::to_owned);

        Ok(crate_uri)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, JsonSchema)]
pub(crate) enum PathRoot {
    Readme,
    Items,
    Src,
}

impl PathRoot {
    fn as_str(&self) -> &str {
        match self {
            PathRoot::Readme => "readme",
            PathRoot::Items => "items",
            PathRoot::Src => "src",
        }
    }
}

impl FromStr for PathRoot {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "readme" => Ok(PathRoot::Readme),
            "items" => Ok(PathRoot::Items),
            "src" => Ok(PathRoot::Src),
            _ => Err(Error::InvalidResourceUri(format!(
                "Unexpected path root: {s}, must be one of 'readme', 'items', or 'src'"
            ))),
        }
    }
}

fn format_xml<T: Serialize>(value: &T, root_tag: Option<&str>) -> Result<String, Error> {
    let mut buffer = String::new();
    let mut serializer = Serializer::with_root(&mut buffer, root_tag)?;
    serializer.indent(' ', 2);
    value.serialize(serializer)?;

    Ok(buffer)
}

fn truncate_resources(mut content: Vec<Content>) -> Result<Vec<Content>, Error> {
    let total = content.len();
    let mut bytes = content.iter().fold(0, |acc, content| match content {
        Content::Resource(resource) => acc + resource.get_text().len(),
        _ => acc,
    });

    while bytes > MAX_RESPONSE_SIZE_BYTES {
        if content.len() == 1 {
            break;
        }
        let Some(last) = content.pop() else {
            break;
        };

        bytes -= match last {
            Content::Resource(resource) => resource.get_text().len(),
            _ => 0,
        };
    }

    if content.len() != total {
        content.push(Content::text(indoc::formatdoc! {"
            NOTE: Query returned {total} matches, \
            but only showing {len} to stay within size limits.

            Please refine your query for more specific results.",
            len = content.len()
        }));
    }

    Ok(content)
}
