//! S3-compatible storage client wrapper.

use anyhow::{Context, Result};
use s3::creds::Credentials;
use s3::region::Region;
use s3::Bucket;

use crate::config::RepositoryConfig;
use crate::storage::StorageOperations;

/// S3 client wrapping the rust-s3 Bucket with a synchronous interface.
pub struct S3Client {
    bucket: Box<Bucket>,
    runtime: tokio::runtime::Runtime,
}

impl S3Client {
    /// Create a new S3 client from repository configuration.
    pub fn new(config: &RepositoryConfig) -> Result<Self> {
        let bucket_name = config
            .bucket_name
            .as_deref()
            .context("bucket_name is required in repository config")?;

        let region = if let Some(ref endpoint) = config.endpoint {
            Region::Custom {
                region: config.region.clone(),
                endpoint: endpoint.clone(),
            }
        } else {
            config.region.parse().context("Invalid AWS region")?
        };

        let credentials = Credentials::default().context("Failed to load AWS credentials")?;

        let bucket = Bucket::new(bucket_name, region, credentials)
            .context("Failed to create S3 bucket client")?;

        let runtime = tokio::runtime::Runtime::new().context("Failed to create tokio runtime")?;

        Ok(Self { bucket, runtime })
    }
}

impl StorageOperations for S3Client {
    fn put_object(&self, key: &str, data: &[u8]) -> Result<()> {
        let response = self
            .runtime
            .block_on(self.bucket.put_object(key, data))
            .with_context(|| format!("Failed to put object: {}", key))?;

        if response.status_code() >= 300 {
            anyhow::bail!(
                "S3 put_object failed with status {} for key: {}",
                response.status_code(),
                key
            );
        }
        Ok(())
    }

    fn get_object(&self, key: &str) -> Result<Vec<u8>> {
        let response = self
            .runtime
            .block_on(self.bucket.get_object(key))
            .with_context(|| format!("Failed to get object: {}", key))?;

        if response.status_code() == 404 {
            anyhow::bail!("Object not found: {}", key);
        }
        if response.status_code() >= 300 {
            anyhow::bail!(
                "S3 get_object failed with status {} for key: {}",
                response.status_code(),
                key
            );
        }
        Ok(response.to_vec())
    }

    fn delete_object(&self, key: &str) -> Result<()> {
        let response = self
            .runtime
            .block_on(self.bucket.delete_object(key))
            .with_context(|| format!("Failed to delete object: {}", key))?;

        if response.status_code() >= 300 {
            anyhow::bail!(
                "S3 delete_object failed with status {} for key: {}",
                response.status_code(),
                key
            );
        }
        Ok(())
    }

    fn list_objects(&self, prefix: &str) -> Result<Vec<String>> {
        let results = self
            .runtime
            .block_on(self.bucket.list(prefix.to_string(), None))
            .with_context(|| format!("Failed to list objects with prefix: {}", prefix))?;

        let keys: Vec<String> = results
            .into_iter()
            .flat_map(|page| page.contents)
            .map(|obj| obj.key)
            .collect();

        Ok(keys)
    }

    fn object_exists(&self, key: &str) -> Result<bool> {
        let response = self.runtime.block_on(self.bucket.head_object(key));

        match response {
            Ok((_, code)) => Ok(code < 300),
            Err(_) => Ok(false),
        }
    }
}

/// Mock S3 client for testing, backed by an in-memory HashMap.
pub mod mock {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;

    pub struct MockS3Client {
        store: RefCell<HashMap<String, Vec<u8>>>,
    }

    impl Default for MockS3Client {
        fn default() -> Self {
            Self::new()
        }
    }

    impl MockS3Client {
        pub fn new() -> Self {
            Self {
                store: RefCell::new(HashMap::new()),
            }
        }
    }

    impl StorageOperations for MockS3Client {
        fn put_object(&self, key: &str, data: &[u8]) -> Result<()> {
            self.store
                .borrow_mut()
                .insert(key.to_string(), data.to_vec());
            Ok(())
        }

        fn get_object(&self, key: &str) -> Result<Vec<u8>> {
            self.store
                .borrow()
                .get(key)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("Object not found: {}", key))
        }

        fn delete_object(&self, key: &str) -> Result<()> {
            self.store.borrow_mut().remove(key);
            Ok(())
        }

        fn list_objects(&self, prefix: &str) -> Result<Vec<String>> {
            let keys: Vec<String> = self
                .store
                .borrow()
                .keys()
                .filter(|k| k.starts_with(prefix))
                .cloned()
                .collect();
            Ok(keys)
        }

        fn object_exists(&self, key: &str) -> Result<bool> {
            Ok(self.store.borrow().contains_key(key))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::mock::MockS3Client;
    use super::*;

    #[test]
    fn test_mock_put_and_get() {
        let client = MockS3Client::new();
        client.put_object("test/key.txt", b"hello").unwrap();

        let data = client.get_object("test/key.txt").unwrap();
        assert_eq!(data, b"hello");
    }

    #[test]
    fn test_mock_get_not_found() {
        let client = MockS3Client::new();
        let result = client.get_object("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_mock_delete() {
        let client = MockS3Client::new();
        client.put_object("key", b"data").unwrap();
        client.delete_object("key").unwrap();
        assert!(!client.object_exists("key").unwrap());
    }

    #[test]
    fn test_mock_list_objects() {
        let client = MockS3Client::new();
        client.put_object("skills/a/1.0/a.skill", b"a").unwrap();
        client.put_object("skills/a/2.0/a.skill", b"a2").unwrap();
        client.put_object("skills/b/1.0/b.skill", b"b").unwrap();

        let mut keys = client.list_objects("skills/a/").unwrap();
        keys.sort();
        assert_eq!(keys, vec!["skills/a/1.0/a.skill", "skills/a/2.0/a.skill"]);
    }

    #[test]
    fn test_mock_object_exists() {
        let client = MockS3Client::new();
        assert!(!client.object_exists("key").unwrap());
        client.put_object("key", b"data").unwrap();
        assert!(client.object_exists("key").unwrap());
    }
}
