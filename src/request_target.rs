use bytecodec::{ByteCount, Decode, Eos, ErrorKind, Result};
use std::fmt;

use util;

/// Request target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RequestTarget<'a>(&'a str);
impl<'a> RequestTarget<'a> {
    /// Makes a new `RequestTarget` instance.
    ///
    /// # Errors
    ///
    /// `target` must be composed of "VCHAR" characters that defined in [RFC 7230].
    /// If it contains any other characters,
    /// an `ErrorKind::InvalidInput` error will be returned.
    ///
    /// [RFC 7230]: https://tools.ietf.org/html/rfc7230
    pub fn new(target: &'a str) -> Result<Self> {
        track_assert!(target.bytes().all(util::is_vchar), ErrorKind::InvalidInput);
        Ok(RequestTarget(target))
    }

    /// Makes a new `RequestTarget` instance without any validation.
    pub unsafe fn new_unchecked(target: &'a str) -> Self {
        RequestTarget(target)
    }

    /// Returns a reference to the inner string of the method.
    pub fn as_str(&self) -> &'a str {
        self.0
    }
}
impl<'a> AsRef<str> for RequestTarget<'a> {
    fn as_ref(&self) -> &str {
        self.0
    }
}
impl<'a> fmt::Display for RequestTarget<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Default)]
pub struct RequestTargetDecoder {
    size: usize,
    idle: bool,
}
impl Decode for RequestTargetDecoder {
    type Item = usize;

    fn decode(&mut self, buf: &[u8], eos: Eos) -> Result<usize> {
        if self.idle {
            Ok(0)
        } else if let Some(n) = buf.iter().position(|b| !util::is_vchar(*b)) {
            track_assert_eq!(buf[n], b' ', ErrorKind::InvalidInput);
            self.size += n;
            self.idle = true;
            Ok(n + 1)
        } else {
            track_assert!(!eos.is_reached(), ErrorKind::UnexpectedEos);
            self.size += buf.len();
            Ok(buf.len())
        }
    }

    fn finish_decoding(&mut self) -> Result<Self::Item> {
        track_assert!(self.idle, ErrorKind::IncompleteDecoding);
        let size = self.size;
        self.size = 0;
        self.idle = false;
        Ok(size)
    }

    fn requiring_bytes(&self) -> ByteCount {
        if self.idle {
            ByteCount::Finite(0)
        } else {
            ByteCount::Unknown
        }
    }

    fn is_idle(&self) -> bool {
        self.idle
    }
}

#[cfg(test)]
mod test {
    use bytecodec::io::IoDecodeExt;
    use bytecodec::ErrorKind;

    use super::*;

    #[test]
    fn request_target_decoder_works() {
        let mut decoder = RequestTargetDecoder::default();
        let item = track_try_unwrap!(decoder.decode_exact(b"/foo/bar HTTP/1.1".as_ref()));
        assert_eq!(item, 8);

        assert_eq!(
            decoder
                .decode_exact(b"/f\too".as_ref())
                .err()
                .map(|e| *e.kind()),
            Some(ErrorKind::InvalidInput)
        )
    }
}
