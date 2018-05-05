use std::fmt;
use std::mem;
use bytecodec::{ByteCount, Decode, DecodeExt, Encode, Eos, ErrorKind, ExactBytesEncode, Result};
use bytecodec::combinator::Length;
use trackable::error::ErrorKindExt;

use {Header, HeaderField, HeaderMut};
use chunked_body::{ChunkedBodyDecoder, ChunkedBodyEncoder};

/// `BodyDecode` is used for representing HTTP body decoders.
pub trait BodyDecode: Decode {
    /// This method is called before starting to decode a HTTP body.
    ///
    /// The default implementation does nothing.
    #[allow(unused_variables)]
    fn initialize(&mut self, header: &Header) -> Result<()> {
        Ok(())
    }
}
impl<'a, T: ?Sized + BodyDecode> BodyDecode for &'a mut T {
    fn initialize(&mut self, header: &Header) -> Result<()> {
        (**self).initialize(header)
    }
}
impl<T: ?Sized + BodyDecode> BodyDecode for Box<T> {
    fn initialize(&mut self, header: &Header) -> Result<()> {
        (**self).initialize(header)
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
    fn update_header(&self, header: &mut HeaderMut) -> Result<()> {
        Ok(())
    }
}
impl<'a, T: ?Sized + BodyEncode> BodyEncode for &'a mut T {
    fn update_header(&self, header: &mut HeaderMut) -> Result<()> {
        (**self).update_header(header)
    }
}
impl<T: ?Sized + BodyEncode> BodyEncode for Box<T> {
    fn update_header(&self, header: &mut HeaderMut) -> Result<()> {
        (**self).update_header(header)
    }
}

/// A body decoder that consumes no bytes.
///
/// This does consume no bytes and immediately returns `()` as the decoded item.
///
/// It is mainly intended to be used for decoding HEAD responses.
/// It can also be used to prefetch the HTTP header before decoding the body of
/// a HTTP message.
#[derive(Debug, Default)]
pub struct NoBodyDecoder;
impl Decode for NoBodyDecoder {
    type Item = ();

    fn decode(&mut self, _buf: &[u8], _eos: Eos) -> Result<(usize, Option<Self::Item>)> {
        Ok((0, Some(())))
    }

    fn is_idle(&self) -> bool {
        false
    }

    fn requiring_bytes(&self) -> ByteCount {
        ByteCount::Finite(0)
    }
}
impl BodyDecode for NoBodyDecoder {}

/// A body encoder that produces no bytes.
#[derive(Debug, Default)]
pub struct NoBodyEncoder;
impl Encode for NoBodyEncoder {
    type Item = ();

    fn encode(&mut self, _buf: &mut [u8], _eos: Eos) -> Result<usize> {
        Ok(0)
    }

    fn start_encoding(&mut self, _item: Self::Item) -> Result<()> {
        Ok(())
    }

    fn is_idle(&self) -> bool {
        true
    }

    fn requiring_bytes(&self) -> ByteCount {
        ByteCount::Finite(0)
    }
}
impl ExactBytesEncode for NoBodyEncoder {
    fn exact_requiring_bytes(&self) -> u64 {
        0
    }
}
impl BodyEncode for NoBodyEncoder {}

/// A body encoder mainly intended to be used for encoding HEAD responses.
///
/// `HeadBodyDecoder` updates HTTP header ordinally but
/// discards all data produced by the inner encoder.
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
        let mut temp_buf = [0; 1024];
        while !self.0.is_idle() {
            track!(self.0.encode(&mut temp_buf, Eos::new(false)))?;
        }
        Ok(0)
    }

    fn start_encoding(&mut self, item: Self::Item) -> Result<()> {
        track!(self.0.start_encoding(item))
    }

    fn is_idle(&self) -> bool {
        self.0.is_idle()
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
    fn update_header(&self, header: &mut HeaderMut) -> Result<()> {
        self.0.update_header(header)
    }
}

/// Basic HTTP body decoder.
///
/// It is typically used for making a body decoder from a `Decode` implementor.
#[derive(Debug, Default)]
pub struct BodyDecoder<D: Decode>(BodyDecoderInner<D>);
impl<D: Decode> BodyDecoder<D> {
    /// Makes a new `BodyDecoder` instance.
    pub fn new(inner: D) -> Self {
        BodyDecoder(BodyDecoderInner::WithoutLength(inner))
    }
}
impl<D: Decode> Decode for BodyDecoder<D> {
    type Item = D::Item;

