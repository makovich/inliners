use crate::SH;
use crate::utils;

use kuchiki::{ElementData, NodeDataRef, NodeRef};

pub const SCRIPT_TAG: SH = ("script[src]", external);
pub const LINK_TAG: SH = ("link[type='application/x-javascript'], link[type='application/javascript'], link[type='text/javascript']", external);
pub const LINK_JSON_TAG: SH = ("link[type='application/json']", external);

fn external(node: &NodeDataRef<ElementData>) {
    let tag = node.name.local.to_string();
    let attr = match tag.as_ref() {
        "link" => "href",
        "script" => "src",
        _ => return,
    };

    retry!(node.attributes.try_borrow())
        .map(|a| a.get(attr)
                  .map(utils::load_string)
                  .transpose()
                  .ok())
        .expect(&format!("cannot find `{}` attr in <{} />", attr, tag))
        .flatten()
        .map(|script| replace(node.as_node(), script));
}

fn replace(node: &NodeRef, content: String) {
    use html5ever::{interface::QualName, local_name, namespace_url, ns};

    let elm = NodeRef::new_element(
        QualName::new(None, ns!(html), local_name!("script")),
        vec![]
    );

    elm.append(NodeRef::new_text(content));

    node.insert_after(elm);
    node.detach();
}
