//! # Response module.
//!
//! A [`Response`] is the type of data sent by the server to the
//! client straight after receiving a request.

use super::Timer;

/// The response struct.
///
/// Responses are sent by servers and received by clients.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Response {
    /// Default response when everything goes fine.
    Ok,

    /// Response that contains the current timer.
    Timer(Timer),
}
