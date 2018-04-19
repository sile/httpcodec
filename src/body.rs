use bytecodec::{ByteCount, Decode, Encode, Eos, Result};
use bytecodec::combinator::Length;

use {Header, HeaderFields, HeaderMut};

pub trait BodyDecode: Decode {
    #[allow(unused_variables)]
    fn initialize(&mut self, header: &Header) -> Result<()> {
        Ok(())
    }
}

pub trait BodyEncode: Encode {
    #[allow(unused_variables)]
    fn update_header(&self, header: &mut HeaderMut) {}
}

// TODO: HeadBodyEncoder

// TODO: s/Unread/HeadBodyDecoder/
#[derive(Debug)]
pub struct Unread;

#[derive(Debug, Default)]
pub struct UnreadDecoder;
impl Decode for UnreadDecoder {
    type Item = Unread;

    fn decode(&mut self, _buf: &[u8], _eos: Eos) -> Result<(usize, Option<Self::Item>)> {
        Ok((0, Some(Unread)))
    }

    fn has_terminated(&self) -> bool {
        false
    }

    fn requiring_bytes(&self) -> ByteCount {
        ByteCount::Finite(0)
    }
}

#[derive(Debug)]
pub enum BodyDecoder<T> {
    Chunked(ChunkedBodyDecoder<T>),
    WithLength(Length<T>),
    WithoutLength(T),
}
impl<T: Decode> BodyDecoder<T> {
    pub fn from_header(_fields: HeaderFields) -> Self {
        unimplemented!()
    }
}
impl<T: Decode> Decode for BodyDecoder<T> {
    type Item = T::Item;

    fn decode(&mut self, buf: &[u8], eos: Eos) -> Result<(usize, Option<Self::Item>)> {
        match *self {
            BodyDecoder::Chunked(ref mut d) => track!(d.decode(buf, eos)),
            BodyDecoder::WithLength(ref mut d) => track!(d.decode(buf, eos)),
            BodyDecoder::WithoutLength(ref mut d) => track!(d.decode(buf, eos)),
        }
    }

    fn has_terminated(&self) -> bool {
        match *self {
            BodyDecoder::Chunked(ref d) => d.has_terminated(),
            BodyDecoder::WithLength(ref d) => d.has_terminated(),
            BodyDecoder::WithoutLength(ref d) => d.has_terminated(),
        }
    }

    fn requiring_bytes(&self) -> ByteCount {
        match *self {
            BodyDecoder::Chunked(ref d) => d.requiring_bytes(),
            BodyDecoder::WithLength(ref d) => d.requiring_bytes(),
            BodyDecoder::WithoutLength(ref d) => d.requiring_bytes(),
        }
    }
}

#[derive(Debug, Default)]
pub struct ChunkedBodyDecoder<T>(T);
impl<T: Decode> Decode for ChunkedBodyDecoder<T> {
    type Item = T::Item;

    fn decode(&mut self, _buf: &[u8], _eos: Eos) -> Result<(usize, Option<Self::Item>)> {
        unimplemented!()
    }

    fn has_terminated(&self) -> bool {
        unimplemented!()
    }

    fn requiring_bytes(&self) -> ByteCount {
        unimplemented!()
    }
}
