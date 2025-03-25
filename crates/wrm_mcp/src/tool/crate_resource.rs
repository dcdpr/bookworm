use std::str::FromStr as _;

use garde::Validate;
use mcp_core::Content;
use schemars::JsonSchema;
use serde_json::Value;
use url::Url;

use super::truncate_resources;
use crate::{
    error::Error,
    tool::{format_xml, CrateUri, PathRoot},
};

/// # crate_resource
///
/// Get the resource for a crate.
///
/// The following resource URIs are supported:
///
/// - `crate://{crate_name}` - list crate versions
/// - `crate://{crate_name}/{crate_version}` - get metadata
/// - `crate://{crate_name}/{crate_version}/readme` - get readme content
/// - `crate://{crate_name}/{crate_version}/items` - list item resources
/// - `crate://{crate_name}/{crate_version}/src` - list source code resources
/// - `crate://{crate_name}/{crate_version}/{path}` - get item/src resource
#[derive(Debug, Clone, PartialEq, JsonSchema, Validate)]
pub struct CrateResource {
    /// Crate resource URI.
    #[garde(skip)]
    uri: CrateUri,
}

impl CrateResource {
    pub(crate) fn new(uri: impl Into<CrateUri>) -> Self {
        Self { uri: uri.into() }
    }

    pub async fn run(&self) -> Result<Vec<Content>, Error> {
        let Some(version) = &self.uri.version else {
            return versions_handler(&self.uri.name).await;
        };

        let Some(root) = &self.uri.root else {
            return metadata_handler(&self.uri.name, version).await;
        };

        match root {
            PathRoot::Readme => readme_handler(&self.uri.name, version).await,
            PathRoot::Items if self.uri.path.as_os_str().is_empty() => {
                list_items_handler(&self.uri.name, version).await
            }
            PathRoot::Items => item_resource_handler(&self.uri).await,
            PathRoot::Src if self.uri.path.as_os_str().is_empty() => {
                list_src_handler(&self.uri.name, version).await
            }
            PathRoot::Src => src_resource_handler(&self.uri).await,
        }
    }
}

async fn versions_handler(crate_name: &str) -> Result<Vec<Content>, Error> {
    wrm_query::crate_versions(crate_name)
        .await?
        .into_iter()
        .filter_map(|v| {
            (!v.yanked).then_some(
                format_xml(&v, None)
                    .map(|s| Content::embedded_text(CrateUri::metadata(crate_name, v.num), s)),
            )
        })
        .collect::<Result<Vec<_>, _>>()
}

async fn metadata_handler(crate_name: &str, crate_version: &str) -> Result<Vec<Content>, Error> {
    let metadata = wrm_query::crate_metadata(crate_name, crate_version).await?;

    Ok(vec![Content::embedded_text(
        CrateUri::metadata(crate_name, crate_version),
        format_xml(&metadata, None)?,
    )])
}

async fn readme_handler(crate_name: &str, crate_version: &str) -> Result<Vec<Content>, Error> {
    // Crates.io does not support "latest" version, so we'll have to fetch the
    // latest version identifier instead.
    let crate_version = if crate_version == "latest" {
        wrm_query::crate_versions(crate_name)
            .await?
            .into_iter()
            .next()
            .ok_or(Error::VersionNotFound {
                crate_name: crate_name.to_string(),
                version: crate_version.to_string(),
            })?
            .num
    } else {
        crate_version.to_owned()
    };

    wrm_query::crate_readme(crate_name, &crate_version)
        .await
        .map(|readme| {
            vec![Content::embedded_text(
                CrateUri::readme(crate_name, crate_version),
                readme,
            )]
        })
        .map_err(Into::into)
}

async fn list_items_handler(crate_name: &str, crate_version: &str) -> Result<Vec<Content>, Error> {
    let content =
        wrm_query::search_crate_type_definitions(crate_name, crate_version, "", vec![], None)
            .await?
            .into_iter()
            .map(|t| {
                Content::embedded_text(t.docs_resource, t.item.documentation.unwrap_or_default())
            })
            .collect::<Vec<_>>();

    truncate_resources(content)
}

