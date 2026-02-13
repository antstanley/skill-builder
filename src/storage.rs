//! Storage operations trait for S3 and filesystem backends.

use anyhow::Result;

/// Trait for storage operations, enabling S3, filesystem, and mock implementations.
pub trait StorageOperations {
    fn put_object(&self, key: &str, data: &[u8]) -> Result<()>;
    fn get_object(&self, key: &str) -> Result<Vec<u8>>;
    fn delete_object(&self, key: &str) -> Result<()>;
    fn list_objects(&self, prefix: &str) -> Result<Vec<String>>;
    fn object_exists(&self, key: &str) -> Result<bool>;
}
