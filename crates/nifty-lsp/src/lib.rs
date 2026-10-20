//! A language server for Nifty: diagnostics, completion, signature help, semantic
//! highlighting, the document outline, go to definition, find references and highlighting
//! the name under the cursor. Every file of a project folder is analysed together, so names
//! resolve across files.

mod context;
mod document;
mod features;
mod index;
mod project;
mod server;
mod signature;
mod workspace;

use lsp_server::Connection;
use lsp_types::{
    CompletionOptions, HoverProviderCapability, InitializeParams, InitializeResult, OneOf,
    SemanticTokensFullOptions, SemanticTokensOptions, SemanticTokensServerCapabilities,
    ServerCapabilities, ServerInfo, SignatureHelpOptions, TextDocumentSyncCapability,
    TextDocumentSyncKind,
};

use crate::document::Encoding;
use crate::features::semantic_tokens;
use crate::server::Server;

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Result<T> = std::result::Result<T, Error>;

const SERVER_NAME: &str = "nifty-lsp";

/// Serves the language server protocol over stdin and stdout until the client exits.
///
/// # Errors
///
/// Fails when the connection breaks or the client breaks the protocol.
pub fn run() -> Result<()> {
    let (connection, io_threads) = Connection::stdio();
    serve(&connection)?;
    drop(connection);
    io_threads.join()?;
    Ok(())
}

fn serve(connection: &Connection) -> Result<()> {
    let (id, params) = connection.initialize_start()?;
    let params: InitializeParams = serde_json::from_value(params)?;
    let encoding = Encoding::negotiate(&params.capabilities);
    let result = InitializeResult {
        capabilities: capabilities(encoding),
        server_info: Some(ServerInfo {
            name: SERVER_NAME.to_owned(),
            version: Some(env!("CARGO_PKG_VERSION").to_owned()),
        }),
    };
    connection.initialize_finish(id, serde_json::to_value(result)?)?;
    Server::new(connection, encoding).run()
}

fn capabilities(encoding: Encoding) -> ServerCapabilities {
    ServerCapabilities {
        position_encoding: Some(encoding.kind()),
        text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
        semantic_tokens_provider: Some(SemanticTokensServerCapabilities::SemanticTokensOptions(
            SemanticTokensOptions {
                legend: semantic_tokens::legend(),
                full: Some(SemanticTokensFullOptions::Bool(true)),
                ..SemanticTokensOptions::default()
            },
        )),
        completion_provider: Some(CompletionOptions {
            trigger_characters: Some(vec![".".to_owned()]),
            ..CompletionOptions::default()
        }),
        signature_help_provider: Some(SignatureHelpOptions {
            trigger_characters: Some(vec!["(".to_owned(), ",".to_owned()]),
            retrigger_characters: Some(vec![":".to_owned()]),
            ..SignatureHelpOptions::default()
        }),
        hover_provider: Some(HoverProviderCapability::Simple(true)),
        document_symbol_provider: Some(OneOf::Left(true)),
        definition_provider: Some(OneOf::Left(true)),
        references_provider: Some(OneOf::Left(true)),
        document_highlight_provider: Some(OneOf::Left(true)),
        ..ServerCapabilities::default()
    }
}

#[cfg(test)]
mod tests;
