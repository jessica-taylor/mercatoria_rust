

use serde::{Serialize, Deserialize};


#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct u4 {
    pub value: u8,
}

pub type Path = Vec<u4>;

pub fn bytes_to_path(bs: &[u8]) -> Path {
    let mut p = Path::new();
    for b in bs {
        p.push(u4 {value: b / 16});
        p.push(u4 {value: b % 16})
    }
    p
}
