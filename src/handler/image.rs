use crate::SH;
use crate::utils;

use kuchiki::{ElementData, NodeDataRef};

pub const TAG: SH = ("img", image);

fn image(node: &NodeDataRef<ElementData>) {
    retry!(node.attributes.try_borrow_mut())
        .map(|mut attr| attr.get_mut("src")
                            .map(utils::make_data_uri))
        .expect("cannot inline <img />");
}

