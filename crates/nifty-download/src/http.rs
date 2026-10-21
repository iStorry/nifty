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
    /// Where in the file the body starts: `0`, unless the server sent only part of it.
    pub starts_at: u64,
    /// Whether the server sent part of the file, as `206 Partial Content`, which says it
    /// can send any part asked for.
    pub partial: bool,
}

/// The bytes of a file from `from` up to, but not including, `to`, or to the end of the
/// file when `to` is `None`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ByteRange {
    pub from: u64,
    pub to: Option<u64>,
}

impl ByteRange {
    /// As a `Range` header's value, which counts its last byte rather than the one after.
    fn header(self) -> String {
        match self.to {
            Some(to) => format!("bytes={}-{}", self.from, to - 1),
            None => format!("bytes={}-", self.from),
        }
    }
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

/// Asks for `url`, or just `range` of it, giving what the server said and the body still
/// to read. A server that can't send part of a file sends all of it, which
/// [`Answer::starts_at`] shows.
///
/// # Errors
/// When the request fails, or the server sends part of the file without saying which.
pub(crate) fn get(
    agent: &Agent,
    url: &str,
    range: Option<ByteRange>,
) -> Result<(Answer, Body), ConnectionError> {
    let mut request = agent.get(url);
    if let Some(range) = range {
        request = request.header(RANGE, range.header());
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
            partial: true,
        }
    } else {
        Answer {
            uri,
            total: response.body().content_length(),
            starts_at: 0,
            partial: false,
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

    #[test]
    fn asks_for_a_range_by_its_last_byte() {
        let rest = ByteRange { from: 4, to: None };
        assert_eq!(rest.header(), "bytes=4-");
        let part = ByteRange {
            from: 0,
            to: Some(100),
        };
        assert_eq!(part.header(), "bytes=0-99");
    }
}
