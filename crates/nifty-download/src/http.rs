//! Talking to web servers: the client every download uses, and what a server says about
//! a file before sending it.

use std::time::Duration;

use ureq::http::header::{CONTENT_RANGE, RANGE};
use ureq::http::{StatusCode, Uri};
use ureq::tls::{RootCerts, TlsConfig, TlsProvider};
use ureq::{Agent, Body, ResponseExt};

use crate::connection::ConnectionError;

/// How long connecting, and then waiting for the server to start answering, may take.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(60);

/// What a server said about the file before its body.
#[derive(Debug)]
pub(crate) struct Answer {
    /// The address the body came from, after any redirects.
    pub uri: Uri,
    /// The whole file's length, when the server said.
    pub total: Option<u64>,
    /// Where in the file the body starts: `0`, unless the server sent only the rest of it.
    pub starts_at: u64,
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

/// Asks for `url`, from byte `from` on when it is more than `0`, giving what the server
/// said and the body still to read. A server that can't send part of a file sends all of
/// it, which [`Answer::starts_at`] shows.
///
/// # Errors
/// When the request fails, or the server sends part of the file without saying which.
pub(crate) fn get(agent: &Agent, url: &str, from: u64) -> Result<(Answer, Body), ConnectionError> {
    let mut request = agent.get(url);
    if from > 0 {
        request = request.header(RANGE, format!("bytes={from}-"));
    }
    let response = request.call()?;
    let uri = response.get_uri().clone();
    let answer = if response.status() == StatusCode::PARTIAL_CONTENT {
        let (starts_at, total) = response
            .headers()
            .get(CONTENT_RANGE)
            .and_then(|range| range.to_str().ok())
            .and_then(content_range)
            .ok_or(ConnectionError::UnclearPart)?;
        Answer {
            uri,
            total,
            starts_at,
        }
    } else {
        Answer {
            uri,
            total: response.body().content_length(),
            starts_at: 0,
        }
    };
    Ok((answer, response.into_body()))
}

/// Where a `Content-Range` such as `bytes 100-199/200` starts, and the whole file's
/// length when it says, which `*` doesn't.
fn content_range(range: &str) -> Option<(u64, Option<u64>)> {
    let (span, total) = range.strip_prefix("bytes ")?.split_once('/')?;
    let (start, _) = span.split_once('-')?;
    Some((start.trim().parse().ok()?, total.trim().parse().ok()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_where_a_part_starts_and_the_whole_length() {
        assert_eq!(content_range("bytes 100-199/200"), Some((100, Some(200))));
        assert_eq!(content_range("bytes 0-9/*"), Some((0, None)));
        assert_eq!(content_range("bytes */200"), None);
        assert_eq!(content_range("items 1-2/3"), None);
    }
}
