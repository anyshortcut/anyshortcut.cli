use curl;
use std;
use std::io::{Read, Write};
use std::cell::{RefCell, RefMut};
use std::fmt;

/// Shortcut alias for results of this module.
pub type Result<T> = std::result::Result<T, Error>;

#[derive(PartialEq, Debug)]
pub enum Method {
    Get,
    Head,
    Post,
    Put,
    Delete,
}

impl fmt::Display for Method {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Method::Get => write!(f, "GET"),
            Method::Head => write!(f, "HEAD"),
            Method::Post => write!(f, "POST"),
            Method::Put => write!(f, "PUT"),
            Method::Delete => write!(f, "DELETE"),
        }
    }
}

///
/// A Http client base on curl.
/// 
pub struct Client {
    shared_handle: RefCell<curl::easy::Easy>,
    base_url: String,
    token: String,
}

impl Client {
    pub fn new(base_url: &str, token: &str) -> Client {
        Client {
            shared_handle: RefCell::new(curl::easy::Easy::new()),
            base_url: base_url.to_string(),
            token: token.to_string(),
        }
    }

    fn request(&self, endpoint: &str, method: Method) -> Result<Request> {
        let url = format!("{}{}", self.base_url, endpoint);
        let mut handle = self.shared_handle.borrow_mut();
        handle.reset();
        Request::new(handle, method, &url)
    }

    pub fn get(&self, endpoint: &str) -> Result<Response> {
        self.request(endpoint, Method::Get)?.send()
    }
}

pub struct Request<'a> {
    handle: RefMut<'a, curl::easy::Easy>,
    headers: curl::easy::List,
    body: Option<Vec<u8>>,
}

impl<'a> Request<'a> {
    pub fn new(
        mut handle: RefMut<'a, curl::easy::Easy>,
        method: Method,
        url: &str,
    ) -> Result<Request<'a>> {
        let mut headers = curl::easy::List::new();
        headers.append(&format!("User-Agent: anyshortcut-cli/{}", "0.0.1")).ok();

        match method {
            Method::Get => handle.get(true)?,
            Method::Head => {
                handle.get(true)?;
                handle.custom_request("HEAD")?;
                handle.nobody(true)?;
            }
            Method::Post => handle.custom_request("POST")?,
            Method::Put => handle.custom_request("PUT")?,
            Method::Delete => handle.custom_request("DELETE")?,
        }

        handle.url(url)?;

        Ok(Request {
            handle,
            headers,
            body: None,
        })
    }

    pub fn with_header(mut self, key: &str, value: &str) -> Result<Request<'a>> {
        self.headers.append(&format!("{}: {}", key, value))?;
        Ok(self)
    }

    /// Sends the request and reads the response body into the response object.
    pub fn send(mut self) -> Result<Response> {
        self.handle.verbose(true)?;
        self.handle.http_headers(self.headers)?;

        match self.body {
            Some(ref body) => {
                let mut body: &[u8] = &body[..];
                self.handle.upload(true)?;
                self.handle.in_filesize(body.len() as u64)?;
                handle_request(&mut self.handle, &mut |buffer| {
                    body.read(buffer).unwrap_or(0)
                })
            }
            None => handle_request(&mut self.handle, &mut |_| 0)
        }
    }
}

fn handle_request(
    handle: &mut curl::easy::Easy,
    read: &mut FnMut(&mut [u8]) -> usize) -> Result<Response> {
    let mut response_body = vec![];
    let mut response_headers = vec![];

    {
        let mut handle = handle.transfer();

        handle.read_function(move |buffer| Ok(read(buffer)))?;

        handle.write_function(|data| {
            Ok(match response_body.write_all(data) {
                Ok(_) => data.len(),
                Err(_) => 0,
            })
        })?;

        handle.header_function(|data| {
            response_headers.push(String::from_utf8_lossy(data).into_owned());
            true
        })?;
        handle.perform()?;
    }

    Ok(Response {
        status: handle.response_code()?,
        headers: response_headers,
        body: Some(response_body),
    })
}

pub type HttpStatus = u32;

#[derive(Clone, Debug)]
pub struct Response {
    status: HttpStatus,
    headers: Vec<String>,
    body: Option<Vec<u8>>,
}

impl Response {
    pub fn status(&self) -> HttpStatus {
        self.status
    }

    pub fn failed(&self) -> bool {
        self.status >= 400 && self.status <= 600
    }

    pub fn ok(&self) -> bool {
        !self.failed()
    }
}

#[derive(Debug)]
pub struct Error {}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum ErrorKind {
    InvalidToken,
    RequestFailed,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt("Http error", f)
    }
}

impl From<curl::Error> for Error {
    fn from(error: curl::Error) -> Error {
        Error {}
    }
}