    fn decode(&mut self, buf: &[u8], eos: Eos) -> Result<(usize, Option<Self::Item>)> {
        self.0.decode(buf, eos)
    }

    fn is_idle(&self) -> bool {
        self.0.is_idle()
    }

    fn requiring_bytes(&self) -> ByteCount {
        self.0.requiring_bytes()
    }
}
impl<D: Decode> BodyDecode for BodyDecoder<D> {
    fn initialize(&mut self, header: &Header) -> Result<()> {
        self.0.initialize(header)
    }
}

enum BodyDecoderInner<D: Decode> {
    Chunked(ChunkedBodyDecoder<D>),
    WithLength(Length<D>),
    WithoutLength(D),
    None,
}
impl<D: Decode> BodyDecoderInner<D> {
    fn update_inner<F>(&mut self, f: F) -> Result<()>
    where
        F: FnOnce(D) -> Result<Self>,
    {
        let inner = match mem::replace(self, BodyDecoderInner::None) {
            BodyDecoderInner::Chunked(x) => x.into_inner(),
            BodyDecoderInner::WithLength(x) => x.into_inner(),
            BodyDecoderInner::WithoutLength(x) => x,
            BodyDecoderInner::None => return Ok(()),
        };
        *self = f(inner)?;
        Ok(())
    }
}
impl<D: Decode> Decode for BodyDecoderInner<D> {
    type Item = D::Item;

    fn decode(&mut self, buf: &[u8], eos: Eos) -> Result<(usize, Option<Self::Item>)> {
        match *self {
            BodyDecoderInner::Chunked(ref mut d) => track!(d.decode(buf, eos)),
            BodyDecoderInner::WithLength(ref mut d) => track!(d.decode(buf, eos)),
            BodyDecoderInner::WithoutLength(ref mut d) => track!(d.decode(buf, eos)),
            BodyDecoderInner::None => track_panic!(ErrorKind::DecoderTerminated),
        }
    }

    fn is_idle(&self) -> bool {
        match *self {
            BodyDecoderInner::Chunked(ref d) => d.is_idle(),
            BodyDecoderInner::WithLength(ref d) => d.is_idle(),
            BodyDecoderInner::WithoutLength(ref d) => d.is_idle(),
            BodyDecoderInner::None => true,
        }
    }

    fn requiring_bytes(&self) -> ByteCount {
        match *self {
            BodyDecoderInner::Chunked(ref d) => d.requiring_bytes(),
            BodyDecoderInner::WithLength(ref d) => d.requiring_bytes(),
            BodyDecoderInner::WithoutLength(ref d) => d.requiring_bytes(),
            BodyDecoderInner::None => ByteCount::Finite(0),
        }
    }
}
impl<D: Decode + Default> Default for BodyDecoderInner<D> {
    fn default() -> Self {
        BodyDecoderInner::WithoutLength(D::default())
    }
}
impl<D: Decode> BodyDecode for BodyDecoderInner<D> {
    fn initialize(&mut self, header: &Header) -> Result<()> {
        self.update_inner(|inner| {
            for field in header.fields() {
                if field.name().eq_ignore_ascii_case("content-length") {
                    let size: u64 = track!(
                        field
                            .value()
                            .parse()
                            .map_err(|e| ErrorKind::InvalidInput.cause(e))
                    )?;
                    return Ok(BodyDecoderInner::WithLength(inner.length(size)));
                } else if field.name().eq_ignore_ascii_case("transfer-encoding") {
                    track_assert_eq!(field.value(), "chunked", ErrorKind::Other);
                    return Ok(BodyDecoderInner::Chunked(ChunkedBodyDecoder::new(inner)));
                }
            }
            Ok(BodyDecoderInner::WithoutLength(inner))
        })
    }
}
impl<D: Decode> fmt::Debug for BodyDecoderInner<D> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            BodyDecoderInner::Chunked(_) => write!(f, "Chunked(_)"),
            BodyDecoderInner::WithLength(_) => write!(f, "WithLength(_)"),
            BodyDecoderInner::WithoutLength(_) => write!(f, "WithoutLength(_)"),
            BodyDecoderInner::None => write!(f, "None"),
        }
    }
}

