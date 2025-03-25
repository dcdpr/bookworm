use std::fs;

use html2text::render::TrivialDecorator;
use url::Url;

use crate::{Error, GLOBAL_CLIENT};

/// Get the source resource for a crate.
pub async fn get_crate_source_resource(uri: &Url) -> Result<String, Error> {
    let dl_cfg = wrm_dl::Config::try_from(uri)?
        .root(&GLOBAL_CLIENT.crates_path)
        .client(GLOBAL_CLIENT.http_client.clone());

    let root = wrm_dl::download(dl_cfg).await?;

    // Convert from `/0.1.0/src/lib.rs` to `src/lib.rs`
    //
    // Uri is guaranteed to be valid, since we parsed it in `Config::try_from`.
    let path = &uri.path()[1..]
        .split_once('/')
        .map(|(_, v)| v)
        .unwrap_or(uri.path());

    let source = fs::read_to_string(root.join(path))?;

    // Strip everything except for the actual source code.
    let source = source
        .split_once("<pre class=\"rust\">")
        .map(|(_, v)| v.rsplit_once("</pre>").map(|(v, _)| v).unwrap_or(v))
        .unwrap_or(&source);

    let source = html2text::config::with_decorator(TrivialDecorator::new())
        .string_from_read(source.as_bytes(), usize::MAX)?;

    // The source is plain text, but we have to remove some elements that we
    // don't care about.
    let mut clean_source = String::new();
    for line in source.lines() {
        // Remove any lines not part of the source code.
        if !line.starts_with(|c: char| c.is_ascii_digit()) {
            continue;
        }

        // Remove leading line numbers.
        let line = line.trim_start_matches(|c: char| c.is_ascii_digit());

        clean_source.push_str(line);
        clean_source.push('\n');
    }

    Ok(clean_source)
}
