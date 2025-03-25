#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Request(#[from] reqwest::Error),

    #[error(transparent)]
    CratesIo(#[from] crates_io_api::Error),

    #[error(transparent)]
    Url(#[from] url::ParseError),

    #[error("invalid response from crates.io")]
    InvalidResponse,

    #[error("not found")]
    NotFound,

    #[error("error downloading crate documentation: {0}")]
    Download(#[from] wrm_dl::Error),

    #[error("error indexing crate documentation: {0}")]
    Index(#[from] wrm_index::Error),

    #[error("error fetching crate documentation: {0}")]
    Docs(#[from] wrm_docs::Error),

    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),

    #[error(transparent)]
    Html2Text(#[from] html2text::Error),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("html scraper error: {0}")]
    Scraper(String),

    #[error("version {crate_version} not found for crate {crate_name}")]
    VersionNotFound {
        crate_name: String,
        crate_version: String,
    },

    #[error(transparent)]
    ParseInt(#[from] std::num::ParseIntError),
}
