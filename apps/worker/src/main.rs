use riichi_application::config::AppConfig;
use riichi_persistence::Database;
use riichi_storage::ObjectAttachmentStore;
use riichi_worker::{WorkerError, process_document_job, process_message};
use tokio::task::JoinSet;
use tokio::time::{Duration, interval, sleep};
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let config = AppConfig::from_env()?;
    let database = Database::connect(&config.database_url, config.max_database_connections).await?;
    database.migrate().await?;
    let attachment_store = ObjectAttachmentStore::from_env()?;

    info!("riichi worker starting");
    let mut expiry_sweep = interval(Duration::from_secs(5));
    let mut deliveries = JoinSet::new();
    let mut document_jobs = JoinSet::new();
    const MAX_CONCURRENT_DELIVERIES: usize = 16;
    const MAX_CONCURRENT_DOCUMENT_JOBS: usize = 4;
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => break,
            _ = expiry_sweep.tick() => {
                match database.sweep_expired_leases().await {
                    Ok(count) if count > 0 => info!(count, "expired leases swept"),
                    Ok(_) => {},
                    Err(error) => warn!(%error, "lease expiry sweep failed"),
                }
            }
            delivery = deliveries.join_next(), if !deliveries.is_empty() => {
                match delivery {
                    Some(Ok((message_id, Ok(event)))) => {
                        info!(message_id = %message_id, event = ?event, "outbox message durably handed off");
                    }
                    Some(Ok((message_id, Err(WorkerError::Delivery(error))))) => {
                        warn!(message_id = %message_id, error = %error, "outbox message retry scheduled");
                    }
                    Some(Ok((message_id, Err(WorkerError::DeadLettered(error))))) => {
                        error!(message_id = %message_id, error = %error, "outbox message dead-lettered");
                    }
                    Some(Ok((_, Err(WorkerError::Database(error))))) => {
                        return Err(Box::new(error) as Box<dyn std::error::Error>);
                    }
                    Some(Ok((_, Err(WorkerError::DocumentJob(error)))))
                    | Some(Ok((_, Err(WorkerError::DocumentJobDeadLettered(error))))) => {
                        return Err(Box::new(error) as Box<dyn std::error::Error>);
                    }
                    Some(Err(error)) => {
                        return Err(Box::new(error) as Box<dyn std::error::Error>);
                    }
                    None => {}
                }
            }
            document_job = document_jobs.join_next(), if !document_jobs.is_empty() => {
                match document_job {
                    Some(Ok((job_id, Ok(())))) => {
                        info!(job_id = %job_id, "document job completed");
                    }
                    Some(Ok((job_id, Err(WorkerError::DocumentJob(error))))) => {
                        warn!(job_id = %job_id, error = %error, "document job retry scheduled");
                    }
                    Some(Ok((job_id, Err(WorkerError::DocumentJobDeadLettered(error))))) => {
                        error!(job_id = %job_id, error = %error, "document job dead-lettered");
                    }
                    Some(Ok((_, Err(WorkerError::Database(error))))) => {
                        return Err(Box::new(error) as Box<dyn std::error::Error>);
                    }
                    Some(Ok((_, Err(error)))) => {
                        return Err(Box::new(error) as Box<dyn std::error::Error>);
                    }
                    Some(Err(error)) => {
                        return Err(Box::new(error) as Box<dyn std::error::Error>);
                    }
                    None => {}
                }
            }
            message = database.claim_next_outbox(None), if deliveries.len() < MAX_CONCURRENT_DELIVERIES => {
                match message? {
                    Some(message) => {
                        let worker_database = database.clone();
                        deliveries.spawn(async move {
                            let message_id = message.id;
                            let result = process_message(&worker_database, &message).await;
                            (message_id, result)
                        });
                    }
                    None => sleep(Duration::from_millis(250)).await,
                }
            }
            job = database.claim_next_document_job(), if document_jobs.len() < MAX_CONCURRENT_DOCUMENT_JOBS => {
                match job? {
                    Some(job) => {
                        let worker_database = database.clone();
                        let worker_attachment_store = attachment_store.clone();
                        document_jobs.spawn(async move {
                            let job_id = job.id;
                            let result = process_document_job(
                                &worker_database,
                                &job,
                                &worker_attachment_store,
                            ).await;
                            (job_id, result)
                        });
                    }
                    None => sleep(Duration::from_millis(250)).await,
                }
            }
        }
    }
    info!("riichi worker stopping");

    Ok(())
}
