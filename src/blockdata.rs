use crate::crypto::HashCode;

pub struct MainBlockBody {
    prev: Option<HashCode>,
    version: u64,
    timestamp_ms: i64,
    tree: Option<HashCode>,
    options: HashCode,
    // signer slashes
    // miner slashes
}
