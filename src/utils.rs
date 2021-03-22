use crate::OPT;

use anyhow::{Result, anyhow, bail};
use once_cell::unsync::Lazy;
use kuchiki::{NodeDataRef, ElementData};
use url::Url;

use std::cell::Cell;
use std::fs;
use std::sync::atomic::{AtomicUsize, Ordering};

const OCTET_STREAM: &str = "application/octet-stream";

// global counter
pub(crate) static GLOB_JOB: AtomicUsize = AtomicUsize::new(1);

// per thread counter
thread_local!(
    pub(crate) static JOB: Lazy<Cell<usize>> = Lazy::new(|| {
        Cell::new(GLOB_JOB.fetch_add(1, Ordering::SeqCst))
    })
);

macro_rules! log {
    ($lvl:ident, $($arg:tt)*) => {
        crate::utils::JOB.with(|f| {
            ::log::$lvl!("[ THREAD #{} / JOB #{} ] {}",
                   ::std::thread::current().name().unwrap_or("-"),
                   f.get(),
                   ::std::format_args!($($arg)*));
        });
    };
}

macro_rules! setup_log {
    () => {
        setup_log!{
            filter_level: ::log::LevelFilter::Warn,
            format_level: false,
            format_timestamp: None,
            format_module_path: false,
        }
    };

    ( $($fn:ident: $val:expr $(,)?)* ) => {
        #[cfg(debug_assertions)]
        ::env_logger::init();

        #[cfg(not(debug_assertions))]
        {
            let mut builder = ::env_logger::Builder::new();
            $(
                builder.$fn($val);
             )*

            builder.parse_default_env();

            if ::std::env::var(::env_logger::DEFAULT_FILTER_ENV).is_ok() {
                builder.default_format();
            }

            builder.init();
        }
    };
}

macro_rules! retry {
    ($op:expr) => {
        ::retry::retry_with_index(::retry::delay::Exponential::from_millis(5).take(20), |att| {
            if att > 1 {
                log!(warn, "attempt #{}", att);
            }
            $op
        })
    };
}

macro_rules! regex {
    (static $name:ident, $re:literal) => {
        static $name: ::once_cell::sync::Lazy<::regex::Regex> = ::once_cell::sync::Lazy::new(|| {
            ::regex::Regex::new($re)
                .unwrap_or_else(|_| ::std::panic!("cannot initialize {}", ::std::stringify!($name)))
        });
    }
}

pub fn load_url(url: &Url) -> Result<(String, Vec<u8>)> {
    match url.scheme() {
        "file" => {
            log!(info, "reading file://{}", url.path());

            let path = url.to_file_path()
                          .map_err(|_| anyhow!("cannot get path"))?;

            let data = fs::read(&path)?;

            let mime = guess_mime(&data)
                          .or_else(|| path.extension()
                                          .and_then(|v| v.to_str())
                          .and_then(guess_mime_by_ext))
                          .unwrap_or(OCTET_STREAM.to_owned());

            Ok((mime, data))
        }
        "http" | "https" => {
            log!(info, "requesting {}", url.as_str());

            let resp = minreq::get(url.as_ref()).send()?;

            if resp.status_code != 200 {
                bail!("Response status code: {}", resp.status_code);
            }

            let mime = resp.headers.get("content-type")
                                   .map(ToOwned::to_owned)
                                   .or_else(|| guess_mime(resp.as_bytes()))
                                   .unwrap_or(OCTET_STREAM.to_owned());

            let data = resp.into_bytes();

            Ok((mime, data))
        }
        _ => Err(anyhow!("not supported URL scheme"))
    }
}

pub fn load_file(href: &str) -> Result<(String, Vec<u8>)> {
    Url::parse(href)
        .or_else(|_| OPT.get_base().join(href))
        .map(|u| load_url(&u))?
}

pub fn load_string(href: &str) -> Result<String> {
    load_file(href).and_then(|(_, content)| {
        String::from_utf8(content)
            .map_err(anyhow::Error::msg)
    })
}

fn guess_mime(data: &[u8]) -> Option<String> {
    let mime = match data {
        &[0x47, 0x49, 0x46, 0x38, 0x37, 0x61, ..] |
        &[0x47, 0x49, 0x46, 0x38, 0x39, 0x61, ..] => "image/gif",
        &[0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, ..] => "image/png",
        &[0x00, 0x00, 0x01, 0x00, ..] => "image/x-icon",
        &[0xFF, 0xD8, 0xFF, ..] => "image/jpeg",
        &[0x42, 0x4D, ..] => "image/bmp",
        &[0x49, 0x49, 0x2A, 0x00, ..] |
        &[0x4D, 0x4D, 0x00, 0x2A, ..] => "image/tiff",

        _ => return None,
    };

    log!(debug, "magic guess {:?} -> {}",
               &data[0..=6]
                   .iter()
                   .map(|v| format!("{:02X}", v))
                   .collect::<Vec<_>>()
                   .join(" "),
               mime);

    Some(mime.to_owned())
}

fn guess_mime_by_ext(ext: &str) -> Option<String> {
    let mime = match ext {
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "woff" => "font/woff",
        "woff2" => "font/woff2",

        _ => mime_guess::from_ext(ext)
                        .first_raw()
                        .unwrap_or(OCTET_STREAM),
    };

    log!(debug, "extension guess {:?} -> {}", ext, mime);

    Some(mime.to_owned())
}

pub fn make_data_uri(url: &mut String) {
    if let Ok((mime, data)) = load_file(url) {
        *url = format!("data:{};base64,{}", mime, base64::encode(data));
    }
}

#[allow(dead_code)]
pub fn make_data_uri_with_mime(mime: &str) -> impl Fn(&mut String) + '_ {
    move |url| if let Ok((_, data)) = load_file(url) {
        *url = format!("data:{};base64,{}", mime, base64::encode(data));
    }
}

pub fn format_node(node: &NodeDataRef<ElementData>) -> String {
    format!("<{} {} />",

            node.name
                .local
                .to_string(),

            retry!(node.attributes.try_borrow())
                .unwrap()
                .map
                .iter()
                .map(|(k,v)| format!("{}=\"{}\"", k.local, v.value))
                .collect::<Vec<_>>()
                .as_slice()
                .join(" "))
}
