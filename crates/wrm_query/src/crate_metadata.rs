use chrono::{DateTime, Utc};
use crates_io_api::CrateResponse;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{Error, GLOBAL_CLIENT};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrateMetadata {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub homepage: Option<Url>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documentation: Option<Url>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository: Option<Url>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub keywords: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub categories: Vec<String>,
    #[serde(flatten)]
    pub version: CrateVersion,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrateVersion {
    pub num: String,
    pub created_at: DateTime<Utc>,
    pub downloads: u64,
    pub license: Option<String>,
    pub published_by: Option<String>,
    pub yanked: bool,
    pub msrv: Option<String>,
}

/// Search for crates on crates.io.
pub async fn crate_metadata(crate_name: &str, crate_version: &str) -> Result<CrateMetadata, Error> {
    let CrateResponse {
        categories,
        crate_data,
        keywords,
        versions,
    } = GLOBAL_CLIENT.crates_client.get_crate(crate_name).await?;

    let version = versions
        .into_iter()
        .find(|v| v.num == crate_version)
        .ok_or(Error::VersionNotFound {
            crate_name: crate_name.to_string(),
            crate_version: crate_version.to_string(),
        })?;

    Ok(CrateMetadata {
        name: crate_data.name,
        description: crate_data.description,
        homepage: crate_data.homepage.map(|v| v.parse()).transpose()?,
        documentation: crate_data.documentation.map(|v| v.parse()).transpose()?,
        repository: crate_data.repository.map(|v| v.parse()).transpose()?,
        keywords: keywords.into_iter().map(|k| k.keyword).collect(),
        categories: categories.into_iter().map(|c| c.category).collect(),
        version: CrateVersion {
            num: version.num,
            created_at: version.created_at,
            downloads: version.downloads,
            license: version.license,
            published_by: version.published_by.map(|u| u.login),
            yanked: version.yanked,
            msrv: version.rust_version,
        },
    })
}
