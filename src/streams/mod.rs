pub mod chunk;
mod stream;

pub use stream::*;

use crate::types::Stream;

fn get_filename(stream: &Stream) -> String {
    format!("{}{}", stream.hash, &stream.permission.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_filename_stability() {
        let stream = Stream {
            chunks: Vec::new(),
            hash: "abc".into(),
            permission: 123,
            path: "".into(),
        };

        let filename = get_filename(&stream);

        assert_eq!(filename, "abc123")
    }
}
