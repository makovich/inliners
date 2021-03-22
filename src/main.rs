#[macro_use]
mod utils;
mod handler;

use anyhow::{Error, Result, bail};
use atty::{Stream::Stdin};
use human_panic::setup_panic;
use kuchiki::{NodeRef, NodeDataRef, ElementData, traits::*};
use once_cell::sync::Lazy;
use rayon::prelude::*;
use structopt::StructOpt;
use url::Url;

use std::env;
use std::io::{self, Read};
use std::ops::Deref;
use std::path::PathBuf;

// (css Selector, node Handler) pair
type SH<'a> = (&'a str, fn(&NodeDataRef<ElementData>));

static OPT: Lazy<Opt> = Lazy::new(Opt::from_args);

fn main() -> Result<()> {
    setup_panic!();

    setup_log!{
        filter_level: OPT.log_level(),
    };

    rayon::ThreadPoolBuilder::new()
        .num_threads(OPT.threads)
        .thread_name(|i| i.to_string())
        .build_global()?;

    let mut todo = vec![
        handler::base::TAG,
        handler::favicon::TAG,
    ];

    if OPT.js() {
        todo.extend(&[
            handler::script::SCRIPT_TAG,
            handler::script::LINK_TAG,
            handler::script::LINK_JSON_TAG,
        ]);
    }

    if OPT.css() {
        todo.extend(&[
            handler::css::INTERN,
            handler::css::EXTERN,
            handler::css::INLINE,
        ]);
    }

    if OPT.img() {
        todo.extend(&[
            handler::image::TAG,
        ]);
    }

    let html = kuchiki::parse_html().one(get_input()?);

    run(&todo, &html);
    save(html)?;

    Ok(())
}

fn run(todo: &[(&str, fn(&NodeDataRef<ElementData>))], html: &NodeRef) {
    todo.iter()
        .flat_map(|(selector, handler)| {
            html.select(selector)
                .map_or(vec![], |v| v.collect())
                .into_iter()
                .map(move |node| UnsafeWrap::new((node, handler)))
        })
        .collect::<Vec<_>>()
        .par_iter()
        .for_each(|w| {
            let (node, handler) = w.deref();
            log!(debug, "{}", utils::format_node(node));
            handler(node);
        });
}

fn get_input() -> Result<String> {
    match OPT.input {
        Some(ref url) => {
            utils::load_url(url)
                  .and_then(|(_, data)|
                        String::from_utf8(data).map_err(Error::msg))
        },
        None if atty::isnt(Stdin) => {
            let mut buf = String::new();
            io::stdin().read_to_string(&mut buf)?;
            Ok(buf)
        },
        None => bail!("No file to process provided."),
    }
}

fn save(html: NodeRef) -> io::Result<()> {
    match OPT.output {
        #[cfg(feature="minify-html")]
        Some(ref path) if OPT.minify => std::fs::write(path, &minify(html)?),

        #[cfg(feature="minify-html")]
        None if OPT.minify => io::Write::write_all(&mut io::stdout(), &minify(html)?),

        Some(ref path) => html.serialize_to_file(path),
        None => html.serialize(&mut io::stdout()),
    }
}

#[cfg(feature="minify-html")]
fn minify(html: NodeRef) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();

    html.serialize(&mut bytes)?;

    let cfg = minify_html::Cfg {
        minify_css: true,
        minify_js: true,
    };

    if let Ok(new_len) = minify_html::in_place(&mut bytes, &cfg) {
        bytes.truncate(new_len);
    } else {
        // if something went wrong - serialize again
        html.serialize(&mut bytes)?;
    }

    Ok(bytes)
}

#[derive(Debug, StructOpt)]
#[structopt(name = env!("CARGO_BIN_NAME"), about)]
struct Opt {
    /// Input file or URL (index.html, https://example.com/path/)
    #[structopt(parse(from_str = Opt::parse_url))]
    input: Option<Url>,

    /// Output file, stdout if not present
    #[structopt(short = "o", long, parse(from_os_str))]
    output: Option<PathBuf>,

    #[structopt(skip = Opt::read_cwd())]
    cwd: Url,

    /// Verbose mode (-v, -vv, -vvv)
    #[structopt(short, long, parse(from_occurrences))]
    verbose: usize,

    /// Silence all output
    #[structopt(short, long)]
    quiet: bool,

    /// Number of threads (use -j1 to turn parallelism off)
    #[structopt(short = "j", long, default_value = "40")]
    threads: usize,

    /// Do not process/embedd JavaScript
    #[structopt(short = "J", long)]
    no_js: bool,

    /// Do not process/embedd CSS stylesheets
    #[structopt(short = "C", long)]
    no_css: bool,

    /// Do not process/embedd images
    #[structopt(short = "I", long)]
    no_img: bool,

    /// Minify HTML
    #[cfg(all(feature="minify-html", not(feature="esbuild")))]
    #[structopt(short = "m", long)]
    minify: bool,

    /// Minify HTML, CSS and JavaScript
    #[cfg(all(feature="minify-html", feature="esbuild"))]
    #[structopt(short = "m", long)]
    minify: bool,
}

impl Opt {
    fn get_base(&self) -> Url {
        self.input
            .as_ref()
            .map(|u| u.join("./").unwrap_or(u.to_owned()))
            .unwrap_or(self.cwd.clone())
    }

    fn parse_url(input: &str) -> Url {
        Url::parse(input)
            .or_else(|_| Opt::read_cwd().join(input))
            .expect(&format!("Cannot parse FILE/URL: {}", input))
    }

    fn read_cwd() -> Url {
        env::current_dir()
            .map_err(|_| ())
            .and_then(Url::from_directory_path)
            .expect("Failed to read current directory")
    }

    #[allow(dead_code)]
    fn log_level(&self) -> log::LevelFilter {
        if self.quiet {
            return log::LevelFilter::Off;
        }

        match self.verbose {
            0 => log::LevelFilter::Warn,
            1 => log::LevelFilter::Info,
            2 => log::LevelFilter::Debug,
            _ => log::LevelFilter::Trace,
        }
    }

    fn js(&self) -> bool {
        !self.no_js
    }

    fn css(&self) -> bool {
        !self.no_css
    }

    fn img(&self) -> bool {
        !self.no_img
    }
}

unsafe impl<T> Send for UnsafeWrap<T> {}
unsafe impl<T> Sync for UnsafeWrap<T> {}

struct UnsafeWrap<T> {
    data: *mut T,
}

impl<T> UnsafeWrap<T> {
    fn new(data: T) -> Self {
        UnsafeWrap {
            data: Box::into_raw(Box::new(data)),
        }
    }
}

impl<T> Deref for UnsafeWrap<T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.data }
    }
}

impl<T> Drop for UnsafeWrap<T> {
    fn drop(&mut self) {
        unsafe { drop(Box::from_raw(self.data)) }
    }
}

#[cfg(test)]
mod tests;
