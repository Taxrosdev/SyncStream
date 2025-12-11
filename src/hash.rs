use xxhash_rust::xxh3::Xxh3;

pub enum Hasher {
    Blake3(Box<blake3::Hasher>),
    Xxh3(Box<Xxh3>),
}

#[derive(Default, Clone, Copy, PartialEq, Debug)]
pub enum HashKind {
    #[default]
    Blake3,
    Xxh3,
}

impl Hasher {
    pub fn new(kind: HashKind) -> Self {
        match kind {
            HashKind::Blake3 => Self::Blake3(Box::new(blake3::Hasher::new())),
            HashKind::Xxh3 => Self::Xxh3(Box::new(Xxh3::new())),
        }
    }

    pub fn update(&mut self, data: &[u8]) {
        match self {
            Hasher::Blake3(hasher) => {
                hasher.update(data);
            }
            Hasher::Xxh3(hasher) => {
                hasher.update(data);
            }
        };
    }

    pub fn finalize(self) -> String {
        match self {
            Self::Blake3(hasher) => hasher.finalize().to_hex().to_string(),
            Self::Xxh3(hasher) => hex::encode(hasher.digest().to_le_bytes()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blake3_stability() {
        let mut hasher = Hasher::new(HashKind::Blake3);

        hasher.update("Test Input".as_bytes());

        assert_eq!(
            hasher.finalize(),
            "333ea0accb9fbcab70b14d102ef9c98c9df7060a72c95b46322aa8ad09a6ec51".to_string()
        );
    }

    #[test]
    fn test_xxh3_stability() {
        let mut hasher = Hasher::new(HashKind::Xxh3);

        hasher.update("Test Input".as_bytes());

        assert_eq!(hasher.finalize(), "9ecc4f8d8238ad80".to_string());
    }
}
