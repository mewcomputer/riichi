use std::{env, path::Path as FsPath, sync::Arc};

use async_trait::async_trait;
use bytes::Bytes;
use object_store::{ObjectStore, ObjectStoreExt, path::Path};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("storage key is invalid")]
    InvalidKey,
    #[error("storage configuration is invalid: {0}")]
    Configuration(String),
    #[error("storage I/O failed: {0}")]
    ObjectStore(#[from] object_store::Error),
    #[error("local storage directory could not be created: {0}")]
    LocalDirectory(#[from] std::io::Error),
}

#[async_trait]
pub trait AttachmentStore: Send + Sync {
    async fn put(&self, storage_key: &str, bytes: Bytes) -> Result<(), StorageError>;
    async fn get(&self, storage_key: &str) -> Result<Bytes, StorageError>;
    async fn delete(&self, storage_key: &str) -> Result<(), StorageError>;
}

#[derive(Clone)]
pub struct ObjectAttachmentStore {
    inner: Arc<dyn ObjectStore>,
}

impl std::fmt::Debug for ObjectAttachmentStore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ObjectAttachmentStore")
            .field("inner", &self.inner.to_string())
            .finish()
    }
}

impl ObjectAttachmentStore {
    pub fn local(root: impl AsRef<FsPath>) -> Result<Self, StorageError> {
        let root = root.as_ref();
        std::fs::create_dir_all(root)?;
        let store = object_store::local::LocalFileSystem::new_with_prefix(root)?;
        Ok(Self {
            inner: Arc::new(store.with_automatic_cleanup(true).with_fsync(true)),
        })
    }

    pub fn s3(
        endpoint: Option<&str>,
        bucket: &str,
        region: &str,
        access_key_id: &str,
        secret_access_key: &str,
        allow_http: bool,
    ) -> Result<Self, StorageError> {
        let mut builder = object_store::aws::AmazonS3Builder::new()
            .with_bucket_name(bucket)
            .with_region(region)
            .with_access_key_id(access_key_id)
            .with_secret_access_key(secret_access_key)
            .with_allow_http(allow_http)
            .with_virtual_hosted_style_request(false);
        if let Some(endpoint) = endpoint {
            builder = builder.with_endpoint(endpoint);
        }
        Ok(Self {
            inner: Arc::new(builder.build()?),
        })
    }

    pub fn from_env() -> Result<Self, StorageError> {
        match env::var("RIICHI_ATTACHMENT_BACKEND")
            .unwrap_or_else(|_| "local".to_owned())
            .as_str()
        {
            "local" => Self::local(
                env::var("RIICHI_ATTACHMENT_DIR").unwrap_or_else(|_| "data/attachments".to_owned()),
            ),
            "s3" => Self::s3(
                env::var("RIICHI_ATTACHMENT_S3_ENDPOINT").ok().as_deref(),
                &required_env("RIICHI_ATTACHMENT_S3_BUCKET")?,
                &env::var("RIICHI_ATTACHMENT_S3_REGION").unwrap_or_else(|_| "us-east-1".to_owned()),
                &required_env("RIICHI_ATTACHMENT_S3_ACCESS_KEY_ID")?,
                &required_env("RIICHI_ATTACHMENT_S3_SECRET_ACCESS_KEY")?,
                env::var("RIICHI_ATTACHMENT_S3_ALLOW_HTTP")
                    .map(|value| value.eq_ignore_ascii_case("true"))
                    .unwrap_or(false),
            ),
            backend => Err(StorageError::Configuration(format!(
                "unsupported attachment backend {backend:?}"
            ))),
        }
    }

    pub fn is_not_found(error: &StorageError) -> bool {
        matches!(
            error,
            StorageError::ObjectStore(object_store::Error::NotFound { .. })
        )
    }

    async fn location(&self, storage_key: &str) -> Result<Path, StorageError> {
        validate_storage_key(storage_key)?;
        Ok(Path::from(storage_key))
    }
}

#[async_trait]
impl AttachmentStore for ObjectAttachmentStore {
    async fn put(&self, storage_key: &str, bytes: Bytes) -> Result<(), StorageError> {
        let location = self.location(storage_key).await?;
        self.inner.put(&location, bytes.into()).await?;
        Ok(())
    }

    async fn get(&self, storage_key: &str) -> Result<Bytes, StorageError> {
        let location = self.location(storage_key).await?;
        Ok(self.inner.get(&location).await?.bytes().await?)
    }

    async fn delete(&self, storage_key: &str) -> Result<(), StorageError> {
        let location = self.location(storage_key).await?;
        self.inner.delete(&location).await?;
        Ok(())
    }
}

fn required_env(name: &str) -> Result<String, StorageError> {
    env::var(name).map_err(|_| StorageError::Configuration(format!("{name} is required")))
}

fn validate_storage_key(storage_key: &str) -> Result<(), StorageError> {
    let path = FsPath::new(storage_key);
    if storage_key.is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(StorageError::InvalidKey);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[tokio::test]
    async fn local_store_round_trips_and_deletes_bytes() {
        let root = std::env::temp_dir().join(format!("riichi-storage-{}", Uuid::now_v7()));
        let store = ObjectAttachmentStore::local(&root).unwrap();

        store
            .put("uploads/example.bin", Bytes::from_static(b"attachment"))
            .await
            .unwrap();
        assert_eq!(
            store.get("uploads/example.bin").await.unwrap(),
            Bytes::from_static(b"attachment")
        );

        store.delete("uploads/example.bin").await.unwrap();
        assert!(store.get("uploads/example.bin").await.is_err());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_path_traversal_keys() {
        assert!(matches!(
            validate_storage_key("../escape"),
            Err(StorageError::InvalidKey)
        ));
        assert!(matches!(
            validate_storage_key("/absolute"),
            Err(StorageError::InvalidKey)
        ));
    }
}
