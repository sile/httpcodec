use std::fmt;
use bytecodec::{ByteCount, Decode, Eos, ErrorKind, Result};

use util;

/// HTTP method.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Method<T>(T);
impl<T: AsRef<str>> Method<T> {
    /// Makes a new `Method` instance.
    ///
    /// # Errors
    ///
    /// `method` must be a "token" defined in [RFC 7230].
    /// Otherwise it will return an `ErrorKind::InvalidInput` error.
    ///
    /// [RFC 7230]: https://tools.ietf.org/html/rfc7230
    pub fn new(method: T) -> Result<Self> {
        track_assert!(
            method.as_ref().bytes().all(util::is_tchar),
            ErrorKind::InvalidInput
        );
        Ok(Method(method))
    }

    /// Makes a new `Method` instance without any validation.
    pub unsafe fn new_unchecked(method: T) -> Self {
        Method(method)
    }

    /// Returns the name of the method.
    pub fn as_str(&self) -> &str {
        self.0.as_ref()
    }

    /// Returns a reference to the inner object of the method.
    pub fn inner_ref(&self) -> &T {
        &self.0
    }

    /// Takes ownership of the method, and returns the inner object.
    pub fn into_inner(self) -> T {
        self.0
    }
}
impl<T: AsRef<str>> AsRef<str> for Method<T> {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}
impl<T: AsRef<str>> fmt::Display for Method<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.as_ref().fmt(f)
    }
}

#[derive(Debug, Default)]
pub(crate) struct MethodDecoder {
    size: usize,
}
impl Decode for MethodDecoder {
    type Item = usize;

    fn decode(&mut self, buf: &[u8], eos: Eos) -> Result<(usize, Option<Self::Item>)> {
        if let Some(n) = buf.iter().position(|b| !util::is_tchar(*b)) {
            track_assert_eq!(buf[n] as char, ' ', ErrorKind::InvalidInput);
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
