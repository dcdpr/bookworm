use crate::{Error, GLOBAL_CLIENT};

/// Search for crates on crates.io.
pub async fn crate_readme(name: &str, version: &str) -> Result<String, Error> {
    let url = format!("https://crates.io/api/v1/crates/{name}/{version}/readme");

    let readme = GLOBAL_CLIENT
        .http_client
        .get(&url)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;

    html2text::from_read(readme.as_bytes(), 80).map_err(Into::into)
}
