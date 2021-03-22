use crate::SH;
use crate::utils;

use kuchiki::{ElementData, NodeDataRef};

pub const TAG: SH = (r#"link[rel="shortcut icon"], link[rel="icon"], link[rel="apple-touch-icon"]"#, favicon);

// oh my... https://en.wikipedia.org/wiki/Favicon
fn favicon(node: &NodeDataRef<ElementData>) {
    retry!(node.attributes.try_borrow_mut())
        .map(|mut attr| attr.get_mut("href")
                            .map(utils::make_data_uri))
        .expect("cannot inline favicon");
}
