use std::convert::TryFrom;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

use http::{HeaderName, HeaderValue, Method, Request, Response, Uri, Version};

use crate::body::Body;
use crate::send_body::AsSendBody;
use crate::transport::time::Instant;
use crate::util::private::Private;
use crate::{Agent, Error, SendBody};

/// Transparent wrapper around [`http::request::Builder`].
///
/// The purpose is to provide the [`.call()`][RequestBuilder::call] and [`.send()`][RequestBuilder::send]
/// functions to make a simpler API for sending requests.
#[derive(Debug)]
pub struct RequestBuilder<B> {
    agent: Agent,
    builder: http::request::Builder,
    _ph: PhantomData<B>,
}

pub struct WithoutBody(());
impl Private for WithoutBody {}

pub struct WithBody(());
impl Private for WithBody {}

impl<Any> RequestBuilder<Any> {
    /// Appends a header to this request builder.
    ///
    /// This function will append the provided key/value as a header to the
    /// set of headers. It does not replace headers.
    ///
    /// # Examples
    ///
    /// ```
    /// let req = ureq::get("https://httpbin.org/get")
    ///     .header("X-Custom-Foo", "bar");
    /// ```
    pub fn header<K, V>(mut self, key: K, value: V) -> Self
    where
        HeaderName: TryFrom<K>,
        <HeaderName as TryFrom<K>>::Error: Into<http::Error>,
        HeaderValue: TryFrom<V>,
        <HeaderValue as TryFrom<V>>::Error: Into<http::Error>,
    {
        self.builder = self.builder.header(key, value);
        self
    }

    /// Overrides the URI for this request.
    ///
    /// Typically this is set via `ureq::get(<uri>)` or `Agent::get(<uri>)`. This
    /// lets us change it.
    ///
    /// # Examples
    ///
    /// ```
    /// let req = ureq::get("https://www.google.com/")
    ///     .uri("https://httpbin.org/get");
    /// ```
    pub fn uri<T>(mut self, uri: T) -> Self
    where
        Uri: TryFrom<T>,
        <Uri as TryFrom<T>>::Error: Into<http::Error>,
    {
        self.builder = self.builder.uri(uri);
        self
    }

    /// Set the HTTP version for this request.
    ///
    /// By default this is HTTP/1.1.
    /// ureq only handles HTTP/1.1 and HTTP/1.0.
    ///
    /// # Examples
    ///
    /// ```
    /// use ureq::http::Version;
    ///
    /// let req = ureq::get("https://www.google.com/")
    ///     .version(Version::HTTP_10);
    /// ```
    pub fn version(mut self, version: Version) -> Self {
        self.builder = self.builder.version(version);
        self
    }
}

impl RequestBuilder<WithoutBody> {
    pub(crate) fn new<T>(agent: Agent, method: Method, uri: T) -> Self
    where
        Uri: TryFrom<T>,
        <Uri as TryFrom<T>>::Error: Into<http::Error>,
    {
        Self {
            agent,
            builder: Request::builder().method(method).uri(uri),
            _ph: PhantomData,
        }
    }

    /// Sends the request and blocks the caller until we receive a response.
    ///
    /// It sends neither `Content-Length` nor `Transfer-Encoding`.
    ///
    /// ```
    /// let res = ureq::get("http://httpbin.org/get")
    ///     .call()?;
    /// # Ok::<_, ureq::Error>(())
    /// ```
    pub fn call(self) -> Result<Response<Body>, Error> {
        let request = self.builder.body(())?;
        do_call(self.agent, request, SendBody::none())
    }
}

impl RequestBuilder<WithBody> {
    pub(crate) fn new<T>(agent: Agent, method: Method, uri: T) -> Self
    where
        Uri: TryFrom<T>,
        <Uri as TryFrom<T>>::Error: Into<http::Error>,
    {
        Self {
            agent,
            builder: Request::builder().method(method).uri(uri),
            _ph: PhantomData,
        }
    }

    /// Set the content-type header.
    ///
    /// ```
    /// let res = ureq::post("http://httpbin.org/post")
    ///     .content_type("text/html; charset=utf-8")
    ///     .send("<html><body>åäö</body></html>")?;
    /// # Ok::<_, ureq::Error>(())
    /// ```
    pub fn content_type<V>(mut self, content_type: V) -> Self
    where
        HeaderValue: TryFrom<V>,
        <HeaderValue as TryFrom<V>>::Error: Into<http::Error>,
    {
        self.builder = self.builder.header("content-type", content_type);
        self
    }

    /// Send body data and blocks the caller until we receive response.
    ///
    /// ```
    /// let res = ureq::post("http://httpbin.org/post")
    ///     .send(&[0_u8; 1000])?;
    /// # Ok::<_, ureq::Error>(())
    /// ```
    pub fn send(self, data: impl AsSendBody) -> Result<Response<Body>, Error> {
        let request = self.builder.body(())?;
        let mut data_ref = data;
        do_call(self.agent, request, data_ref.as_body())
    }

    /// Send body data as JSON.
    ///
    /// Requires the **json** feature.
    ///
    /// The data typically derives [`Serialize`](serde::Serialize) and is converted
    /// to a string before sending (does allocate).
    ///
    /// ```
    /// use serde::Serialize;
    ///
    /// #[derive(Serialize)]
    /// struct MyData {
    ///     thing: String,
    /// }
    ///
    /// let body = MyData {
    ///     thing: "yo".to_string(),
    /// };
    ///
    /// let res = ureq::post("http://httpbin.org/post")
    ///     .send_json(&body)?;
    /// # Ok::<_, ureq::Error>(())
    /// ```
    #[cfg(feature = "json")]
    pub fn send_json(self, data: impl serde::ser::Serialize) -> Result<Response<Body>, Error> {
        let request = self.builder.body(())?;
        let body = SendBody::from_json(&data)?;
        do_call(self.agent, request, body)
    }
}

fn do_call(agent: Agent, request: Request<()>, body: SendBody) -> Result<Response<Body>, Error> {
    let response = agent.do_run(request, body, Instant::now)?;
    Ok(response)
}

impl<MethodLimit> Deref for RequestBuilder<MethodLimit> {
    type Target = http::request::Builder;

    fn deref(&self) -> &Self::Target {
        &self.builder
    }
}

impl<MethodLimit> DerefMut for RequestBuilder<MethodLimit> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.builder
    }
}

// TODO(martin): implement reasonable Debug

#[cfg(test)]
mod test {

    use super::*;

    #[test]
    fn disallow_empty_host() {
        let err = crate::get("file:///some/path").call().unwrap_err();
        assert_eq!(err.to_string(), "http: invalid format");
        assert!(matches!(err, Error::Http(_)));
    }
}
