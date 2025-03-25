use std::{
    fs,
    path::{Path, PathBuf},
};

use dom_query::{Document, Selection};
use rusqlite::Connection;
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("missing docs")]
    MissingDocs,

    #[error("not found")]
    NotFound,

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),

    #[error(transparent)]
    Url(#[from] url::ParseError),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Item {
    pub path: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_info: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documentation: Option<String>,
    pub src_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SrcMatch {
    pub path: String,
    pub line: usize,
    pub column: usize,
    pub context: String,
}

pub struct Docs<'a> {
    root: PathBuf,
    conn: &'a Connection,
}

impl<'a> Docs<'a> {
    /// Create a new `Docs` instance.
    pub fn new(root: impl Into<PathBuf>, conn: &'a Connection) -> Result<Self, Error> {
        let root = root.into();
        if !root.is_dir() {
            return Err(Error::MissingDocs);
        }

        rusqlite::vtab::array::load_module(conn)?;

        Ok(Self { root, conn })
    }

    /// Get the item details for a given item path.
    pub fn item(&self, path: &str) -> Result<Item, Error> {
        let (path, fragment) = path.rsplit_once('#').unwrap_or((path, ""));

        let (name, kind) = self.conn.query_row(
            "SELECT name, type FROM searchIndex WHERE path = ?",
            [&path],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )?;

        let html = fs::read_to_string(self.root.join(path))?;
        let document = Document::from(html);

        // For fragmented url, find the element with the given selector,
        // otherwise find the main content.
        let selector = fragment
            .is_empty()
            .then(|| "#main-content".to_owned())
            .unwrap_or(format!("[id='{fragment}']"));

        // Find the element with the given selector, or abort.
        let Some(element) = document.select(&selector).iter().next() else {
            return Err(Error::NotFound);
        };

        // For fragmented url, get the inner HTML of the element as the type
        // info.
        let type_info = (!fragment.is_empty()).then(|| element.inner_html().to_string());

        let documentation = if fragment.is_empty() {
            // For non-fragmented url, get the documentation from the main page
            // section, but we remove some details to reduce the size.
            element
                .select("#trait-implementations-list .impl-items")
                .remove();

            Some(element.inner_html().to_string())
        } else {
            // If we're looking for a specific fragment, find its documentation.
            find_documentation(&element)
        };

        let src_path = element
            .select("a.src")
            .iter()
            .next()
            .and_then(|e| e.attr("href"))
            .as_ref()
            .map(|v| v.split_once('#').unwrap_or((v, "")))
            .and_then(|(src, fragment)| {
                self.root
                    .join(Path::new(path).parent().unwrap_or(Path::new("")))
                    .join(src)
                    .canonicalize()
                    .ok()
                    .map(|p| format!("{}#{fragment}", p.to_string_lossy()))
            })
            .and_then(|p| {
                let root = self
                    .root
                    .canonicalize()
                    .ok()?
                    .to_string_lossy()
                    .into_owned();
                p.strip_prefix(&root).map(ToOwned::to_owned)
            });

        Ok(Item {
            path: name,
            kind,
            type_info,
            documentation,
            src_path,
        })
    }

    // TODO
    pub fn search_src(&self, _query: &str) -> Result<Vec<SrcMatch>, Error> {
        Ok(vec![])
    }
}

/// Recursively search for documentation part of the current element.
fn find_documentation(element: &Selection<'_>) -> Option<String> {
    for element in element.iter() {
        // Check if the current element is a `docblock`.
        if element.has_class("docblock") {
            return Some(element.inner_html().to_string());
        }

        // Check siblings.
        if let Some(sibling) = element.next_sibling().iter().next() {
            if find_documentation(&sibling).is_some() {
                return Some(sibling.inner_html().to_string());
            }
        }

        // Try to go up the tree.
        let parent = element.parent();
        if parent.is_empty() {
            break;
        }

        if let Some(element) = find_documentation(&parent) {
            return Some(element);
        }
    }

    None
}
