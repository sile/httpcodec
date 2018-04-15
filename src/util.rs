use bytecodec::{ByteCount, Decode, Eos};
use bytecodec::bytes::CopyableBytesDecoder;
use bytecodec::combinator::Buffered;

use {ErrorKind, Result};

#[derive(Debug, Default)]
pub struct SpaceDecoder(CopyableBytesDecoder<[u8; 1]>);
impl Decode for SpaceDecoder {
    type Item = ();

    fn decode(&mut self, buf: &[u8], eos: Eos) -> Result<(usize, Option<Self::Item>)> {
        let (size, item) = track!(self.0.decode(buf, eos))?;
        if let Some(b) = item {
            track_assert_eq!(b[0], b' ', ErrorKind::InvalidInput);
        }
        Ok((size, item.map(|_| ())))
    }

    fn has_terminated(&self) -> bool {
        self.0.has_terminated()
    }

    fn requiring_bytes(&self) -> ByteCount {
        self.0.requiring_bytes()
    }
}

#[derive(Debug, Default)]
pub struct CrlfDecoder(CopyableBytesDecoder<[u8; 2]>);
impl Decode for CrlfDecoder {
    type Item = ();

    fn decode(&mut self, buf: &[u8], eos: Eos) -> Result<(usize, Option<Self::Item>)> {
        let (size, item) = track!(self.0.decode(buf, eos))?;
        if let Some(b) = item {
            track_assert_eq!(b, [b'\r', b'\n'], ErrorKind::InvalidInput);
        }
        Ok((size, item.map(|_| ())))
    }

    fn has_terminated(&self) -> bool {
        self.0.has_terminated()
    }

    fn requiring_bytes(&self) -> ByteCount {
        self.0.requiring_bytes()
    }
}

#[derive(Debug, Default)]
pub struct WithOwsDecoder<D: Decode> {
    is_front_ows_finished: bool,
    inner: Buffered<D>,
}
impl<D: Decode> Decode for WithOwsDecoder<D> {
    type Item = D::Item;

    fn decode(&mut self, buf: &[u8], eos: Eos) -> Result<(usize, Option<Self::Item>)> {
        let mut offset = 0;
        for &b in buf {
            if self.is_front_ows_finished || !is_whitespace(b) {
                self.is_front_ows_finished = true;
                break;
            }
            offset += 1;
        }
        if !self.is_front_ows_finished {
            track_assert!(!eos.is_reached(), ErrorKind::UnexpectedEos);
            return Ok((offset, None));
        }

        if !self.inner.has_item() {
            offset += track!(self.inner.decode(&buf[offset..], eos))?.0;
            if !self.inner.has_item() {
                return Ok((offset, None));
            }
        }

        for &b in &buf[offset..] {
            if !is_whitespace(b) {
                self.is_front_ows_finished = false;
                return Ok((offset, self.inner.take_item()));
            }
            offset += 1;
        }
        Ok((offset, None))
    }

    fn has_terminated(&self) -> bool {
        self.inner.has_terminated()
    }

    fn requiring_bytes(&self) -> ByteCount {
        ByteCount::Unknown
    }
}

#[derive(Debug, Default)]
pub struct WithSpDecoder<D: Decode> {
    inner: Buffered<D>,
    space: CopyableBytesDecoder<[u8; 1]>,
}
impl<D: Decode> Decode for WithSpDecoder<D> {
    type Item = D::Item;

    fn decode(&mut self, buf: &[u8], eos: Eos) -> Result<(usize, Option<Self::Item>)> {
        let mut offset = 0;
        if !self.inner.has_item() {
            offset += track!(self.inner.decode(buf, eos))?.0;
        }
        if self.inner.has_item() {
            let (size, item) = track!(self.space.decode(&buf[offset..], eos))?;
            offset += size;
            if let Some(b) = item {
                track_assert_eq!(b[0], b' ', ErrorKind::InvalidInput);
            }
            let item = item.and_then(|_| self.inner.take_item());
            Ok((offset, item))
        } else {
            Ok((offset, None))
        }
    }

    fn has_terminated(&self) -> bool {
        self.inner.has_terminated()
    }

    fn requiring_bytes(&self) -> ByteCount {
        // TODO
        ByteCount::Unknown
    }
}

#[derive(Debug, Default)]
pub struct WithColonDecoder<D: Decode> {
    inner: Buffered<D>,
    space: CopyableBytesDecoder<[u8; 1]>,
}
impl<D: Decode> Decode for WithColonDecoder<D> {
    type Item = D::Item;

    fn decode(&mut self, buf: &[u8], eos: Eos) -> Result<(usize, Option<Self::Item>)> {
        let mut offset = 0;
        if !self.inner.has_item() {
            offset += track!(self.inner.decode(buf, eos))?.0;
        }
        if self.inner.has_item() {
            let (size, item) = track!(self.space.decode(&buf[offset..], eos))?;
            offset += size;
            if let Some(b) = item {
                track_assert_eq!(b[0], b':', ErrorKind::InvalidInput);
            }
            let item = item.and_then(|_| self.inner.take_item());
            Ok((offset, item))
        } else {
            Ok((offset, None))
        }
    }

    fn has_terminated(&self) -> bool {
        self.inner.has_terminated()
    }

    fn requiring_bytes(&self) -> ByteCount {
        // TODO
        ByteCount::Unknown
    }
}

#[derive(Debug, Default)]
pub struct WithCrlfDecoder<D: Decode> {
    inner: Buffered<D>,
    crlf: CopyableBytesDecoder<[u8; 2]>,
}
impl<D: Decode> Decode for WithCrlfDecoder<D> {
    type Item = D::Item;

    fn decode(&mut self, buf: &[u8], eos: Eos) -> Result<(usize, Option<Self::Item>)> {
        let mut offset = 0;
        if !self.inner.has_item() {
            offset += track!(self.inner.decode(buf, eos))?.0;
        }
        if self.inner.has_item() {
            let (size, item) = track!(self.crlf.decode(&buf[offset..], eos))?;
            offset += size;
            if let Some(b) = item {
                track_assert_eq!(b, [b'\r', b'\n'], ErrorKind::InvalidInput);
            }
            let item = item.and_then(|_| self.inner.take_item());
            Ok((offset, item))
        } else {
            Ok((offset, None))
        }
    }

    fn has_terminated(&self) -> bool {
        self.inner.has_terminated()
    }

    fn requiring_bytes(&self) -> ByteCount {
        // TODO
        ByteCount::Unknown
    }
}

pub fn is_whitespace(b: u8) -> bool {
    match b {
        b' ' | b'\t' => true,
        _ => false,
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

pub fn is_vchar(b: u8) -> bool {
    0x21 <= b && b <= 0x7E
}
