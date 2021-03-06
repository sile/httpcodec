use bytecodec::bytes::CopyableBytesDecoder;
use bytecodec::{ByteCount, Decode, Eos, ErrorKind, Result};
use std::fmt;

/// HTTP version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum HttpVersion {
    /// HTTP/1.0
    V1_0,

    /// HTTP/1.1
    V1_1,
}
impl HttpVersion {
    /// Returns the string representation of the version.
    pub fn as_str(&self) -> &'static str {
        match *self {
            HttpVersion::V1_0 => "HTTP/1.0",
            HttpVersion::V1_1 => "HTTP/1.1",
        }
    }
}
impl AsRef<str> for HttpVersion {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}
impl fmt::Display for HttpVersion {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.as_ref().fmt(f)
    }
}

#[derive(Debug, Default)]
pub(crate) struct HttpVersionDecoder(CopyableBytesDecoder<[u8; 8]>);
impl Decode for HttpVersionDecoder {
    type Item = HttpVersion;

    fn decode(&mut self, buf: &[u8], eos: Eos) -> Result<usize> {
        track!(self.0.decode(buf, eos))
    }

    fn finish_decoding(&mut self) -> Result<Self::Item> {
        let v = track!(self.0.finish_decoding())?;
        let v = match v.as_ref() {
            b"HTTP/1.0" => HttpVersion::V1_0,
            b"HTTP/1.1" => HttpVersion::V1_1,
            _ => track_panic!(ErrorKind::InvalidInput, "Unknown HTTP version: {:?}", v),
        };
        Ok(v)
    }

    fn requiring_bytes(&self) -> ByteCount {
        self.0.requiring_bytes()
    }

    fn is_idle(&self) -> bool {
        self.0.is_idle()
    }
}

#[cfg(test)]
mod test {
    use bytecodec::io::IoDecodeExt;
    use bytecodec::ErrorKind;

    use super::*;

    #[test]
    fn http_version_decoder_works() {
        let mut decoder = HttpVersionDecoder::default();

        let item = track_try_unwrap!(decoder.decode_exact(b"HTTP/1.0\r\n".as_ref()));
        assert_eq!(item, HttpVersion::V1_0);

        let item = track_try_unwrap!(decoder.decode_exact(b"HTTP/1.1\r\n".as_ref()));
        assert_eq!(item, HttpVersion::V1_1);

        assert_eq!(
            decoder
                .decode_exact(b"HTTP/1.2\r\n".as_ref())
                .err()
                .map(|e| *e.kind()),
            Some(ErrorKind::InvalidInput)
        )
    }
}
