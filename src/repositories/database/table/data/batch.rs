use crate::error::AppError;
use crate::models::database::table::TableDataType;
use crate::models::id::TableId;
use crate::repositories::KasaneDbWrite;
use kasane_logic::SpatialIdSet;
use tokio::sync::{mpsc, oneshot};

/// 書き込みバッチャーへ渡す**検証済み**の書き込み要求。
pub enum WriteOp {
    Insert {
        table_id: TableId,
        data_type: TableDataType,
        ids: SpatialIdSet,
        value: Vec<u8>,
        reply: oneshot::Sender<Result<(), AppError>>,
    },
    Upsert {
        table_id: TableId,
        data_type: TableDataType,
        ids: SpatialIdSet,
        value: Vec<u8>,
        reply: oneshot::Sender<Result<(), AppError>>,
    },
    Remove {
        table_id: TableId,
        data_type: TableDataType,
        ids: SpatialIdSet,
        reply: oneshot::Sender<Result<(), AppError>>,
    },
}

impl WriteOp {
    /// バッチ処理を諦めるときに、各 op の応答チャネルだけを取り出す。
    fn into_reply(self) -> oneshot::Sender<Result<(), AppError>> {
        match self {
            WriteOp::Insert { reply, .. }
            | WriteOp::Upsert { reply, .. }
            | WriteOp::Remove { reply, .. } => reply,
        }
    }
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

            // 1バッチの処理中に panic しても、ライタスレッド自体は落とさない。
            // （落とすと write_sender は生きたまま受信側だけ消え、以降の全書き込みが恒久的に失敗する）
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                process_batch(&db, batch);
            }));
            if result.is_err() {
                // panic 時、当該バッチの reply は drop され呼び出し側は RecvError を受け取る。
                tracing::error!("write batcher panicked while processing a batch; continuing");
            }
        }
    });
}

pub fn process_batch(db: &crate::db_init::AppDb, ops: Vec<WriteOp>) {
    let write_txn = match db.env.write_txn() {
        Ok(txn) => txn,
        Err(e) => {
            let err_msg = format!("Failed to begin WriteTxn: {}", e);
            for op in ops {
                let _ = op
                    .into_reply()
                    .send(Err(AppError::InternalError(err_msg.clone())));
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
                table_id,
                data_type,
                ids,
                value,
                reply,
            } => (
                db_write.data_insert(table_id, data_type, ids, &value),
                reply,
            ),
            WriteOp::Upsert {
                table_id,
                data_type,
                ids,
                value,
                reply,
            } => (
                db_write.data_upsert(table_id, data_type, ids, &value),
                reply,
            ),
            WriteOp::Remove {
                table_id,
                data_type,
                ids,
                reply,
            } => (db_write.data_remove(table_id, data_type, ids), reply),
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
        // 失敗した op には自身のエラーを、それ以外には巻き添えで発生した`abort`を返す。
        // 他のリクエストの巻き添えで`abort`するケースはなく、LMDBのレイヤーのエラーにより`abort`するため、ほとんど発生することはない
        for (reply, res) in replies.into_iter().zip(results) {
            let err = res
                .err()
                .unwrap_or_else(|| AppError::InternalError("Batch aborted".to_string()));
            let _ = reply.send(Err(err));
        }
    }
}
