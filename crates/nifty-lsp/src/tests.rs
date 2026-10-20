//! Drives a whole session over an in-memory connection, as an editor would.

use std::fs;
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
use tempfile::TempDir;
use url::Url;

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

    /// Opens `text` as version 1 of `uri`, a file outside any project, and returns the
    /// diagnostics the server publishes.
    fn open(&mut self, uri: &Uri, text: &str) -> PublishDiagnosticsParams {
        self.open_in_project(uri, text, 1).remove(0)
    }

    /// Opens `text` as version 1 of `uri` and returns the diagnostics the server publishes
    /// for each of the `files` in its project.
    fn open_in_project(
        &mut self,
        uri: &Uri,
        text: &str,
        files: usize,
    ) -> Vec<PublishDiagnosticsParams> {
        self.notify::<DidOpenTextDocument>(DidOpenTextDocumentParams {
            text_document: TextDocumentItem::new(
                uri.clone(),
                "nifty".to_owned(),
                1,
                text.to_owned(),
            ),
        });
        (0..files)
            .map(|_| self.expect_notification::<PublishDiagnostics>())
            .collect()
    }

    fn definition(&mut self, uri: &Uri, position: Position) -> Option<GotoDefinitionResponse> {
        self.request::<GotoDefinition>(GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams::new(
                TextDocumentIdentifier::new(uri.clone()),
                position,
            ),
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        })
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

    let definition = client.definition(&uri, Position::new(1, 13));
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

/// Writes `files` into a new project folder and returns it with each file's URI.
fn project_on_disk(files: &[(&str, &str)]) -> (TempDir, Vec<Uri>) {
    let root = tempfile::tempdir().expect("a scratch folder");
    let uris = files
        .iter()
        .map(|(relative, text)| {
            let path = root.path().join(relative);
            if let Some(folder) = path.parent() {
                fs::create_dir_all(folder).expect("create the file's folder");
            }
            fs::write(&path, text).expect("write the file");
            Url::from_file_path(&path)
                .expect("an absolute path")
                .as_str()
                .parse()
                .expect("a valid uri")
        })
        .collect();
    (root, uris)
}

#[test]
fn analyses_every_file_of_a_project_together() {
    let main = "window(\"App\") {\n  Header()\n}\n";
    let (_root, uris) = project_on_disk(&[
        ("main.nf", main),
        ("ui/header.nf", "view Header() {}\nstate title = 1\n"),
        ("ui/footer.nf", "state title = 2\n"),
    ]);
    let [main_uri, header_uri, footer_uri] = [&uris[0], &uris[1], &uris[2]];
    let (mut client, _) = Client::start(InitializeParams::default());

    let published = client.open_in_project(main_uri, main, 3);
    let for_file = |uri: &Uri| {
        published
            .iter()
            .find(|params| params.uri == *uri)
            .expect("every project file gets diagnostics")
    };
    assert_eq!(for_file(main_uri).version, Some(1));
    assert_eq!(messages(for_file(main_uri)), Vec::<&str>::new());
    assert_eq!(for_file(footer_uri).version, None);
    let duplicates: Vec<_> = [header_uri, footer_uri]
        .into_iter()
        .flat_map(|uri| &for_file(uri).diagnostics)
        .collect();
    assert_eq!(duplicates.len(), 1);
    assert_eq!(duplicates[0].message, "`title` is already declared");
    let related = duplicates[0]
        .related_information
        .as_ref()
        .expect("the error points at the other declaration");
    assert_eq!(related[0].message, "`title` is first declared here");

    let definition = client.definition(main_uri, Position::new(1, 4));
    let header = Range::new(Position::new(0, 5), Position::new(0, 11));
    assert_eq!(
        definition,
        Some(GotoDefinitionResponse::Scalar(Location::new(
            header_uri.clone(),
            header
        )))
    );

    client.shut_down();
}
