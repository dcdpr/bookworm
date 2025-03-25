use std::{future::Future, pin::Pin};

use indoc::{formatdoc, indoc};
use mcp_core::{
    content::Content,
    handler::{PromptError, ResourceError, ToolError},
    prompt::Prompt,
    protocol::ServerCapabilities,
    resource::Resource,
    tool::Tool,
};
use mcp_server::router::CapabilitiesBuilder;
use schemars::JsonSchema;
use serde_json::Value;

use crate::tool::{self, TryFromSchema as _};

#[derive(Debug, Clone, Copy)]
pub struct Server;

impl mcp_server::Router for Server {
    fn name(&self) -> String {
        "bookworm".to_string()
    }

    fn instructions(&self) -> String {
        indoc! {r#"
            The "bookworm" server provides access to Rust crate type definitions,
            documentation and source code.
        "#}
        .to_owned()
    }

    fn capabilities(&self) -> ServerCapabilities {
        CapabilitiesBuilder::new().with_tools(false).build()
    }

    fn list_tools(&self) -> Vec<Tool> {
        let mut tools = vec![];

        load_tool::<tool::SearchCrates>(&mut tools);
        load_tool::<tool::SearchCrateItems>(&mut tools);
        // load_tool::<tool::SearchCrateSrc>(&mut tools);
        load_tool::<tool::CrateResource>(&mut tools);
        load_tool::<tool::CrateVersions>(&mut tools);
        load_tool::<tool::CrateReadme>(&mut tools);

        tools
    }

    fn call_tool(
        &self,
        tool_name: &str,
        args: Value,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<Content>, ToolError>> + Send + 'static>> {
        let tool_name = tool_name.to_owned();

        Box::pin(async move {
            Ok(match tool_name.as_str() {
                "crates_search" => tool::SearchCrates::try_from(args)?.run().await?,
                "crate_search_items" => tool::SearchCrateItems::try_from(args)?.run().await?,
                // "crate_search_src" => tool::SearchCrateItems::try_from(args)?.run().await?,
                "crate_resource" => tool::CrateResource::try_from(args)?.run().await?,
                "crate_versions" => tool::CrateVersions::try_from(args)?.run().await?,
                "crate_readme" => tool::CrateReadme::try_from(args)?.run().await?,
                _ => {
                    return Err(ToolError::NotFound(
                        formatdoc! {"
                        Tool '{}' not found.

                        Available tools:

                        - `crates_search`
                        - `crate_search_items`
                        - `crate_resource`
                        - `crate_versions`
                        - `crate_readme`
                ", tool_name}
                        .to_owned(),
                    ))
                }
            })
        })
    }

    fn list_resources(&self) -> Vec<Resource> {
        vec![]
    }

    fn read_resource(
        &self,
        _uri: &str,
    ) -> Pin<Box<dyn Future<Output = Result<String, ResourceError>> + Send + 'static>> {
        Box::pin(async move {
            Err(ResourceError::NotFound(
                "This server does not provide any resources".to_string(),
            ))
        })
    }

    fn list_prompts(&self) -> Vec<Prompt> {
        vec![]
    }

    fn get_prompt(
        &self,
        _prompt_name: &str,
    ) -> Pin<Box<dyn Future<Output = Result<String, PromptError>> + Send + 'static>> {
        Box::pin(async move {
            Err(PromptError::NotFound(
                "This server does not provide any prompts".to_string(),
            ))
        })
    }
}

fn load_tool<S: JsonSchema>(tools: &mut Vec<Tool>) {
    match Tool::try_from_schema::<S>() {
        Ok(tool) => tools.push(tool),
        Err(e) => eprintln!("Failed to load tool: {e}"),
    }
}
