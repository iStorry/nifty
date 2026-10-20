//! Talking to web servers: the client every download uses, and what a server says about
//! a file before sending it.

use std::time::Duration;

use ureq::http::Uri;
use ureq::tls::{RootCerts, TlsConfig, TlsProvider};
use ureq::{Agent, Body, ResponseExt};

/// How long connecting, and then waiting for the server to start answering, may take.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(60);

/// What a server said about the file before its body.
#[derive(Debug)]
pub(crate) struct Answer {
    /// The address the body came from, after any redirects.
    pub uri: Uri,
    /// The body's length, when the server said.
    pub length: Option<u64>,
}

pub(crate) fn agent() -> Agent {
    let tls = TlsConfig::builder()
        .provider(TlsProvider::NativeTls)
        .root_certs(RootCerts::PlatformVerifier)
        .build();
    Agent::config_builder()
        .tls_config(tls)
        .timeout_connect(Some(CONNECT_TIMEOUT))
        .timeout_recv_response(Some(RESPONSE_TIMEOUT))
        .build()
        .into()
}

/// Asks for `url`, giving what the server said and the body still to read.
pub(crate) fn get(agent: &Agent, url: &str) -> Result<(Answer, Body), ureq::Error> {
    let response = agent.get(url).call()?;
    let answer = Answer {
        uri: response.get_uri().clone(),
        length: response.body().content_length(),
    };
    Ok((answer, response.into_body()))
}
