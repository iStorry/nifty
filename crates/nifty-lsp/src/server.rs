//! The message loop: keeps open documents up to date and answers requests about them.

use std::collections::HashMap;

use lsp_server::{Connection, ErrorCode, ExtractError, Message, Notification, Request, Response};
use lsp_types::notification::{
    DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument,
    Notification as LspNotification, PublishDiagnostics,
};
use lsp_types::request::{
    Completion, DocumentHighlightRequest, DocumentSymbolRequest, GotoDefinition, References,
    Request as LspRequest, SemanticTokensFullRequest, SignatureHelpRequest,
};
use lsp_types::{
    CompletionParams, CompletionResponse, DidChangeTextDocumentParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, DocumentHighlight, DocumentHighlightParams, DocumentSymbolParams,
    DocumentSymbolResponse, GotoDefinitionParams, GotoDefinitionResponse, Location,
    PublishDiagnosticsParams, ReferenceParams, SemanticTokens, SemanticTokensParams,
    SemanticTokensResult, SignatureHelp, SignatureHelpParams, TextDocumentPositionParams, Uri,
};

use crate::Result;
use crate::document::{Document, Encoding};
use crate::features::{
    completion, diagnostics, navigation, outline, semantic_tokens, signature_help,
};

pub(crate) struct Server<'conn> {
    connection: &'conn Connection,
    encoding: Encoding,
    documents: HashMap<Uri, Document>,
}

impl<'conn> Server<'conn> {
    pub(crate) fn new(connection: &'conn Connection, encoding: Encoding) -> Self {
        Self {
            connection,
            encoding,
            documents: HashMap::new(),
        }
    }

    /// Runs until the client asks the server to shut down or closes the connection.
    pub(crate) fn run(mut self) -> Result<()> {
        for message in &self.connection.receiver {
            match message {
                Message::Request(request) => {
                    if self.connection.handle_shutdown(&request)? {
                        return Ok(());
                    }
                    let response = self.handle_request(request);
                    self.connection.sender.send(response.into())?;
                }
                Message::Notification(notification) => self.handle_notification(notification)?,
                Message::Response(_) => {}
            }
        }
        Ok(())
    }

    fn handle_request(&self, request: Request) -> Response {
        RequestDispatch::new(self, request)
            .on::<Completion>(Self::completion)
            .on::<SignatureHelpRequest>(Self::signature_help)
            .on::<SemanticTokensFullRequest>(Self::semantic_tokens)
            .on::<DocumentSymbolRequest>(Self::document_symbols)
            .on::<GotoDefinition>(Self::definition)
            .on::<References>(Self::references)
            .on::<DocumentHighlightRequest>(Self::highlights)
            .finish()
    }

    fn handle_notification(&mut self, notification: Notification) -> Result<()> {
        match notification.method.as_str() {
            DidOpenTextDocument::METHOD => {
                let params: DidOpenTextDocumentParams = extract(notification)?;
                let document = params.text_document;
                self.update(document.uri, document.text, document.version)
            }
            DidChangeTextDocument::METHOD => {
                let mut params: DidChangeTextDocumentParams = extract(notification)?;
                // The server asks for full sync, so the last change holds the whole text.
                let Some(change) = params.content_changes.pop() else {
                    return Ok(());
                };
                let document = params.text_document;
                self.update(document.uri, change.text, document.version)
            }
            DidCloseTextDocument::METHOD => {
                let params: DidCloseTextDocumentParams = extract(notification)?;
                let uri = params.text_document.uri;
                self.documents.remove(&uri);
                self.publish_diagnostics(uri, Vec::new(), None)
            }
            _ => Ok(()),
        }
    }

    fn update(&mut self, uri: Uri, text: String, version: i32) -> Result<()> {
        let document = Document::new(text);
        let found = diagnostics::diagnostics(&document, self.encoding);
        self.documents.insert(uri.clone(), document);
        self.publish_diagnostics(uri, found, Some(version))
    }

    fn publish_diagnostics(
        &self,
        uri: Uri,
        diagnostics: Vec<lsp_types::Diagnostic>,
        version: Option<i32>,
    ) -> Result<()> {
        let params = PublishDiagnosticsParams {
            uri,
            diagnostics,
            version,
        };
        let notification = Notification::new(PublishDiagnostics::METHOD.to_owned(), params);
        self.connection.sender.send(notification.into())?;
        Ok(())
    }

