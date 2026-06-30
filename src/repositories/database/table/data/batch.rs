use crate::error::AppError;
use crate::models::database::table::data::ZoomLevelPolicy;
use crate::models::spatial_id::SpatialId;
use crate::repositories::KasaneDbWrite;
use tokio::sync::{mpsc, oneshot};

pub enum WriteOp {
    Insert {
        db_name: String,
        table_name: String,
        ids: Vec<SpatialId>,
        policy: ZoomLevelPolicy,
        value: serde_json::Value,
        reply: oneshot::Sender<Result<(), AppError>>,
    },
    Upsert {
        db_name: String,
        table_name: String,
        ids: Vec<SpatialId>,
        policy: ZoomLevelPolicy,
        value: serde_json::Value,
        reply: oneshot::Sender<Result<(), AppError>>,
    },
    Remove {
        db_name: String,
        table_name: String,
        ids: Vec<SpatialId>,
        policy: ZoomLevelPolicy,
        reply: oneshot::Sender<Result<(), AppError>>,
    },
}

pub fn spawn_batcher() -> (mpsc::Sender<WriteOp>, mpsc::Receiver<WriteOp>) {
    mpsc::channel::<WriteOp>(10000)
}

pub fn run_batcher(db: crate::db_init::AppDb, mut rx: mpsc::Receiver<WriteOp>) {
    std::thread::spawn(move || {
        let max_batch_size = 500;
        let timeout = std::time::Duration::from_millis(5);

        loop {
            let first_op = match rx.blocking_recv() {
                Some(op) => op,
                None => break,
            };

            let mut batch = Vec::with_capacity(max_batch_size);
            batch.push(first_op);

            let start_time = std::time::Instant::now();
            while batch.len() < max_batch_size && start_time.elapsed() < timeout {
                if let Ok(op) = rx.try_recv() {
                    batch.push(op);
                } else {
                    std::thread::sleep(std::time::Duration::from_micros(100));
                }
            }

            process_batch(&db, batch);
        }
    });
}

pub fn process_batch(db: &crate::db_init::AppDb, ops: Vec<WriteOp>) {
    let write_txn = match db.env.write_txn() {
        Ok(txn) => txn,
        Err(e) => {
            let err_msg = format!("Failed to begin WriteTxn: {}", e);
            for op in ops {
                let reply = match op {
                    WriteOp::Insert { reply, .. } => reply,
                    WriteOp::Upsert { reply, .. } => reply,
                    WriteOp::Remove { reply, .. } => reply,
                };
                let _ = reply.send(Err(AppError::InternalError(err_msg.clone())));
            }
            return;
        }
    };
    let mut db_write = KasaneDbWrite::new(write_txn, db);

    let mut results = Vec::with_capacity(ops.len());
    let mut replies = Vec::with_capacity(ops.len());

    for op in ops {
        let (res, reply) = match op {
            WriteOp::Insert {
                db_name,
                table_name,
                ids,
                policy,
                value,
                reply,
            } => {
                let r = handle_insert(&mut db_write, &db_name, &table_name, &ids, &policy, value);
                (r, reply)
            }
            WriteOp::Upsert {
                db_name,
                table_name,
                ids,
                policy,
                value,
                reply,
            } => {
                let r = handle_upsert(&mut db_write, &db_name, &table_name, &ids, &policy, value);
                (r, reply)
            }
            WriteOp::Remove {
                db_name,
                table_name,
                ids,
                policy,
                reply,
            } => {
                let r = handle_remove(&mut db_write, &db_name, &table_name, &ids, &policy);
                (r, reply)
            }
        };
        results.push(res);
        replies.push(reply);
    }

    let all_ok = results.iter().all(|r| r.is_ok());

    if all_ok {
        if let Err(e) = db_write.commit() {
            let err_msg = format!("Failed to commit batch: {}", e);
            for reply in replies {
                let _ = reply.send(Err(AppError::InternalError(err_msg.clone())));
            }
            return;
        }

        for reply in replies {
            let _ = reply.send(Ok(()));
        }
    } else {
        let _ = db_write.abort();
        for (reply, res) in replies.into_iter().zip(results) {
            let err = res
                .err()
                .unwrap_or_else(|| AppError::InternalError("Batch aborted".to_string()));
            let _ = reply.send(Err(err));
        }
    }
}

fn handle_insert(
    db: &mut KasaneDbWrite,
    db_name: &str,
    table_name: &str,
    spatial_ids: &[SpatialId],
    policy: &ZoomLevelPolicy,
    value: serde_json::Value,
) -> Result<(), AppError> {
    let table = match db.table_info(db_name, table_name)? {
        Some(v) => v,
        None => {
            return Err(AppError::TableNotFound {
                name: table_name.to_string(),
            });
        }
    };

    let parsed_value = crate::services::helpers::value::interpret_value(
        table.data_type,
        table.constraints.as_ref(),
        value,
    )?;
    let ids = crate::services::helpers::spatial_ids::process_spatial_ids(
        spatial_ids,
        table.max_zoom_level,
        policy,
    )?;

    db.data_insert(table.id, table.data_type, ids, &parsed_value)?;
    Ok(())
}

fn handle_upsert(
    db: &mut KasaneDbWrite,
    db_name: &str,
    table_name: &str,
    spatial_ids: &[SpatialId],
    policy: &ZoomLevelPolicy,
    value: serde_json::Value,
) -> Result<(), AppError> {
    let table = match db.table_info(db_name, table_name)? {
        Some(v) => v,
        None => {
            return Err(AppError::TableNotFound {
                name: table_name.to_string(),
            });
        }
    };

    let parsed_value = crate::services::helpers::value::interpret_value(
        table.data_type,
        table.constraints.as_ref(),
        value,
    )?;
    let ids = crate::services::helpers::spatial_ids::process_spatial_ids(
        spatial_ids,
        table.max_zoom_level,
        policy,
    )?;

    db.data_upsert(table.id, table.data_type, ids, &parsed_value)?;
    Ok(())
}

fn handle_remove(
    db: &mut KasaneDbWrite,
    db_name: &str,
    table_name: &str,
    spatial_ids: &[SpatialId],
    policy: &ZoomLevelPolicy,
) -> Result<(), AppError> {
    let table = match db.table_info(db_name, table_name)? {
        Some(v) => v,
        None => {
            return Err(AppError::TableNotFound {
                name: table_name.to_string(),
            });
        }
    };

    let ids = crate::services::helpers::spatial_ids::process_spatial_ids(
        spatial_ids,
        table.max_zoom_level,
        policy,
    )?;

    db.data_remove(table.id, table.data_type, ids)?;
    Ok(())
}
