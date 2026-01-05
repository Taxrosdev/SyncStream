//! For things that are instant that shouldn't always be instant.
//! An example is `fs::read_to_end` vs `fs::read`
//!
//! This should only be used in tests or in the rare few cases where this may be applicable.

/// Not recommended outside of tests, as loads entire file into memory.
#[cfg(test)]
pub async fn read_to_end<P: AsRef<std::path::Path>>(path: P) -> Result<Vec<u8>, std::io::Error> {
    #[cfg(feature = "tokio")]
    let data = tokio::fs::read(path).await?;
    #[cfg(not(feature = "tokio"))]
    let data = std::fs::read(path)?;

    Ok(data)
}
