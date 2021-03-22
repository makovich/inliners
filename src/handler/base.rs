use crate::SH;
use crate::OPT;

use html5ever::{interface::QualName, local_name, namespace_url, ns};
use kuchiki::{Attribute, ElementData, ExpandedName, NodeDataRef, NodeRef};

pub const TAG: SH = (r#"head"#, base_href);

fn base_href(node: &NodeDataRef<ElementData>) {
    if let Ok(tag) = node.as_node().select_first("base[href]") {
        log!(debug, "{} found; skipping", tag.as_node().to_string());
        return
    }

    match OPT.get_base().scheme() {
        "https" |
        "http" => {
            let elm = NodeRef::new_element(
                QualName::new(None, ns!(html), local_name!("base")),
                vec![(
                    ExpandedName::new("", "href"),
                    Attribute {
                        prefix: None,
                        value: OPT.get_base().to_string(),
                    },
                )]);

            log!(debug, "appending {}", elm.to_string());

            node.as_node().append(elm);
        }
        _ => return
    }
}
