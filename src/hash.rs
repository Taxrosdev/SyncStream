use xxhash_rust::xxh3::Xxh3;

pub const DEFAULT_HASH: &str = "blake3";

pub enum Hasher {
    Blake3 { hasher: Box<blake3::Hasher> },
    Xxh3 { hasher: Box<Xxh3>, len: OutputSize },
}

pub enum OutputSize {
    Size64,
    Size128,
}

impl Hasher {
    /// Returns `None` if nothing matches
    pub fn new(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "blake3" => Some(Self::Blake3 {
                hasher: Box::new(blake3::Hasher::new()),
            }),
            "xxh3_64" => Some(Self::Xxh3 {
                hasher: Box::new(Xxh3::new()),
                len: OutputSize::Size64,
            }),
            "xxh3_128" => Some(Self::Xxh3 {
                hasher: Box::new(Xxh3::new()),
                len: OutputSize::Size128,
            }),
            _ => None,
        }
    }

    pub fn new_default() -> Self {}

    pub fn update(&mut self, data: &[u8]) {
        match self {
            Hasher::Blake3 { hasher } => {
                hasher.update(data);
            }
            Hasher::Xxh3 { hasher, .. } => hasher.update(data),
        };
    }

    pub fn finalize(self) -> String {
        match self {
            Hasher::Blake3 { hasher } => hasher.finalize().to_hex().to_string(),
            Hasher::Xxh3 { hasher, len } => match len {
                OutputSize::Size64 => hex::encode(hasher.digest().to_le_bytes()),
                OutputSize::Size128 => hex::encode(hasher.digest128().to_le_bytes()),
            },
        }
    }
}
