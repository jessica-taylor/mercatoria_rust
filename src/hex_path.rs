use serde::{Deserialize, Serialize};

#[allow(non_camel_case_types)]
/// A hexadecimal digit.
#[derive(PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Debug, Clone, Copy)]
pub struct u4 {
    pub value: u8,
}

/// A hexadecimal string.
pub type HexPath = Vec<u4>;

/// Converts a byte array to a hexadecimal string.
pub fn bytes_to_path(bs: &[u8]) -> HexPath {
    let mut p = HexPath::new();
    for b in bs {
        p.push(u4 { value: b / 16 });
        p.push(u4 { value: b % 16 })
    }
    p
}

/// Is the first vector a prefix of the second?
pub fn is_prefix<T: Eq>(pre: &[T], full: &[T]) -> bool {
    pre.len() <= full.len() && pre.iter().zip(full.iter()).all(|(x, y)| x == y)
}

/// Is the first vector a postfix of the second?
pub fn is_postfix<T: Eq>(post: &[T], full: &[T]) -> bool {
    post.len() <= full.len()
        && post
            .iter()
            .rev()
            .zip(full.iter().rev())
            .all(|(x, y)| x == y)
}
