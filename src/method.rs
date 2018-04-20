use std::fmt;
use bytecodec::{ByteCount, Decode, Eos, ErrorKind, Result};

use util;

/// HTTP method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Method<'a>(&'a str);
impl<'a> Method<'a> {
    /// Makes a new `Method` instance.
    ///
    /// # Errors
    ///
    /// `method` must be a "token" defined in [RFC 7230].
    /// Otherwise it will return an `ErrorKind::InvalidInput` error.
    ///
    /// [RFC 7230]: https://tools.ietf.org/html/rfc7230
    pub fn new(method: &'a str) -> Result<Self> {
        track_assert!(method.bytes().all(util::is_tchar), ErrorKind::InvalidInput);
        Ok(Method(method))
    }

    /// Makes a new `Method` instance without any validation.
    pub unsafe fn new_unchecked(method: &'a str) -> Self {
        Method(method)
    }

    /// Returns a reference to the inner string of the method.
    pub fn as_str(&self) -> &str {
        self.0
    }
}
impl<'a> AsRef<str> for Method<'a> {
    fn as_ref(&self) -> &str {
        self.0
    }
}
impl<'a> fmt::Display for Method<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
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

#[cfg(test)]
mod test {
    use bytecodec::ErrorKind;
    use bytecodec::io::IoDecodeExt;

    use super::*;

    #[test]
    fn method_decoder_works() {
        let mut decoder = MethodDecoder::default();
        let item = track_try_unwrap!(decoder.decode_exact(b"GET / HTTP/1.1".as_ref()));
        assert_eq!(item, 3);

        assert_eq!(
            decoder
                .decode_exact(b"G:T".as_ref())
                .err()
                .map(|e| *e.kind()),
            Some(ErrorKind::InvalidInput)
        )
    }
}
