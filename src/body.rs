use bytecodec::{ByteCount, Decode, DecodeExt, Encode, Eos, ErrorKind, ExactBytesEncode, Result};
use bytecodec::combinator::Length;
use trackable::error::ErrorKindExt;

use {Header, HeaderMut};

/// `BodyDecode` is used for representing HTTP body decoders.
pub trait BodyDecode: Decode {
    /// This method is called before starting to decode a HTTP body.
    ///
    /// The default implementation always returns `Ok(())`.
    #[allow(unused_variables)]
    fn initialize(&mut self, header: &Header) -> Result<()> {
        Ok(())
    }
}

/// `BodyEncode` is used for representing HTTP body encoders.
pub trait BodyEncode: Encode {
    /// This method is called before starting to encode a HTTP body.
    ///
    /// It is used for adjusting HTTP header by using the encoder specific information.
    ///
    /// The default implementation does nothing.
    #[allow(unused_variables)]
    fn update_header(&self, header: &mut HeaderMut) {}
}

/// A body decoder mainly intended for HEAD responses.
///
/// This does consume no bytes and immediately returns `()` as the decoded item.
///
/// It can also be used to prefetch the HTTP header before decoding the body of a HTTP message.
#[derive(Debug, Default)]
pub struct HeadBodyDecoder;
impl Decode for HeadBodyDecoder {
    type Item = ();

    fn decode(&mut self, _buf: &[u8], _eos: Eos) -> Result<(usize, Option<Self::Item>)> {
        Ok((0, Some(())))
    }

    fn has_terminated(&self) -> bool {
        false
    }

    fn requiring_bytes(&self) -> ByteCount {
        ByteCount::Finite(0)
    }
}
impl BodyDecode for HeadBodyDecoder {}

/// A body encoder mainly intended for HEAD responses.
///
/// Although it actually does not encode anything, an inner body encoder `E` is required to correctly update HTTP headers.
#[derive(Debug, Default)]
pub struct HeadBodyEncoder<E>(E);
impl<E: BodyEncode> HeadBodyEncoder<E> {
    /// Makes a new `HeadBodyEncoder` instance.
    pub fn new(inner: E) -> Self {
        HeadBodyEncoder(inner)
    }

    /// Returns a reference to a inner body encoder.
    pub fn inner_ref(&self) -> &E {
        &self.0
    }

    /// Returns a mutable reference to a inner body encoder.
    pub fn inner_mut(&mut self) -> &mut E {
        &mut self.0
    }

    /// Takes ownership of `HeadBodyEncoder` and returns the inner body encoder.
    pub fn into_inner(self) -> E {
        self.0
    }
}
impl<E: BodyEncode> Encode for HeadBodyEncoder<E> {
    type Item = E::Item;

    fn encode(&mut self, _buf: &mut [u8], _eos: Eos) -> Result<usize> {
        // TODO: track!(self.0.cancel())?;
        Ok(0)
    }

    fn start_encoding(&mut self, item: Self::Item) -> Result<()> {
        track!(self.0.start_encoding(item))
    }

    fn is_idle(&self) -> bool {
        true
    }

    fn requiring_bytes(&self) -> ByteCount {
        ByteCount::Finite(0)
    }
}
impl<E: BodyEncode> ExactBytesEncode for HeadBodyEncoder<E> {
    fn exact_requiring_bytes(&self) -> u64 {
        0
    }
}
impl<E: BodyEncode> BodyEncode for HeadBodyEncoder<E> {
    fn update_header(&self, header: &mut HeaderMut) {
        self.0.update_header(header);
    }
}

/// Basic body decoder.
///
/// It is typically used for making a body encoder from a `Decode` implementor.
///
/// TODO: doc for header handlings
///
/// TODO: introduce BodyDecoderInner
#[derive(Debug)]
pub enum BodyDecoder<D> {
    Chunked(ChunkedBodyDecoder<D>),
    WithLength(Length<D>),
    WithoutLength(D),
}
impl<D: Decode> BodyDecoder<D> {
    pub fn new(inner: D) -> Self {
        BodyDecoder::WithoutLength(inner)
    }
}
impl<D: Decode> Decode for BodyDecoder<D> {
    type Item = D::Item;

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
impl<D: Decode + Default> Default for BodyDecoder<D> {
    fn default() -> Self {
        BodyDecoder::WithoutLength(D::default())
    }
}
impl<D: Decode> BodyDecode for BodyDecoder<D> {
    fn initialize(&mut self, header: &Header) -> Result<()> {
        use std::mem::{forget, replace, uninitialized};

        let inner = match *self {
            BodyDecoder::Chunked(ref mut t) => replace(t, unsafe { uninitialized() }).0,
            BodyDecoder::WithLength(ref mut _t) => {
                // TODO: replace(t, unsafe { uninitialized() }).into_inner()
                unimplemented!()
            }
            BodyDecoder::WithoutLength(ref mut t) => replace(t, unsafe { uninitialized() }),
        };
        for field in header.fields() {
            if field.name().eq_ignore_ascii_case("content-length") {
                let size: u64 = track!(
                    field
                        .value()
                        .parse()
                        .map_err(|e| ErrorKind::InvalidInput.cause(e))
                )?;
                forget(replace(self, BodyDecoder::WithLength(inner.length(size))));
                return Ok(());
            } else if field.name().eq_ignore_ascii_case("transfer-encoding") {
                track_assert_eq!(field.value(), "chunked", ErrorKind::Other);
                forget(replace(
                    self,
                    BodyDecoder::Chunked(ChunkedBodyDecoder(inner)),
                ));
                return Ok(());
            }
        }
        forget(replace(self, BodyDecoder::WithoutLength(inner)));
        Ok(())
    }
}

pub struct BodyEncoder;

// TODO: priv
pub struct ChunkedBodyEncoder;

// TODO: priv
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