/// Basic HTTP body encoder.
///
/// It is typically used for making a body encoder from a `Encode` implementor.
///
/// If `E::requiring_bytes()` returns `ByteCount::Unknown`,
/// the chunked body transfer encoding will be used.
#[derive(Debug, Default)]
pub struct BodyEncoder<E>(BodyEncoderInner<E>);
impl<E> BodyEncoder<E> {
    /// Makes a new `BodyEncoder` instance.
    pub fn new(inner: E) -> Self {
        BodyEncoder(BodyEncoderInner::NotStarted(inner))
    }
}
impl<E: Encode> Encode for BodyEncoder<E> {
    type Item = E::Item;

    fn encode(&mut self, buf: &mut [u8], eos: Eos) -> Result<usize> {
        self.0.encode(buf, eos)
    }

    fn start_encoding(&mut self, item: Self::Item) -> Result<()> {
        self.0.start_encoding(item)
    }

    fn is_idle(&self) -> bool {
        self.0.is_idle()
    }

    fn requiring_bytes(&self) -> ByteCount {
        self.0.requiring_bytes()
    }
}
impl<E: Encode> BodyEncode for BodyEncoder<E> {
    fn update_header(&self, header: &mut HeaderMut) -> Result<()> {
        match self.0 {
            BodyEncoderInner::NotStarted(_) | BodyEncoderInner::None => {
                track_panic!(ErrorKind::Other)
            }
            BodyEncoderInner::WithLength(ref x) => {
                let n = track_assert_some!(x.requiring_bytes().to_u64(), ErrorKind::Other);
                header.add_field(HeaderField::new("Content-Length", &n.to_string())?);
                Ok(())
            }
            BodyEncoderInner::Chunked(ref x) => x.update_header(header),
        }
    }
}

#[derive(Debug)]
enum BodyEncoderInner<E> {
    NotStarted(E),
    WithLength(E),
    Chunked(ChunkedBodyEncoder<E>),
    None,
}
impl<E: Encode> Encode for BodyEncoderInner<E> {
    type Item = E::Item;

    fn encode(&mut self, buf: &mut [u8], eos: Eos) -> Result<usize> {
        match *self {
            BodyEncoderInner::NotStarted(_) => Ok(0),
            BodyEncoderInner::None => track_panic!(ErrorKind::Other),
            BodyEncoderInner::WithLength(ref mut x) => track!(x.encode(buf, eos)),
            BodyEncoderInner::Chunked(ref mut x) => track!(x.encode(buf, eos)),
        }
    }

    fn start_encoding(&mut self, item: Self::Item) -> Result<()> {
        let mut inner =
            if let BodyEncoderInner::NotStarted(x) = mem::replace(self, BodyEncoderInner::None) {
                x
            } else {
                track_panic!(ErrorKind::EncoderFull);
            };
        track!(inner.start_encoding(item))?;
        let this = match inner.requiring_bytes() {
            ByteCount::Infinite => track_panic!(ErrorKind::Other),
            ByteCount::Unknown => BodyEncoderInner::Chunked(ChunkedBodyEncoder::new(inner)),
            ByteCount::Finite(_) => BodyEncoderInner::WithLength(inner),
        };
        *self = this;
        Ok(())
    }

    fn is_idle(&self) -> bool {
        match *self {
            BodyEncoderInner::NotStarted(_) | BodyEncoderInner::None => true,
            BodyEncoderInner::WithLength(ref x) => x.is_idle(),
            BodyEncoderInner::Chunked(ref x) => x.is_idle(),
        }
    }

    fn requiring_bytes(&self) -> ByteCount {
        match *self {
            BodyEncoderInner::NotStarted(_) => ByteCount::Finite(0),
            BodyEncoderInner::WithLength(ref x) => x.requiring_bytes(),
            BodyEncoderInner::Chunked(ref x) => x.requiring_bytes(),
            BodyEncoderInner::None => ByteCount::Finite(0),
        }
    }
}
impl<E: Default> Default for BodyEncoderInner<E> {
    fn default() -> Self {
        BodyEncoderInner::NotStarted(E::default())
    }
}
