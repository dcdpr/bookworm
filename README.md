# Bookworm

A collection of tools to work with [docs.rs](https://docs.rs) documentation.

## Crates

### `wrm_dl`

Download the documentation for a crate from [docs.rs](https://docs.rs) and store
it in a local directory.

It comes both as a library and a binary, you can run it locally using:

```sh
cargo run --bin wrm-dl --features cli -- regex
Documentation downloaded successfully to /tmp/...
```

### `wrm_index`

Index locally stored crate documentation into a SQLite database.

It comes both as a library and a binary, you can run it locally using:

```sh
cargo run --bin wrm-index --features cli -- /tmp/...
Documentation indexed successfully to ./index.sqlite
```

### `wrm_query`

Query the documentation for a crate, using a locally stored version of the crate
documentation, and the index database.

### `wrm_docs`

Fetch details from locally cached docs.rs documentation.

### `wrm_mcp`

A [model context protocol][mcp] server to serve the documentation for a crate.

Run it locally using:

```sh
cargo run --bin wrm-mcp
```

Adding the server to your MCP client depends on the client, but the following
example works for Claude.ai:

```json
{
  "mcpServers": {
    "bookworm": {
      "command": "/path/to/wrm-mcp"
    }
  }
}
```

#### Tools

The following tools are available to an LLM with MCP client capabilities:

##### `crates_search`

Get a list of crates matching the given query.

The returned list contains a list of URIs for each crate to fetch additional
crate information.

##### `crate_search_items`

Get a list of items matching the given query.

Each item type contains the following information:

- Item Path (e.g. `serde_json::value::Value`)
- Item Type (e.g. `enum`)
- Type Signature
- Documentation
- Related Resource URIs

##### `crate_search_src` (**TODO**)

Search all the crate's source code for a given query.

Each item contains the URI of the source code file, the line number, and the
contents of the matched line, including any optional context surrounding the
match.

##### `crate_resource`

Once you find a crate (or know the crate name), you can fetch relevant resources
through the `crate_resource` tool. This tool takes a URI to the resource.

The following URIs are supported:

- `crate://{crate_name}` - list crate versions
- `crate://{crate_name}/{crate_version}` - get metadata
- `crate://{crate_name}/{crate_version}/readme` - get readme content
- `crate://{crate_name}/{crate_version}/items` - list item resources
- `crate://{crate_name}/{crate_version}/src` - list source code resources
- `crate://{crate_name}/{crate_version}/{path}` - get item/src resource

###### `crate://{crate_name}` - list crate versions

Returns a list of crate versions for the given crate name.

Each item in the list contains the following information:

- Version
- Release Date
- MSRV
- Downloads
- Publisher

###### `crate://{crate_name}/{crate_version}` - get crate metadata

The following metadata is returned:

- Name
- Version
- Release Date
- Description
- Homepage
- Repository
- License
- URIs:
  - Readme (e.g. `crate://serde_json/1.0.85/readme`)
  - Crate Item (e.g. `crate://serde_json/1.0.85/items`)
  - Source code (e.g. `crate://serde_json/1.0.85/src`)

###### `crate://{crate_name}/{crate_version}/readme` - get crate readme

Returns the crate README as a string, formatted as Markdown.

###### `crate://{crate_name}/{crate_version}/items` - list crate items

Returns a list of items for the given crate version.

An item is a component of a crate. There are several kinds of items:

- Modules
- Function definitions
- Type definitions
- Struct definitions
- Enumeration definitions
- Trait definitions
- Implementations

Use `search_crate_items` to search for specific items.

###### `crate://{crate_name}/{crate_version}/src` - list crate source code resources

Returns a list of source code resources for the given crate version.

For Example:

```xml
<Resources>
  <Resource uri="crate://serde_json/1.0.85/src/serde_json/lib.rs" />
  <Resource uri="crate://serde_json/1.0.85/src/serde_json/value.rs" />
  <Resource uri="crate://serde_json/1.0.85/src/serde_json/map.rs" />
  <Resource uri="crate://serde_json/1.0.85/src/serde_json/value/mod.rs" />
  ...
</Resources>
```

Use `search_crate_src` to search all the crate's source code.

###### `crate://{crate_name}/{crate_version}/{crate_resource_path}` - get crate resource

Returns the content of the resource at the given path.

##### Url Templating

- `{crate_name}` is the exact name of the crate.
- `{crate_version}` is either a (partial) semver compatible version number, or
  `latest` for the latest published crate version.

[mcp]: https://github.com/jean-airoldi/model-context-protocol
