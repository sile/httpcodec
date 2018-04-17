use std::fmt;
use bytecodec::{ByteCount, Decode, Eos, ErrorKind, Result};

use util;

/// Request target ([RFC 7230#5.3]).
///
/// [RFC 7230#5.3]: https://tools.ietf.org/html/rfc7230#section-5.3
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RequestTarget<T>(T);
impl<T: AsRef<str>> RequestTarget<T> {
    /// Makes a new `RequestTarget` instance.
    ///
    /// # Errors
    ///
    /// `target` must be composed of "VCHAR" characters that defined in [RFC 7230].
    /// If it contains any other characters,
    /// an `ErrorKind::InvalidInput` error will be returned.
    ///
    /// [RFC 7230]: https://tools.ietf.org/html/rfc7230
    pub fn new(target: T) -> Result<Self> {
        track_assert!(
            target.as_ref().bytes().all(util::is_vchar),
            ErrorKind::InvalidInput
        );
        Ok(RequestTarget(target))
    }

    /// Makes a new `RequestTarget` instance without any validation.
    pub unsafe fn new_unchecked(target: T) -> Self {
        RequestTarget(target)
    }

    /// Returns the string representation of the target.
    pub fn as_str(&self) -> &str {
        self.0.as_ref()
    }

    /// Returns a reference to the inner object of the target.
    pub fn inner_ref(&self) -> &T {
        &self.0
    }

    /// Takes ownership of the target, and returns the inner object.
    pub fn into_inner(self) -> T {
        self.0
    }
}
impl<T: AsRef<str>> AsRef<str> for RequestTarget<T> {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}
impl<T: AsRef<str>> fmt::Display for RequestTarget<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.as_ref().fmt(f)
    }
}

#[derive(Debug, Default)]
pub struct RequestTargetDecoder {
    size: usize,
}
impl Decode for RequestTargetDecoder {
    type Item = usize;

    fn decode(&mut self, buf: &[u8], eos: Eos) -> Result<(usize, Option<Self::Item>)> {
        if let Some(n) = buf.iter().position(|b| !util::is_vchar(*b)) {
            track_assert_eq!(buf[n], b' ', ErrorKind::InvalidInput);
            let size = self.size + n;
            self.size = 0;
            Ok((n + 1, Some(size)))
        } else {
            track_assert!(!eos.is_reached(), ErrorKind::UnexpectedEos);
            self.size += buf.len();
            Ok((buf.len(), None))
        }
    }

    fn has_terminated(&self) -> bool {
        false
    }

    fn requiring_bytes(&self) -> ByteCount {
        ByteCount::Unknown
    }
}
