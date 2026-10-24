//! Native messaging's framing: each message is its length, a 32-bit number
//! in the machine's byte order, then that many bytes of UTF-8 JSON.
//!
//! The host and the app frame their messages the same way on the socket, so
//! the host passes the extension's messages on as they came. Both ends take
//! at most [`LIMIT`] bytes a message, whichever way it goes: the browsers
//! take no more from a host, and no message of ours comes near it, so a
//! length past it means the stream is broken, and nothing that large is
//! read into memory.

use std::io::{self, ErrorKind, Read, Write};

use serde::Serialize;
use serde::de::DeserializeOwned;

/// The most a message may be: what Chrome and Firefox take from a host, 1 MB
/// (1024 × 1024 bytes in both).
/// <https://developer.chrome.com/docs/extensions/develop/concepts/native-messaging#native-messaging-host-protocol>
/// <https://developer.mozilla.org/en-US/docs/Mozilla/Add-ons/WebExtensions/Native_messaging#app_side>
pub const LIMIT: usize = 1024 * 1024;

/// Reads one message; `None` if the stream ended before it began. A stream
/// that ends partway, or a length past `limit`, is an error.
pub fn read_message(r: &mut impl Read, limit: usize) -> io::Result<Option<Vec<u8>>> {
    let mut header = [0u8; 4];
    let mut got = 0;
    while got < header.len() {
        match r.read(&mut header[got..]) {
            Ok(0) if got == 0 => return Ok(None),
            Ok(0) => return Err(ErrorKind::UnexpectedEof.into()),
            Ok(n) => got += n,
            Err(e) if e.kind() == ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    let len = u32::from_ne_bytes(header) as usize;
    if len > limit {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            format!("a message of {len} bytes, more than {limit}"),
        ));
    }
    let mut message = vec![0; len];
    r.read_exact(&mut message)?;
    Ok(Some(message))
}

/// Writes one message, and flushes it: the other side waits for it.
pub fn write_message(w: &mut impl Write, message: &[u8], limit: usize) -> io::Result<()> {
    w.write_all(&frame(message, limit)?)?;
    w.flush()
}

/// The bytes of one message, its length first, so it can be written in one
/// go.
pub fn frame(message: &[u8], limit: usize) -> io::Result<Vec<u8>> {
    let len = u32::try_from(message.len())
        .ok()
        .filter(|&len| len as usize <= limit)
        .ok_or_else(|| {
            io::Error::new(
                ErrorKind::InvalidInput,
                format!("a message of {} bytes, more than {limit}", message.len()),
            )
        })?;
    let mut framed = Vec::with_capacity(4 + message.len());
    framed.extend_from_slice(&len.to_ne_bytes());
    framed.extend_from_slice(message);
    Ok(framed)
}

/// Reads one message as `T`. Not JSON, or not a `T`, is
/// [`ErrorKind::InvalidData`].
pub fn read_json<T: DeserializeOwned>(r: &mut impl Read, limit: usize) -> io::Result<Option<T>> {
    match read_message(r, limit)? {
        Some(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
        None => Ok(None),
    }
}

/// Writes `message` as JSON.
pub fn write_json(w: &mut impl Write, message: &impl Serialize, limit: usize) -> io::Result<()> {
    write_message(w, &serde_json::to_vec(message)?, limit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::FromExtension;

    fn framed(json: &str) -> Vec<u8> {
        frame(json.as_bytes(), LIMIT).expect("fits")
    }

    #[test]
    fn round_trip() {
        let mut stream = Vec::new();
        write_message(
            &mut stream,
            b"{\"type\":\"tab_closed\",\"tab_id\":1}",
            LIMIT,
        )
        .expect("written");
        write_message(&mut stream, b"{}", LIMIT).expect("written");
        assert_eq!(&stream[..4], &32u32.to_ne_bytes());
        let mut r = stream.as_slice();
        let first: FromExtension = read_json(&mut r, LIMIT).expect("read").expect("a message");
        assert_eq!(first, FromExtension::TabClosed { tab_id: 1 });
        assert_eq!(
            read_message(&mut r, LIMIT).expect("read"),
            Some(b"{}".to_vec())
        );
        assert_eq!(read_message(&mut r, LIMIT).expect("read"), None);
    }

    #[test]
    fn empty_stream_is_the_end() {
        assert_eq!(read_message(&mut &[][..], LIMIT).expect("read"), None);
    }

    #[test]
    fn truncated_length_is_an_error() {
        let err = read_message(&mut &[5u8, 0][..], LIMIT).expect_err("truncated");
        assert_eq!(err.kind(), ErrorKind::UnexpectedEof);
    }

    #[test]
    fn truncated_body_is_an_error() {
        let mut stream = framed("{\"type\":\"hello\"}");
        stream.truncate(stream.len() - 3);
        let err = read_message(&mut stream.as_slice(), LIMIT).expect_err("truncated");
        assert_eq!(err.kind(), ErrorKind::UnexpectedEof);
    }

    /// A length past the limit fails before anything that large is taken.
    #[test]
    fn oversize_is_refused_both_ways() {
        let header = ((LIMIT + 1) as u32).to_ne_bytes();
        let err = read_message(&mut &header[..], LIMIT).expect_err("too big");
        assert_eq!(err.kind(), ErrorKind::InvalidData);
        let err = read_message(&mut &u32::MAX.to_ne_bytes()[..], LIMIT).expect_err("too big");
        assert_eq!(err.kind(), ErrorKind::InvalidData);

        let mut out = Vec::new();
        let err = write_message(&mut out, &vec![b' '; LIMIT + 1], LIMIT).expect_err("too big");
        assert_eq!(err.kind(), ErrorKind::InvalidInput);
        assert!(out.is_empty());
        write_message(&mut out, &vec![b' '; LIMIT], LIMIT).expect("just fits");
    }

    /// Reads that come in pieces, as from a pipe, still make one message.
    #[test]
    fn short_reads() {
        struct Trickle<'a>(&'a [u8]);
        impl Read for Trickle<'_> {
            fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
                let n = self.0.len().min(buf.len()).min(1);
                buf[..n].copy_from_slice(&self.0[..n]);
                self.0 = &self.0[n..];
                Ok(n)
            }
        }
        let stream = framed("{\"a\":1}");
        let mut r = Trickle(&stream);
        assert_eq!(
            read_message(&mut r, LIMIT).expect("read"),
            Some(b"{\"a\":1}".to_vec())
        );
    }

    #[test]
    fn not_json_is_invalid_data() {
        let stream = framed("not json");
        let err = read_json::<FromExtension>(&mut stream.as_slice(), LIMIT).expect_err("not JSON");
        assert_eq!(err.kind(), ErrorKind::InvalidData);
    }
}
