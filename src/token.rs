use std::mem;
use bytecodec::{ByteCount, Decode, Encode, Eos, ExactBytesEncode};
use bytecodec::bytes::BytesEncoder;

use {ErrorKind, Result};

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Token<B>(B);
impl<B: AsRef<[u8]>> Token<B> {
    pub fn new(token: B) -> Result<Self> {
        track_assert!(
            token.as_ref().iter().cloned().all(is_tchar),
            ErrorKind::InvalidInput
        );
        Ok(Token(token))
    }
}
impl<B: AsRef<[u8]>> AsRef<[u8]> for Token<B> {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

pub trait PushByte {
    fn push_byte(&mut self, b: u8) -> bool;
    fn take_bytes(&mut self) -> Self;
}
impl PushByte for Vec<u8> {
    fn push_byte(&mut self, b: u8) -> bool {
        self.push(b);
        true
    }
    fn take_bytes(&mut self) -> Self {
        mem::replace(self, Vec::new())
    }
}

#[derive(Debug, Default, Clone)]
pub struct Bytes<B> {
    inner: B,
    offset: usize,
}
impl<B: AsRef<[u8]>> AsRef<[u8]> for Bytes<B> {
    fn as_ref(&self) -> &[u8] {
        &self.inner.as_ref()[..self.offset]
    }
}
impl<B: AsMut<[u8]> + Clone> PushByte for Bytes<B> {
    fn push_byte(&mut self, b: u8) -> bool {
        if let Some(x) = self.inner.as_mut().get_mut(self.offset) {
            *x = b;
            self.offset += 1;
            true
        } else {
            false
        }
    }
    fn take_bytes(&mut self) -> Self {
        let cloned = self.clone();
        self.offset = 0;
        cloned
    }
}

#[derive(Debug, Default)]
pub struct TokenDecoder<B>(B);
impl<B: PushByte> Decode for TokenDecoder<B> {
    type Item = Token<B>;

    fn decode(&mut self, buf: &[u8], eos: Eos) -> Result<(usize, Option<Self::Item>)> {
        for i in 0..buf.len() {
            if !is_tchar(buf[i]) {
                let item = self.0.take_bytes();
                return Ok((i, Some(Token(item))));
            }
            track_assert!(self.0.push_byte(buf[i]), ErrorKind::Other, "Buffer full");
        }

        let item = if eos.is_reached() {
            Some(Token(self.0.take_bytes()))
        } else {
            None
        };
        Ok((buf.len(), item))
    }

    fn has_terminated(&self) -> bool {
        false
    }

    fn requiring_bytes(&self) -> ByteCount {
        ByteCount::Unknown
    }
}

#[derive(Debug, Default)]
pub struct TokenEncoder<B>(BytesEncoder<Token<B>>);
impl<B: AsRef<[u8]>> Encode for TokenEncoder<B> {
    type Item = Token<B>;

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
impl<B: AsRef<[u8]>> ExactBytesEncode for TokenEncoder<B> {
    fn exact_requiring_bytes(&self) -> u64 {
        self.0.exact_requiring_bytes()
    }
}

// https://tools.ietf.org/html/rfc7230#section-3.2.6
pub fn is_tchar(b: u8) -> bool {
    match b {
        b'!' | b'#' | b'$' | b'%' | b'&' | b'\'' | b'*' | b'+' | b'-' | b'.' | b'^' | b'_'
        | b'`' | b'|' | b'~' => true,
        _ => is_digit(b) || is_alpha(b),
    }
}

pub fn is_digit(b: u8) -> bool {
    b'0' <= b && b <= b'9'
}

pub fn is_alpha(b: u8) -> bool {
    (b'a' <= b && b <= b'z') || (b'A' <= b && b <= b'Z')
}
