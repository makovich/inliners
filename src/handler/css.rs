use crate::SH;
use crate::utils;

use kuchiki::{ElementData, ExpandedName, Attribute, NodeDataRef, NodeRef};
use rayon::prelude::*;
use regex::Captures;

use std::collections::{HashMap, HashSet};
use std::sync::RwLock;

regex!(static RE_URL, r#"(?x)

        url\(["']?          # url(, url(", url('
        (?P<url>[^"')]+?)   # resourse location
        ["']?\)             # closing url() bracket

"#);

regex!(static RE_IMPORT, r#"(?x)

        @import\s+          # @import
        (?:url\(["']?)?     # url(, url(", url('
        ["']{1}             # not url(), just "string" or 'string'
        (?P<url>[^"')]+)    # resourse location
        ["']{1}             # closing quote
        \)?                 # maybe closing url() bracket
        \s*                 # maybe spaces before @media or ;
        (?P<media>.*)       # @media queries
        ;                   # end;

"#);

const EMPTY: &str = "";

pub const EXTERN: SH = ("link[rel=stylesheet]", external);
pub const INTERN: SH = ("style", internal);
pub const INLINE: SH = ("[style]", inline);

fn external(node: &NodeDataRef<ElementData>) {
    retry!(node.attributes.try_borrow())
        .map(|attr| attr.get("href")
                        .map(utils::load_string)
                        .transpose()
                        .ok())
        .expect("cannot find `href` attr in <link />")
        .flatten()
        .map(|css| patch(node.as_node(), css));
}

fn internal(node: &NodeDataRef<ElementData>) {
    patch(node.as_node(), node.text_contents())
}

fn inline(node: &NodeDataRef<ElementData>) {
    retry!(node.attributes.try_borrow_mut())
        .map(|mut a| a.get_mut("style")
                      .map(patch_url))
        .expect("cannot find `href` attr in <link />");
}

fn patch(node: &NodeRef, mut content: String) {
    use html5ever::{interface::QualName, local_name, namespace_url, ns};

    log!(info, "looking for @import's");
    patch_import(&mut content);

    log!(info, "looking for url()'s");
    patch_url(&mut content);

    let elm = NodeRef::new_element(
        QualName::new(None, ns!(html), local_name!("style")),
        vec![(
            ExpandedName::new("", "type"),
            Attribute {
                prefix: None,
                value: "text/css".to_owned(),
            },
        )]);

    elm.append(NodeRef::new_text(content));

    node.insert_after(elm);
    node.detach();
}

fn patch_import(css: &mut String) {
    let map = RwLock::new(HashMap::new());

    RE_IMPORT
        .captures_iter(&css)
        .for_each(|v| {
            log::info!("patch_import() {:?}", v.name("url"));
        });

    // Deduplicate @import URLs, download in parallel and make lookup table "url => content"
    RE_IMPORT
        .captures_iter(&css)
        .filter_map(|cap| cap.name("url").map(Into::into))
        .collect::<HashSet<&str>>()
        .par_iter()
        .for_each(|&url| {
            log!(debug, "patch_import() downloading {}", url);
            if let Ok(content) = utils::load_string(url) {
                let mut map = map.write().expect("cannot reach shared HashMap out");
                map.insert(url, content);
            }
        });

    let urls = map.into_inner().expect("cannot unwrap RwLock");
    log!(trace, "patch_import()\n{:#?}", urls);

    let patched = RE_IMPORT
        .replace_all(&css, |cap: &Captures| {

            urls.get(&cap["url"])
                .map(|content| {
                    let data = cap.name("media")
                                  .map(|s| s.as_str())
                                  .filter(|&s| s != EMPTY)
                                  .map_or(content.to_owned(), |media_query| {
                                      format!("@media {} {{\n{}\n}}", media_query, content.to_owned())
                                  });

                    log!(trace, "replacing @import '{}' with\n{}", &cap["url"], data);

                    data
                })
                .unwrap_or_else(|| {
                    log!(debug, "leaving @import as is {}", &cap[0]);
                    cap[0].to_owned()
                })
    });

    log!(trace, "patch_import()\n{}", patched);
    *css = patched.into_owned();
}

fn patch_url(css: &mut String) {
    let map = RwLock::new(HashMap::new());

    // Deduplicate URLs, download in parallel and make lookup table "url => data_uri"
    RE_URL
        .captures_iter(&css)
        .filter_map(|cap| cap.name("url").map(Into::into))
        .collect::<HashSet<&str>>()
        .par_iter()
        .for_each(|&url| {
            let mut new_url = url.to_owned();
            utils::make_data_uri(&mut new_url);
            let mut map = map.write().expect("cannot reach shared HashMap");
            map.insert(url, format!("url({})", new_url));
        });

    let urls = map.into_inner().expect("cannot unwrap RwLock");
    log!(trace, "patch_url()\n{:#?}", urls);

    let patched = RE_URL
        .replace_all(&css, |cap: &Captures| {

            urls.get(&cap["url"])
                .map(ToOwned::to_owned)
                .map(|v| {
                    log!(debug, "making datauri for {}", &cap[0]);
                    v
                })
                .unwrap_or_else(|| {
                    log!(debug, "skipping {}", &cap[0]);
                    format!("url({})", &cap["url"])
                })
    });

    log!(trace, "patch_url()\n{}", patched);
    *css = patched.into_owned();
}

#[cfg(test)]
mod tests {
    use super::*;

    const CSS: &str = r#"
    @import url("fineprint.css") print;
    @import url("bluish.css") projection, tv;
    @import 'custom.css';
    @import url("chrome://communicator/skin/");
    @import "common.css" screen, projection;
    @import url('landscape.css') screen and (orientation:landscape);

    @font-face {
      font-family: header;
      src: url("chava.ttf") format("truetype");
    }

    @font-face {
        font-family: body;
        src: url("poiretone.ttf") format("truetype");
    }

    @media screen {
        .test {
            background:url("test.jpg") repeat-y;
        }
    }

    .grass {
        background-image:url(green.png);
    }

    .logo {
        background: center / contain no-repeat url("../../media/examples/firefox-logo.svg"),
                    #eee 35% url("../../media/examples/lizard.png");
    }
    "#;

    #[allow(dead_code)]
    fn do_log() {
        env_logger::builder().is_test(true).init();
    }

    #[test]
    fn regex_import() {
        let expected = [
            ("fineprint.css", "print"),
            ("bluish.css", "projection, tv"),
            ("custom.css", ""),
            ("chrome://communicator/skin/", ""),
            ("common.css", "screen, projection"),
            ("landscape.css", "screen and (orientation:landscape)"),
        ];

        RE_IMPORT
            .captures_iter(&CSS)
            .enumerate()
            .for_each(|(i, c)| {
                let url = c.name("url").map_or("", |m| m.as_str());
                let media = c.name("media").map_or("", |m| m.as_str());
                assert_eq!(expected[i], (url, media));
            });
    }

    #[test]
    fn regex_url() {
        let expected = [
            // URLs from @import directive
            ("fineprint.css"),
            ("bluish.css"),
            ("chrome://communicator/skin/"),
            ("landscape.css"),

            // from the rest of the CSS
            ("chava.ttf"),
            ("poiretone.ttf"),
            ("test.jpg"),
            ("green.png"),
            ("../../media/examples/firefox-logo.svg"),
            ("../../media/examples/lizard.png"),
        ];

        RE_URL
            .captures_iter(&CSS)
            .enumerate()
            .for_each(|(i, c)| {
                assert_eq!(expected[i], (c.get(1).map_or("", |m| m.as_str())));
            })
    }
}