    fn completion(&self, params: &CompletionParams) -> Option<CompletionResponse> {
        let (document, offset) = self.locate(&params.text_document_position)?;
        completion::completions(document, offset).map(CompletionResponse::Array)
    }

    fn signature_help(&self, params: &SignatureHelpParams) -> Option<SignatureHelp> {
        let (document, offset) = self.locate(&params.text_document_position_params)?;
        signature_help::signature_help(document, offset)
    }

    fn semantic_tokens(&self, params: &SemanticTokensParams) -> Option<SemanticTokensResult> {
        let document = self.documents.get(&params.text_document.uri)?;
        let data = semantic_tokens::semantic_tokens(document, self.encoding);
        Some(SemanticTokensResult::Tokens(SemanticTokens {
            result_id: None,
            data,
        }))
    }

    fn document_symbols(&self, params: &DocumentSymbolParams) -> Option<DocumentSymbolResponse> {
        let document = self.documents.get(&params.text_document.uri)?;
        let symbols = outline::outline(document, self.encoding);
        Some(DocumentSymbolResponse::Nested(symbols))
    }

    fn definition(&self, params: &GotoDefinitionParams) -> Option<GotoDefinitionResponse> {
        let position = &params.text_document_position_params;
        let (document, offset) = self.locate(position)?;
        let range = navigation::definition(document, offset, self.encoding)?;
        let uri = position.text_document.uri.clone();
        Some(GotoDefinitionResponse::Scalar(Location::new(uri, range)))
    }

    fn references(&self, params: &ReferenceParams) -> Option<Vec<Location>> {
        let position = &params.text_document_position;
        let (document, offset) = self.locate(position)?;
        let include_declaration = params.context.include_declaration;
        let ranges = navigation::references(document, offset, include_declaration, self.encoding)?;
        let locations = ranges
            .into_iter()
            .map(|range| Location::new(position.text_document.uri.clone(), range))
            .collect();
        Some(locations)
    }

    fn highlights(&self, params: &DocumentHighlightParams) -> Option<Vec<DocumentHighlight>> {
        let (document, offset) = self.locate(&params.text_document_position_params)?;
        navigation::highlights(document, offset, self.encoding)
    }

    /// The open document a request points into, and the byte offset it points at.
    fn locate(&self, params: &TextDocumentPositionParams) -> Option<(&Document, usize)> {
        let document = self.documents.get(&params.text_document.uri)?;
        let offset = document.offset(params.position, self.encoding);
        Some((document, offset))
    }
}

/// Routes a request to the first handler registered for its method.
struct RequestDispatch<'a, 'conn> {
    server: &'a Server<'conn>,
    state: DispatchState,
}

enum DispatchState {
    Pending(Request),
    Answered(Response),
}

impl<'a, 'conn> RequestDispatch<'a, 'conn> {
    fn new(server: &'a Server<'conn>, request: Request) -> Self {
        Self {
            server,
            state: DispatchState::Pending(request),
        }
    }

    fn on<R: LspRequest>(self, handler: fn(&Server<'conn>, &R::Params) -> R::Result) -> Self {
        let DispatchState::Pending(request) = self.state else {
            return self;
        };
        let id = request.id.clone();
        let state = match request.extract::<R::Params>(R::METHOD) {
            Ok((id, params)) => {
                DispatchState::Answered(Response::new_ok(id, handler(self.server, &params)))
            }
            Err(ExtractError::MethodMismatch(request)) => DispatchState::Pending(request),
            Err(ExtractError::JsonError { method, error }) => {
                DispatchState::Answered(Response::new_err(
                    id,
                    ErrorCode::InvalidParams as i32,
                    format!("invalid params for {method}: {error}"),
                ))
            }
        };
        Self {
            server: self.server,
            state,
        }
    }

    fn finish(self) -> Response {
        match self.state {
            DispatchState::Answered(response) => response,
            DispatchState::Pending(request) => Response::new_err(
                request.id,
                ErrorCode::MethodNotFound as i32,
                format!("unsupported request: {}", request.method),
            ),
        }
    }
}

/// Malformed params for a notification the server relies on mean the client and server
/// disagree on the protocol, so the error ends the session.
fn extract<P: serde::de::DeserializeOwned>(notification: Notification) -> Result<P> {
    let method = notification.method.clone();
    notification
        .extract(&method)
        .map_err(|error| format!("invalid params for {method}: {error}").into())
}
