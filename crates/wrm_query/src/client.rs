use std::{path::PathBuf, sync::LazyLock, time::Duration};

use reqwest::header::{self, USER_AGENT};

pub(crate) static GLOBAL_CLIENT: LazyLock<Client> = LazyLock::new(Client::default);

pub(crate) struct Client {
    pub crates_path: PathBuf,
    pub http_client: reqwest::Client,
    pub crates_client: crates_io_api::AsyncClient,
}

impl Default for Client {
    fn default() -> Self {
        let mut headers = header::HeaderMap::new();
        headers.insert(
            USER_AGENT,
            header::HeaderValue::from_static("bookworm (https://github.com/dcdpr/bookworm)"),
        );

        let http_client = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .expect("Client::default()");

        let crates_client = crates_io_api::AsyncClient::with_http_client(
            http_client.clone(),
            Duration::from_secs(1),
        );

        Self {
            crates_path: std::env::temp_dir().join("bookworm/crates"),
            http_client,
            crates_client,
        }
    }
}
