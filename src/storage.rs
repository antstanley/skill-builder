//! Storage operations trait for S3 and filesystem backends.

use anyhow::Result;

/// Trait for storage operations, enabling S3, filesystem, and mock implementations.
pub trait StorageOperations {
    /// Store data at the given key.
    ///
    /// # Errors
    ///
    /// Returns an error if the write operation fails.
    fn put_object(&self, key: &str, data: &[u8]) -> Result<()>;

    /// Retrieve data for the given key.
    ///
    /// # Errors
    ///
    /// Returns an error if the key does not exist or cannot be read.
    fn get_object(&self, key: &str) -> Result<Vec<u8>>;

    /// Delete the object at the given key.
    ///
    /// # Errors
    ///
    /// Returns an error if the delete operation fails.
    fn delete_object(&self, key: &str) -> Result<()>;

    /// List all object keys matching the given prefix.
    ///
    /// # Errors
    ///
    /// Returns an error if the listing operation fails.
    fn list_objects(&self, prefix: &str) -> Result<Vec<String>>;

    /// Check whether an object exists at the given key.
    ///
    /// # Errors
    ///
    /// Returns an error if the existence check fails.
    fn object_exists(&self, key: &str) -> Result<bool>;
}
