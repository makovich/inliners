use super::*;

use std::env;
use std::fs::File;
use std::path::Path;
use std::sync::Once;

const TESTDATA_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/test_data");

static PNG: Lazy<String> = Lazy::new(|| format!("data:image/png;base64,{}",    base64::encode(read_bytes("img/i.png"))));
static BMP: Lazy<String> = Lazy::new(|| format!("data:image/bmp;base64,{}",    base64::encode(read_bytes("img/i.bmp"))));
static TIF: Lazy<String> = Lazy::new(|| format!("data:image/tiff;base64,{}",   base64::encode(read_bytes("img/i.tif"))));
static JPG: Lazy<String> = Lazy::new(|| format!("data:image/jpeg;base64,{}",   base64::encode(read_bytes("img/i.jpg"))));
static GIF: Lazy<String> = Lazy::new(|| format!("data:image/gif;base64,{}",    base64::encode(read_bytes("img/i.gif"))));
static ICO: Lazy<String> = Lazy::new(|| format!("data:image/x-icon;base64,{}", base64::encode(read_bytes("favicon.ico"))));

const NUM_OF_THREADS: usize = 1;

static SETUP: Once = Once::new();

fn setup() {
    SETUP.call_once(|| {
        let _ = env_logger::builder().is_test(true).try_init();

        env::set_current_dir(TESTDATA_PATH).expect("cannot cd");

        rayon::ThreadPoolBuilder::new()
            .num_threads(NUM_OF_THREADS)
            .thread_name(|i| i.to_string())
            .build_global()
            .expect("rayon cannot be initialized");
    });
}

fn test(handlers: &[SH], inline: &str, expect: &str) {
    setup();

    let inline = kuchiki::parse_html().one(inline);
    let expect = kuchiki::parse_html().one(expect);


    run(handlers, &inline);
    assert_eq!(inline.to_string(), expect.to_string());
}

#[allow(dead_code)]
#[cfg(feature="minify-html")]
fn test_with_minify(handlers: &[SH], inline: &str, expect: &str) {
    setup();

    let inline = kuchiki::parse_html().one(inline);

    run(handlers, &inline);

    let min = minify(inline).map_or(String::new(), |b| unsafe {
        String::from_utf8_unchecked(b)
    });

    assert_eq!(min, expect);
}

#[test]
#[cfg(all(feature="minify-html", feature="esbuild"))]
fn full_with_minify() {
    test_with_minify(
        &[
            handler::base::TAG,
            handler::favicon::TAG,
            handler::css::EXTERN,
            handler::css::INLINE,
            handler::css::INTERN,
            handler::script::LINK_JSON_TAG,
            handler::script::LINK_TAG,
            handler::script::SCRIPT_TAG,
            handler::image::TAG,
        ],
        &read_string("test.html"),
        &read_string("test.min.html"),
    );
}

#[test]
fn favicon_embed() {
    test(
        &[
            handler::favicon::TAG
        ],

        r#"
            <html lang="en">
              <head>
                <link rel="shortcut icon" href="favicon.ico">
              </head>
              <body>
              </body>
            </html>
        "#,

        &format!(r#"
            <html lang="en">
              <head>
                <link rel="shortcut icon" href="{}">
              </head>
              <body>
              </body>
            </html>
        "#, *ICO)
    );
}

#[test]
fn no_favicon_leave_url() {
    let html = r#"
            <html lang="en">
              <head>
                <link rel="shortcut icon" href="no-icon.gif">
              </head>
              <body>
              </body>
            </html>
    "#;

    test(
        &[ handler::favicon::TAG ],
        html,
        html
    );
}

#[test]
fn inline_img_tag() {
    test(
        &[ handler::image::TAG ],
        r#"<img src="img/i.png" class="bip">"#,
        &format!(r#"<html><head><img src="{}" class="bip"></head><body></body></html>"#, *PNG),
    );
}

#[test]
fn inline_css() {
    test(
        &[
            handler::css::EXTERN,
            handler::css::INLINE,
            handler::css::INTERN,
        ],
        r#"
            <link href="assets/a/001.css" rel="stylesheet">
            <style>
                @import 'assets/a/002.css';
                body { backgrouond-image: url("img/i.tif"); }
            </style>
            <p style="background: url("img/i.jpg")"
        "#,
        &format!(r#"
            <style type="text/css">p {{
    color: red;
}}
</style>
            <style type="text/css">
                p {{
    background-image: url({});
}}
body {{
    background-image: url(not-exists.img);
    background-image: url({});
}}

                body {{ backgrouond-image: url({}); }}
            </style>
            <p style="background: url({})"
        "#, *BMP, *GIF, *TIF, *JPG),
    );
}

#[allow(dead_code)]
fn read_bytes<P: AsRef<Path>>(path: P) -> Vec<u8> {
    let mut f = File::open(Path::new(TESTDATA_PATH).join(path)).unwrap();
    let mut b = Vec::new();
    f.read_to_end(&mut b).unwrap();
    b
}

#[allow(dead_code)]
fn read_string<P: AsRef<Path>>(path: P) -> String {
    unsafe {
        let mut s = String::from_utf8_unchecked(read_bytes(path));
        let l = s.trim_end().len();
        s.truncate(l);
        s
    }
}
