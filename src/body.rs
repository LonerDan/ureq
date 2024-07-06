use std::fs::File;
use std::io::{self, Read, Stdin};
use std::net::TcpStream;

pub struct Body<'a> {
    inner: BodyInner<'a>,
    ended: bool,
}

impl<'a> Body<'a> {
    pub fn empty() -> Body<'static> {
        BodyInner::ByteSlice(&[]).into()
    }

    pub fn from_reader(reader: &'a mut dyn Read) -> Body<'a> {
        BodyInner::Reader(reader).into()
    }

    pub fn from_owned_reader(reader: impl Read + 'static) -> Body<'static> {
        BodyInner::OwnedReader(Box::new(reader)).into()
    }
}

mod private {
    pub trait Private {}
}
use http::Response;
use private::Private;

pub trait AsBody: Private {
    #[doc(hidden)]
    fn as_body(&mut self) -> Body;
}

pub(crate) enum BodyInner<'a> {
    ByteSlice(&'a [u8]),
    Reader(&'a mut dyn Read),
    OwnedReader(Box<dyn Read>),
}

macro_rules! impl_into_body_slice {
    ($t:ty) => {
        impl Private for $t {}
        impl AsBody for $t {
            fn as_body(&mut self) -> Body {
                BodyInner::ByteSlice((*self).as_ref()).into()
            }
        }
    };
}

impl_into_body_slice!(&[u8]);
impl_into_body_slice!(&str);
impl_into_body_slice!(String);
impl_into_body_slice!(Vec<u8>);
impl_into_body_slice!(&String);
impl_into_body_slice!(&Vec<u8>);

macro_rules! impl_into_body {
    ($t:ty, $s:tt) => {
        impl Private for $t {}
        impl AsBody for $t {
            fn as_body(&mut self) -> Body {
                BodyInner::$s(self).into()
            }
        }
    };
}

impl_into_body!(&File, Reader);
impl_into_body!(&TcpStream, Reader);
impl_into_body!(&Stdin, Reader);
impl_into_body!(File, Reader);
impl_into_body!(TcpStream, Reader);
impl_into_body!(Stdin, Reader);

#[cfg(target_family = "unix")]
use std::os::unix::net::UnixStream;

#[cfg(target_family = "unix")]
impl_into_body!(UnixStream, Reader);

pub struct RecvBody;

impl Read for RecvBody {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        todo!()
    }
}

impl<'a> From<BodyInner<'a>> for Body<'a> {
    fn from(inner: BodyInner<'a>) -> Self {
        Body {
            inner,
            ended: false,
        }
    }
}

impl_into_body!(RecvBody, Reader);

impl Private for Response<RecvBody> {}
impl AsBody for Response<RecvBody> {
    fn as_body(&mut self) -> Body {
        BodyInner::Reader(self.body_mut()).into()
    }
}
