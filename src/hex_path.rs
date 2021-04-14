use serde::{Deserialize, Serialize};
use std::convert::AsRef;
use std::fmt;
use std::ops::Index;
use std::slice::SliceIndex;

#[allow(non_camel_case_types)]
/// A hexadecimal digit.
#[derive(PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Debug, Clone, Copy)]
pub struct u4(pub u8);

impl fmt::Display for u4 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A hexadecimal string.
#[derive(PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Debug, Clone)]
pub struct HexPath(pub Vec<u4>);

impl fmt::Display for HexPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for digit in self.iter() {
            if digit.0 < 10 {
                write!(f, "{}", (('0' as u8) + digit.0) as char)?;
            } else {
                write!(f, "{}", (('A' as u8) + (digit.0 - 10)) as char)?;
            }
        }
        Ok(())
    }
}

impl AsRef<Vec<u4>> for HexPath {
    fn as_ref(&self) -> &Vec<u4> {
        &self.0
    }
}

impl HexPath {
    pub fn iter(&self) -> impl Iterator<Item = &u4> {
        self.0.iter()
    }
    pub fn into_iter(self) -> impl Iterator<Item = u4> {
        self.0.into_iter()
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl AsRef<[u4]> for HexPath {
    fn as_ref(&self) -> &[u4] {
        self.0.as_ref()
    }
}

impl<Idx: SliceIndex<[u4]>> Index<Idx> for HexPath {
    type Output = Idx::Output;
    fn index(&self, index: Idx) -> &Self::Output {
        &self.0[index]
    }
}

/// Converts a byte array to a hexadecimal string.
pub fn bytes_to_path(bs: &[u8]) -> HexPath {
    let mut p = Vec::new();
    for b in bs {
        p.push(u4(b / 16));
        p.push(u4(b % 16))
    }
    HexPath(p)
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
