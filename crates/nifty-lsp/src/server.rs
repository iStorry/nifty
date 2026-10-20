//! The message loop: keeps open documents up to date and answers requests about them.

use lsp_server::{Connection, ErrorCode, ExtractError, Message, Notification, Request, Response};
use lsp_types::notification::{
    DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument, LogMessage,
    Notification as LspNotification, PublishDiagnostics,
};
use lsp_types::request::{
    Completion, DocumentHighlightRequest, DocumentSymbolRequest, GotoDefinition, HoverRequest,
    References, Request as LspRequest, SemanticTokensFullRequest, SignatureHelpRequest,
};
use lsp_types::{
    CompletionParams, CompletionResponse, DidChangeTextDocumentParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, DocumentHighlight, DocumentHighlightParams, DocumentSymbolParams,
    DocumentSymbolResponse, GotoDefinitionParams, GotoDefinitionResponse, Hover, HoverParams,
    Location, LogMessageParams, MessageType, ReferenceParams, SemanticTokens, SemanticTokensParams,
    SemanticTokensResult, SignatureHelp, SignatureHelpParams, TextDocumentPositionParams,
};

use crate::Result;
use crate::document::Encoding;
use crate::features::{completion, hover, navigation, outline, semantic_tokens, signature_help};
use crate::project::Cursor;
use crate::workspace::{Refreshed, Workspace};

pub(crate) struct Server<'conn> {
    connection: &'conn Connection,
    encoding: Encoding,
    workspace: Workspace,
}

impl<'conn> Server<'conn> {
    pub(crate) fn new(connection: &'conn Connection, encoding: Encoding) -> Self {
        Self {
            connection,
            encoding,
            workspace: Workspace::default(),
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
            .on::<HoverRequest>(Self::hover)
            .on::<SemanticTokensFullRequest>(Self::semantic_tokens)
            .on::<DocumentSymbolRequest>(Self::document_symbols)
            .on::<GotoDefinition>(Self::definition)
            .on::<References>(Self::references)
            .on::<DocumentHighlightRequest>(Self::highlights)
            .finish()
    }

    fn handle_notification(&mut self, notification: Notification) -> Result<()> {
        let refreshed = match notification.method.as_str() {
            DidOpenTextDocument::METHOD => {
                let params: DidOpenTextDocumentParams = extract(notification)?;
                let document = params.text_document;
                self.workspace.update(
                    &document.uri,
                    document.text,
                    document.version,
                    self.encoding,
                )
            }
            DidChangeTextDocument::METHOD => {
                let mut params: DidChangeTextDocumentParams = extract(notification)?;
                // The server asks for full sync, so the last change holds the whole text.
                let Some(change) = params.content_changes.pop() else {
                    return Ok(());
                };
                let document = params.text_document;
                self.workspace
                    .update(&document.uri, change.text, document.version, self.encoding)
            }
            DidCloseTextDocument::METHOD => {
                let params: DidCloseTextDocumentParams = extract(notification)?;
                self.workspace
                    .close(&params.text_document.uri, self.encoding)
            }
            _ => return Ok(()),
        };
        self.send_refreshed(refreshed)
    }

    fn send_refreshed(&self, refreshed: Refreshed) -> Result<()> {
        for message in refreshed.warnings {
            let params = LogMessageParams {
                typ: MessageType::WARNING,
                message,
            };
            self.notify::<LogMessage>(params)?;
        }
        for params in refreshed.diagnostics {
            self.notify::<PublishDiagnostics>(params)?;
        }
        Ok(())
    }

    fn notify<N: LspNotification>(&self, params: N::Params) -> Result<()> {
        let notification = Notification::new(N::METHOD.to_owned(), params);
        self.connection.sender.send(notification.into())?;
        Ok(())
    }

    fn completion(&self, params: &CompletionParams) -> Option<CompletionResponse> {
        let cursor = self.locate(&params.text_document_position)?;
        completion::completions(cursor).map(CompletionResponse::Array)
    }

    fn signature_help(&self, params: &SignatureHelpParams) -> Option<SignatureHelp> {
        signature_help::signature_help(self.locate(&params.text_document_position_params)?)
    }

    fn hover(&self, params: &HoverParams) -> Option<Hover> {
        let cursor = self.locate(&params.text_document_position_params)?;
        hover::hover(cursor, self.encoding)
    }

    fn semantic_tokens(&self, params: &SemanticTokensParams) -> Option<SemanticTokensResult> {
        let (project, file) = self.workspace.file(&params.text_document.uri)?;
        let data = semantic_tokens::semantic_tokens(project, file, self.encoding);
        Some(SemanticTokensResult::Tokens(SemanticTokens {
            result_id: None,
            data,
        }))
    }

    fn document_symbols(&self, params: &DocumentSymbolParams) -> Option<DocumentSymbolResponse> {
        let (project, file) = self.workspace.file(&params.text_document.uri)?;
        let symbols = outline::outline(project.document(file), self.encoding);
        Some(DocumentSymbolResponse::Nested(symbols))
    }

    fn definition(&self, params: &GotoDefinitionParams) -> Option<GotoDefinitionResponse> {
        let cursor = self.locate(&params.text_document_position_params)?;
        let location = navigation::definition(cursor, self.encoding)?;
        Some(GotoDefinitionResponse::Scalar(location))
    }

    fn references(&self, params: &ReferenceParams) -> Option<Vec<Location>> {
        let cursor = self.locate(&params.text_document_position)?;
        let include_declaration = params.context.include_declaration;
        navigation::references(cursor, include_declaration, self.encoding)
    }

    fn highlights(&self, params: &DocumentHighlightParams) -> Option<Vec<DocumentHighlight>> {
        let cursor = self.locate(&params.text_document_position_params)?;
        navigation::highlights(cursor, self.encoding)
    }

    /// The project file a request points into, and the byte offset it points at.
    fn locate(&self, params: &TextDocumentPositionParams) -> Option<Cursor<'_>> {
        self.workspace
            .locate(&params.text_document.uri, params.position, self.encoding)
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
