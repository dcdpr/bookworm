use std::{
    fmt, fs,
    io::{BufRead as _, BufReader},
    path::{Path, PathBuf},
    str::FromStr,
};

use dom_query::Document;
use rusqlite::Connection;
use schemars::JsonSchema;
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("sqlite error")]
    Sqlite(#[from] rusqlite::Error),

    #[error("IO failure")]
    Io(#[from] std::io::Error),

    #[error("HTML parsing error: {0}")]
    HtmlParsing(String),

    #[error("source path does not exist: {0}")]
    SourceNotFound(PathBuf),

    #[error("source path is not a directory: {0}")]
    SourceNotDirectory(PathBuf),

    #[error("unknown entry type: {0}")]
    UnknownEntryType(String),
}

#[derive(Default)]
pub struct Config {
    /// Path to the documentation directory to index.
    pub source: PathBuf,

    /// File to save the SQLite database to.
    pub output: PathBuf,
}

impl Config {
    pub fn source(mut self, source: impl Into<PathBuf>) -> Self {
        self.source = source.into();
        self
    }

    pub fn output(mut self, output: impl Into<PathBuf>) -> Self {
        self.output = output.into();
        self
    }
}

/// Indexes a local docs.rs documentation directory into a SQLite database.
pub fn index(config: Config) -> Result<(), Error> {
    if !config.source.exists() {
        return Err(Error::SourceNotFound(config.source));
    }

    if !config.source.is_dir() {
        return Err(Error::SourceNotDirectory(config.source));
    }

    if !config.output.exists() {
        if let Some(parent) = config.output.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::File::create(&config.output)?;
    }

    let mut conn = Connection::open(&config.output)?;
    let entries = recursive_walk(&config.source, &config.source, "")?;
    generate_sqlite_index(entries, &mut conn)?;

    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub enum EntryType {
    Constant,
    Enum,
    Function,
    Macro,
    Method,
    Module,
    Struct,
    Trait,
    Type,
    Variant,
    Attribute,
}

impl EntryType {
    pub fn all() -> Vec<EntryType> {
        vec![
            EntryType::Constant,
            EntryType::Enum,
            EntryType::Function,
            EntryType::Macro,
            EntryType::Method,
            EntryType::Module,
            EntryType::Struct,
            EntryType::Trait,
            EntryType::Type,
            EntryType::Variant,
            EntryType::Attribute,
        ]
    }
}

impl fmt::Display for EntryType {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), std::fmt::Error> {
        match self {
            EntryType::Constant => write!(f, "Constant"),
            EntryType::Enum => write!(f, "Enum"),
            EntryType::Function => write!(f, "Function"),
            EntryType::Macro => write!(f, "Macro"),
            EntryType::Method => write!(f, "Method"),
            EntryType::Module => write!(f, "Module"),
            EntryType::Struct => write!(f, "Struct"),
            EntryType::Trait => write!(f, "Trait"),
            EntryType::Type => write!(f, "Type"),
            EntryType::Variant => write!(f, "Variant"),
            EntryType::Attribute => write!(f, "Attribute"),
        }
    }
}

impl FromStr for EntryType {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "constant" => Ok(Self::Constant),
            "enum" => Ok(Self::Enum),
            "fn" | "function" => Ok(Self::Function),
            "macro" => Ok(Self::Macro),
            "method" => Ok(Self::Method),
            "module" => Ok(Self::Module),
            "struct" => Ok(Self::Struct),
            "trait" => Ok(Self::Trait),
            "type" => Ok(Self::Type),
            "variant" => Ok(Self::Variant),
            "attr" | "attribute" => Ok(Self::Attribute),
            _ => Err(Error::UnknownEntryType(s.to_owned())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocsetEntry {
    pub name: String,
    pub ty: EntryType,
    pub path: PathBuf,
}

impl DocsetEntry {
    pub fn new(name: impl Into<String>, ty: EntryType, path: impl Into<PathBuf>) -> Self {
        Self {
            name: name.into(),
            ty,
            path: path.into(),
        }
    }
}

const ROOT_SKIP_DIRS: &[&str] = &["src", "implementors"];

fn recursive_walk(
    root: &Path,
    cur_dir: &Path,
    module_path: &str,
) -> Result<Vec<DocsetEntry>, Error> {
    let mut all_entries = vec![];
    for dir_entry in fs::read_dir(cur_dir)? {
        let dir_entry = dir_entry?;

        let entries = if dir_entry.file_type()?.is_dir() {
            let dir_name = dir_entry.file_name().to_string_lossy().to_string();
            let module_path = if module_path.is_empty() {
                if ROOT_SKIP_DIRS.contains(&dir_name.as_str()) {
                    // Ignore some of the root directories which are of no
                    // interest to us.
                    continue;
                }

                let path = dir_entry.path();
                let path = path.strip_prefix(root).unwrap_or(&path).to_owned();
                if path == Path::new(&dir_name) {
                    // Ignore the current directory.
                    String::new()
                } else {
                    dir_name
                }
            } else {
                format!("{module_path}::{dir_name}")
            };

            recursive_walk(root, &dir_entry.path(), &module_path)?
        } else {
            parse_rustdoc_file(root, &dir_entry.path(), module_path)?
        };

        all_entries.extend(entries);
    }

    Ok(all_entries)
}

fn parse_rustdoc_file(
    root: &Path,
    file_path: &Path,
    module_path: &str,
) -> Result<Vec<DocsetEntry>, Error> {
    let mut entries = vec![];

    if file_path.extension().is_none_or(|ext| ext != "html") {
        return Ok(entries);
    }

    let Some(file_name) = file_path.file_name().map(|p| p.to_string_lossy()) else {
        return Ok(entries);
    };

    let mut file = fs::File::open(file_path)?;
    if check_if_redirection(&mut file)? {
        return Ok(entries);
    }
    // TODO: Unsure if we want this or not.
    //
    // Even if we do, we currently can't skip these aliases, because we are not
    // rewriting paths in the raw HTML we send back to the client. This causes
    // LLMs to interpret `../foo/bar.html` as paths relative to the crate
    // resource URI they sent as the request parameter, which results in an
    // absolute path that could point to an alias, which we skipped.
    //
    // If we were to rewrite HTML by fetching all `a[href]` attributes and
    // making them absolute URIs such as `crate://...`, then we wouldn't have to
    // worry about this, and we could choose to skip aliases (although that also
    // means any code generated by the LLM wouldn't use aliases, which sometimes
    // means you get more verbose import statements).
    //
    // if check_if_inner_type_alias(file_path)? {
    //     return Ok(entries);
    // }

    let parts = file_name.split('.').collect::<Vec<_>>();
    let path = file_path.strip_prefix(root).unwrap_or(file_path).to_owned();

    match parts.len() {
        2 if parts[0] == "index" => {
            let module_path = path
                .parent()
                .map(|p| p.to_string_lossy())
                .unwrap_or_default()
                .replace('/', "::");

            entries.push(DocsetEntry::new(module_path, EntryType::Module, path))
        }

        3 => {
            let name = if module_path.is_empty() {
                parts[1].to_owned()
            } else {
                format!("{module_path}::{}", parts[1])
            };

            let ty = EntryType::from_str(parts[0])?;

            // Parse implementations for structs, enums, and traits.
            if matches!(ty, EntryType::Struct | EntryType::Enum | EntryType::Trait) {
                entries.extend(parse_impl_methods(root, file_path, &name)?);
            }

            // Parse enum variants if this is an enum/type alias.
            if matches!(ty, EntryType::Enum | EntryType::Type) {
                entries.extend(parse_enum_variants(root, file_path, &name)?)
            };

            entries.push(DocsetEntry::new(name, ty, path))
        }

        _ => {}
    };

    Ok(entries)
}

fn parse_enum_variants(root: &Path, path: &Path, parent: &str) -> Result<Vec<DocsetEntry>, Error> {
    let mut entries = vec![];

    let document = Document::from(fs::read_to_string(path)?);

    // We also call this for `Type` types, since these can be type aliases of
    // enums. Because of this, we have to account for type aliases that do not
    // have a `variant` section.
    for variant_element in document.select("section.variant").iter() {
        // Extract the variant ID which has format "variant.VariantName"
        let Some(id) = variant_element.id() else {
            continue;
        };

        let Some(variant) = id.strip_prefix("variant.") else {
            continue;
        };

        let name = format!("{parent}::{variant}");

        // From `enum.Value` to `enum.Value#variant.Array`
        let path = path.strip_prefix(root).unwrap_or(path).to_owned();
        let mut path = path.to_path_buf();
        path.as_mut_os_string().push(format!("#{id}"));

        entries.push(DocsetEntry::new(name, EntryType::Variant, path));
    }

    Ok(entries)
}

fn parse_impl_methods(root: &Path, path: &Path, parent: &str) -> Result<Vec<DocsetEntry>, Error> {
    let mut entries = vec![];

    let document = Document::from(fs::read_to_string(path)?);
    for impl_block in document.select("div.impl-items").iter() {
        for method_element in impl_block.select("details.toggle.method-toggle").iter() {
            // Find the method section which contains the ID and name
            let Some(section) = method_element.select("section.method").iter().next() else {
                continue;
            };

            let Some(section_id) = section.id() else {
                continue;
            };

            let Some(method_name) = section_id.strip_prefix("method.") else {
                continue;
            };

            let name = format!("{parent}::{method_name}");

            // From file path to file path with fragment
            let path = path.strip_prefix(root).unwrap_or(path).to_owned();
            let mut method_path = path.to_path_buf();
            method_path
                .as_mut_os_string()
                .push(format!("#{section_id}"));

            entries.push(DocsetEntry::new(name, EntryType::Method, method_path));
        }
    }

    Ok(entries)
}

// TODO: Figure out in what situations a redirect page is used.
fn check_if_redirection(html_file: &mut fs::File) -> Result<bool, Error> {
    // 512 bytes should get to the end of the head section for most redirection
    // pages in one read, while reading less data than the 8kB default.
    let mut reader = BufReader::with_capacity(512, html_file);

    let mut file_contents = String::new();
    loop {
        let prev_len = file_contents.len();
        let n = reader.read_line(&mut file_contents)?;
        if n == 0 {
            // EOF
            break;
        }
        if file_contents[prev_len..prev_len + n].contains("</head>") {
            // End of the head section, stop here instead of parsing the whole
            // file.
            break;
        }
    }

    Ok(file_contents.contains("<title>Redirection</title>"))
}

/// Crates can alias their own types at different modules within the same crate.
///
/// For example, a common pattern is to have a `error::Error` type that is then
/// also aliased as `Error` in the root module.
///
/// This is useful while developing, but is noise when indexing the
/// documentation for a crate. We only care about the "source" of the type, not
/// any internal aliases.
#[expect(dead_code)]
fn check_if_inner_type_alias(path: &Path) -> Result<bool, Error> {
    // Skip checking module index files.
    if path.file_name().unwrap_or_default() == "index.html" {
        return Ok(false);
    }

    let document = Document::from(fs::read_to_string(path)?);
    for element in document.select("span.sub-heading a.src").iter() {
        // Skip checking if the link is not a source link.
        let Some(href) = element.attr("href") else {
            continue;
        };

        // Skip checking if the link points to a non-Rust path.
        let Some((stripped, _)) = href.rsplit_once(".rs.html") else {
            continue;
        };

        // Skip checking if the link points to a non-src path.
        let Some((root, stripped)) = stripped.split_once("src/") else {
            continue;
        };

        // Special-case for `mod.rs` files.
        let stripped = stripped.strip_suffix("/mod").unwrap_or(stripped);

        // Skip checking if the link points to the root of the crate.
        let Some(parent) = path.parent() else {
            continue;
        };

        // If the link points to a different path, we have an inner type alias.
        if !parent.ends_with(stripped) {
            // If the documentation file related to the source file is a
            // redirect, we should NOT consider it an inner type alias.
            //
            // For example, `regex/struct.Captures.html` points to the source at
            // `src/regex/regex/string.rs.html`, but if you look at the related
            // documentation file at `regex/regex/string/struct.Captures.html`,
            // you will see that it is a redirect back to the file we're
            // currently looking at.
            let ref_path = parent
                .join(root)
                .join(stripped)
                .join(path.file_name().unwrap_or_default())
                .canonicalize()
                .ok();

            if let Some(ref_path) = ref_path {
                let mut file = fs::File::open(ref_path)?;

                if check_if_redirection(&mut file)? {
                    return Ok(false);
                }
            }

            return Ok(true);
        }
    }

    Ok(false)
}

fn generate_sqlite_index(entries: Vec<DocsetEntry>, conn: &mut Connection) -> Result<(), Error> {
    conn.execute("DROP TABLE IF EXISTS searchIndex;", ())?;
    conn.execute(
        "
        CREATE TABLE searchIndex(id INTEGER PRIMARY KEY, name TEXT, type TEXT, path TEXT);
        CREATE UNIQUE INDEX anchor ON searchIndex (name, type, path);
        ",
        (),
    )?;

    let transaction = conn.transaction()?;

    {
        let mut stmt = transaction
            .prepare("INSERT INTO searchIndex (name, type, path) VALUES (?1, ?2, ?3)")?;
        for entry in entries {
            stmt.execute([
                entry.name,
                entry.ty.to_string(),
                entry.path.to_string_lossy().to_string(),
            ])?;
        }
    }

    transaction.commit()?;

    Ok(())
}
