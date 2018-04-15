use bytecodec::{ByteCount, Decode, Eos};

use {ErrorKind, Result};
use util;

#[derive(Debug)]
pub struct Method<T>(pub(crate) T);

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

// #[derive(Debug, Default)]
// pub struct MethodEncoder<B>(TokenEncoder<B>);
// impl<B: AsRef<[u8]>> Encode for MethodEncoder<B> {
//     type Item = Method<B>;

//     fn encode(&mut self, buf: &mut [u8], eos: Eos) -> Result<usize> {
//         track!(self.0.encode(buf, eos))
//     }

//     fn start_encoding(&mut self, item: Self::Item) -> Result<()> {
//         track!(self.0.start_encoding(item.0))
//     }

//     fn is_idle(&self) -> bool {
//         self.0.is_idle()
//     }

//     fn requiring_bytes(&self) -> ByteCount {
//         self.0.requiring_bytes()
//     }
// }
// impl<B: AsRef<[u8]>> ExactBytesEncode for MethodEncoder<B> {
//     fn exact_requiring_bytes(&self) -> u64 {
//         self.0.exact_requiring_bytes()
//     }
// }
