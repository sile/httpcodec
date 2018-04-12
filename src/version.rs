use bytecodec::{ByteCount, Decode, Encode, Eos, ExactBytesEncode};
use bytecodec::bytes::BytesEncoder;

use {ErrorKind, Result};

#[derive(Debug)]
pub enum HttpVersion {
    V1_0,
    V1_1,
}
impl AsRef<[u8]> for HttpVersion {
    fn as_ref(&self) -> &[u8] {
        match *self {
            HttpVersion::V1_0 => b"HTTP/1.0",
            HttpVersion::V1_1 => b"HTTP/1.1",
        }
    }
}

#[derive(Debug, Default)]
pub struct HttpVersionDecoder {
    position: usize,
}
impl Decode for HttpVersionDecoder {
    type Item = HttpVersion;

    fn decode(&mut self, buf: &[u8], eos: Eos) -> Result<(usize, Option<Self::Item>)> {
        for (i, b) in buf.iter().cloned().enumerate() {
            if self.position == 7 {
                let version = match b {
                    b'0' => HttpVersion::V1_0,
                    b'1' => HttpVersion::V1_1,
                    _ => track_panic!(
                        ErrorKind::InvalidInput,
                        "Unknown HTTP version: 1.{}",
                        char::from(b)
                    ),
                };
                self.position = 0;
                return Ok((i + 1, Some(version)));
            } else {
                track_assert_eq!(b"HTTP/1."[self.position], b, ErrorKind::InvalidInput);
            }
        }
        track_assert!(!eos.is_reached(), ErrorKind::UnexpectedEos);
        Ok((buf.len(), None))
    }

    fn has_terminated(&self) -> bool {
        false
    }

    fn requiring_bytes(&self) -> ByteCount {
        ByteCount::Finite((8 - self.position) as u64)
    }
}

#[derive(Debug, Default)]
pub struct HttpVersionEncoder(BytesEncoder<HttpVersion>);
impl Encode for HttpVersionEncoder {
    type Item = HttpVersion;

    fn encode(&mut self, buf: &mut [u8], eos: Eos) -> Result<usize> {
        track!(self.0.encode(buf, eos))
    }

    fn start_encoding(&mut self, item: Self::Item) -> Result<()> {
        track!(self.0.start_encoding(item))
    }

    fn is_idle(&self) -> bool {
        self.0.is_idle()
    }

    fn requiring_bytes(&self) -> ByteCount {
        self.0.requiring_bytes()
    }
}
impl ExactBytesEncode for HttpVersionEncoder {
    fn exact_requiring_bytes(&self) -> u64 {
        self.0.exact_requiring_bytes()
    }
}
