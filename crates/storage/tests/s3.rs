use bytes::Bytes;
use riichi_storage::{AttachmentStore, ObjectAttachmentStore};
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires a running S3-compatible object store"]
async fn s3_store_round_trips_and_deletes_bytes() {
    let store = ObjectAttachmentStore::from_env().unwrap();
    let key = format!("ci/{}.bin", Uuid::now_v7());
    let bytes = Bytes::from_static(b"s3 attachment");

    store.put(&key, bytes.clone()).await.unwrap();
    assert_eq!(store.get(&key).await.unwrap(), bytes);
    store.delete(&key).await.unwrap();
    assert!(store.get(&key).await.is_err());
}