async fn list_src_handler(crate_name: &str, crate_version: &str) -> Result<Vec<Content>, Error> {
    let uris = wrm_query::list_crate_source_resources(crate_name, Some(crate_version)).await?;

    Ok(vec![Content::embedded_text(
        CrateUri::src(crate_name, crate_version),
        format_xml(&uris, None)?,
    )])
}

async fn item_resource_handler(uri: &CrateUri) -> Result<Vec<Content>, Error> {
    wrm_query::get_crate_item_resource(&uri.into())
        .await
        .map_err(Into::into)
        .and_then(|item| {
            Ok(vec![Content::embedded_text(
                uri.to_string(),
                format_xml(&item, None)?,
            )])
        })
}

async fn src_resource_handler(uri: &CrateUri) -> Result<Vec<Content>, Error> {
    wrm_query::get_crate_source_resource(&uri.into())
        .await
        .map(|src| vec![Content::embedded_text(uri.to_string(), src)])
        .map_err(Into::into)
}

impl TryFrom<Value> for CrateResource {
    type Error = Error;

    fn try_from(args: Value) -> Result<Self, Self::Error> {
        let uri = args
            .get("uri")
            .and_then(Value::as_str)
            .ok_or_else(|| Error::MissingParameter("uri"))?;

        let this = Self {
            uri: CrateUri::try_from(&Url::from_str(uri)?)?,
        };

        this.validate()?;

        Ok(this)
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, path::PathBuf};

    use url::Url;

    use super::*;

    struct TestCase {
        uri: &'static str,
        expected: Result<ExpectedUri, Error>,
    }

    #[derive(Debug, Clone, PartialEq, Default)]
    struct ExpectedUri {
        name: &'static str,
        version: Option<&'static str>,
        root: Option<PathRoot>,
        path: &'static str,
        fragment: Option<&'static str>,
    }

    impl From<ExpectedUri> for CrateUri {
        fn from(expected: ExpectedUri) -> Self {
            CrateUri {
                name: expected.name.to_owned(),
                version: expected.version.map(|v| v.to_owned()),
                root: expected.root,
                path: PathBuf::from(expected.path),
                fragment: expected.fragment.map(|f| f.to_owned()),
            }
        }
    }

