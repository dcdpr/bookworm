use rusqlite::Connection;
use url::Url;
use wrm_docs::Item;

use crate::{Error, GLOBAL_CLIENT};

/// Get the documentation for a specific crate item.
pub async fn get_crate_item_resource(uri: &Url) -> Result<Item, Error> {
    // Convert from `/0.1.0/items/path/to/item.html` to `path/to/item.html`
    // Uri is guaranteed to be valid, since we parsed it in `Config::try_from`.
    let path = &uri.path()[1..]
        .split_once('/')
        .and_then(|(_, rest)| rest.split_once('/'))
        .map(|(_, v)| v)
        .unwrap_or(uri.path());

    // Download the crate.
    let dl_cfg = wrm_dl::Config::try_from(uri)?
        .root(&GLOBAL_CLIENT.crates_path)
        .client(GLOBAL_CLIENT.http_client.clone());
    let root = wrm_dl::download(dl_cfg).await?;

    // Index the crate.
    let index_file = root.join("index.sqlite");
    let index_cfg = wrm_index::Config::default()
        .source(&root)
        .output(&index_file);
    wrm_index::index(index_cfg)?;

    // Get the item details.
    let conn = Connection::open(index_file)?;
    wrm_docs::Docs::new(root, &conn)?
        .item(path)
        .map_err(Error::from)
}
