

use serde::{Serialize, Deserialize};


#[derive(PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Debug, Clone, Copy)]
pub struct u4 {
    pub value: u8,
}

pub type HexPath = Vec<u4>;

pub fn bytes_to_path(bs: &[u8]) -> HexPath {
    let mut p = HexPath::new();
    for b in bs {
        p.push(u4 {value: b / 16});
        p.push(u4 {value: b % 16})
    }
    p
}