    #[test]
    fn test_try_from_url() {
        let mut test_cases: HashMap<&'static str, TestCase> = HashMap::new();

        test_cases.insert("complete uri with fragment", TestCase {
            uri: "crate://serde_json/1.0.0/src/value/mod.rs#L30",
            expected: Ok(ExpectedUri {
                name: "serde_json",
                version: Some("1.0.0"),
                root: Some(PathRoot::Src),
                path: "value/mod.rs",
                fragment: Some("L30"),
            }),
        });

        test_cases.insert("complete uri without fragment", TestCase {
            uri: "crate://tokio/1.2.3/items/io/struct.AsyncReadExt.html",
            expected: Ok(ExpectedUri {
                name: "tokio",
                version: Some("1.2.3"),
                root: Some(PathRoot::Items),
                path: "io/struct.AsyncReadExt.html",
                fragment: None,
            }),
        });

        test_cases.insert("readme with empty path", TestCase {
            uri: "crate://regex/latest/readme",
            expected: Ok(ExpectedUri {
                name: "regex",
                version: Some("latest"),
                root: Some(PathRoot::Readme),
                path: "",
                fragment: None,
            }),
        });

        test_cases.insert("items with method reference fragment", TestCase {
            uri: "crate://diesel/2.0.0/items/query_dsl/trait.FilterDsl.html#method.filter",
            expected: Ok(ExpectedUri {
                name: "diesel",
                version: Some("2.0.0"),
                root: Some(PathRoot::Items),
                path: "query_dsl/trait.FilterDsl.html",
                fragment: Some("method.filter"),
            }),
        });

        test_cases.insert("src with complex nested path", TestCase {
            uri: "crate://log/0.4.17/src/log/macros.rs",
            expected: Ok(ExpectedUri {
                name: "log",
                version: Some("0.4.17"),
                root: Some(PathRoot::Src),
                path: "log/macros.rs",
                fragment: None,
            }),
        });

        test_cases.insert("hyphenated crate name", TestCase {
            uri: "crate://proc-macro2/1.0.47/readme",
            expected: Ok(ExpectedUri {
                name: "proc-macro2",
                version: Some("1.0.47"),
                root: Some(PathRoot::Readme),
                path: "",
                fragment: None,
            }),
        });

        test_cases.insert("semver with pre-release tag", TestCase {
            uri: "crate://tokio/1.0.0-alpha.1/items/io/index.html",
            expected: Ok(ExpectedUri {
                name: "tokio",
                version: Some("1.0.0-alpha.1"),
                root: Some(PathRoot::Items),
                path: "io/index.html",
                fragment: None,
            }),
        });

        test_cases.insert("partial version", TestCase {
            uri: "crate://serde/1/items/index.html",
            expected: Ok(ExpectedUri {
                name: "serde",
                version: Some("1"),
                root: Some(PathRoot::Items),
                path: "index.html",
                fragment: None,
            }),
        });

        test_cases.insert("uri with version but no root", TestCase {
            uri: "crate://actix-web/4.0.0",
            expected: Ok(ExpectedUri {
                name: "actix-web",
                version: Some("4.0.0"),
                root: None,
                path: "",
                fragment: None,
            }),
        });

        test_cases.insert("uri with only crate name", TestCase {
            uri: "crate://clap",
            expected: Ok(ExpectedUri {
                name: "clap",
                version: None,
                root: None,
                path: "",
                fragment: None,
            }),
        });

        test_cases.insert("invalid scheme", TestCase {
            uri: "http://crates.io/serde_json",
            expected: Err(Error::InvalidResourceUri(
                "Invalid URI scheme: \"http\", expected `crate`".to_owned(),
            )),
        });

        test_cases.insert("missing host (crate name)", TestCase {
            uri: "crate:///1.0.0/src/value.rs",
            expected: Err(Error::InvalidResourceUri(
                "Missing crate name in uri host".to_owned(),
            )),
        });

        test_cases.insert("invalid root path", TestCase {
            uri: "crate://serde_json/1.0.0/invalid/value.rs",
            expected: Err(Error::InvalidResourceUri(
                "Unexpected path root: invalid, must be one of 'readme', 'items', or 'src'"
                    .to_owned(),
            )),
        });

        test_cases.insert("empty uri", TestCase {
            uri: "crate://",
            expected: Err(Error::InvalidResourceUri(
                "Missing crate name in uri host".to_owned(),
            )),
        });

        test_cases.insert("invalid path root", TestCase {
            uri: "crate://serde_json//",
            expected: Err(Error::InvalidResourceUri(
                "Unexpected path root: , must be one of 'readme', 'items', or 'src'".to_owned(),
            )),
        });

        for (name, test_case) in test_cases {
            let url = Url::parse(test_case.uri).expect("Failed to parse URL");
            let result = CrateUri::try_from(&url);

            match (result, test_case.expected) {
                (Ok(actual), Ok(expected)) => {
                    // Convert ExpectedUri to CrateUri for direct comparison
                    let expected_uri = CrateUri::from(expected);
                    assert_eq!(
                        actual.name, expected_uri.name,
                        "Case '{}': name mismatch",
                        name
                    );
                    assert_eq!(
                        actual.version, expected_uri.version,
                        "Case '{}': version mismatch",
                        name
                    );
                    assert_eq!(
                        actual.root, expected_uri.root,
                        "Case '{}': root mismatch",
                        name
                    );
                    assert_eq!(
                        actual.path, expected_uri.path,
                        "Case '{}': path mismatch",
                        name
                    );
                    assert_eq!(
                        actual.fragment, expected_uri.fragment,
                        "Case '{}': fragment mismatch",
                        name
                    );
                }
                (Err(actual_error), Err(expected_error)) => {
                    // Compare the actual error with the expected error directly
                    assert_eq!(
                        format!("{:?}", actual_error),
                        format!("{:?}", expected_error),
                        "Case '{}': error mismatch",
                        name
                    );
                }
                (Ok(actual), Err(expected_error)) => {
                    panic!(
                        "Case '{}': expected error {:?}, got success: {:?}",
                        name, expected_error, actual
                    );
                }
                (Err(actual_error), Ok(expected)) => {
                    panic!(
                        "Case '{}': expected success with {:?}, got error: {:?}",
                        name, expected, actual_error
                    );
                }
            }
        }
    }
}
