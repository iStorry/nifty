//! Drives a whole session over an in-memory connection, as an editor would.

use std::thread::{self, JoinHandle};

use lsp_server::{Connection, Message, Notification, Request, RequestId, Response};
use lsp_types::notification::{
    DidOpenTextDocument, Exit, Initialized, Notification as LspNotification, PublishDiagnostics,
};
use lsp_types::request::{GotoDefinition, Initialize, Request as LspRequest, Shutdown};
use lsp_types::{
    DidOpenTextDocumentParams, GotoDefinitionParams, GotoDefinitionResponse, InitializeParams,
    InitializeResult, InitializedParams, Location, PartialResultParams, Position,
    PositionEncodingKind, PublishDiagnosticsParams, Range, TextDocumentIdentifier,
    TextDocumentItem, TextDocumentPositionParams, Uri, WorkDoneProgressParams,
};
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::serve;

const SOURCE: &str = "state count = 0\nfn bump() { count += 1 }\nbump(";

struct Client {
    connection: Connection,
    next_id: i32,
    server: JoinHandle<Result<(), String>>,
}

impl Client {
    /// Starts a server and completes the initialize handshake.
    fn start(params: InitializeParams) -> (Self, InitializeResult) {
        let (server, connection) = Connection::memory();
        let server = thread::spawn(move || serve(&server).map_err(|error| error.to_string()));
        let mut client = Self {
            connection,
            next_id: 0,
            server,
        };
        let initialized = client.request::<Initialize>(params);
        client.notify::<Initialized>(InitializedParams {});
        (client, initialized)
    }

    /// Opens `text` as version 1 of `uri` and returns the diagnostics the server publishes.
    fn open(&mut self, uri: &Uri, text: &str) -> PublishDiagnosticsParams {
        self.notify::<DidOpenTextDocument>(DidOpenTextDocumentParams {
            text_document: TextDocumentItem::new(
                uri.clone(),
                "nifty".to_owned(),
                1,
                text.to_owned(),
            ),
        });
        self.expect_notification::<PublishDiagnostics>()
    }

    /// Shuts the server down and checks that it exited cleanly.
    fn shut_down(mut self) {
        self.request::<Shutdown>(());
        self.notify::<Exit>(());
        let outcome = self
            .server
            .join()
            .expect("the server thread should not panic");
        assert_eq!(outcome, Ok(()));
    }

    fn request<R: LspRequest>(&mut self, params: R::Params) -> R::Result
    where
        R::Result: DeserializeOwned,
    {
        self.next_id += 1;
        let id = RequestId::from(self.next_id);
        let request = Request::new(id.clone(), R::METHOD.to_owned(), params);
        self.send(request.into());
        match self.receive() {
            Message::Response(Response {
                id: response_id,
                response_result: Ok(result),
            }) if response_id == id => parse(result),
            other => panic!("expected a response to {}, got {other:?}", R::METHOD),
        }
    }

    fn notify<N: LspNotification>(&self, params: N::Params) {
        self.send(Notification::new(N::METHOD.to_owned(), params).into());
    }

    fn expect_notification<N: LspNotification>(&self) -> N::Params {
        match self.receive() {
            Message::Notification(notification) if notification.method == N::METHOD => {
                parse(notification.params)
            }
            other => panic!("expected {}, got {other:?}", N::METHOD),
        }
    }

    fn send(&self, message: Message) {
        self.connection
            .sender
            .send(message)
            .expect("the server should be listening");
    }

    fn receive(&self) -> Message {
        self.connection
            .receiver
            .recv()
            .expect("the server should reply")
    }
}

fn messages(published: &PublishDiagnosticsParams) -> Vec<&str> {
    published
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect()
}

fn parse<T: DeserializeOwned>(value: Value) -> T {
    serde_json::from_value(value).expect("the server should send valid protocol messages")
}

#[test]
fn reports_diagnostics_and_answers_requests() {
    let (mut client, initialized) = Client::start(InitializeParams::default());
    assert_eq!(
        initialized.capabilities.position_encoding,
        Some(PositionEncodingKind::UTF16)
    );
    let uri: Uri = "file:///app.nf".parse().expect("valid uri");

    let published = client.open(&uri, SOURCE);
    assert_eq!(published.version, Some(1));
    assert_eq!(
        messages(&published),
        ["expected an expression, found end of file"]
    );

    let definition = client.request::<GotoDefinition>(GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams::new(
            TextDocumentIdentifier::new(uri.clone()),
            Position::new(1, 13),
        ),
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    });
    let declaration = Range::new(Position::new(0, 6), Position::new(0, 11));
    assert_eq!(
        definition,
        Some(GotoDefinitionResponse::Scalar(Location::new(
            uri,
            declaration
        )))
    );

    client.shut_down();
}

#[test]
fn reports_type_errors_once_the_file_parses() {
    let (mut client, _) = Client::start(InitializeParams::default());
    let uri: Uri = "file:///app.nf".parse().expect("valid uri");

    let published = client.open(&uri, "window(\"App\") {\n  missing()\n}\n");
    assert_eq!(messages(&published), ["unknown element or view `missing`"]);

    client.shut_down();
}

#[test]
fn counts_columns_in_utf8_when_the_client_offers_it() {
    let mut params = InitializeParams::default();
    params.capabilities.general = Some(lsp_types::GeneralClientCapabilities {
        position_encodings: Some(vec![
            PositionEncodingKind::UTF16,
            PositionEncodingKind::UTF8,
        ]),
        ..lsp_types::GeneralClientCapabilities::default()
    });
    let (client, initialized) = Client::start(params);
    assert_eq!(
        initialized.capabilities.position_encoding,
        Some(PositionEncodingKind::UTF8)
    );
    client.shut_down();
}
