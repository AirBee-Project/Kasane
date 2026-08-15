//! `tests/fixtures/db_compat/` のフィクスチャを生成する手動ツール。
//!
//! 通常の `cargo test` / CI では走らない（`gen-fixtures` フィーチャを立てたときだけ
//! コンパイルされる）。`SCHEMA_VERSION` を上げたときなど、新しい版のフィクスチャを
//! 追加したい開発者が手動で実行してコミットする。
//!
//! ```bash
//! # LMDB
//! cargo test --no-default-features --features "backend-lmdb,gen-fixtures" \
//!     --test gen_db_compat_fixture -- --nocapture
//!
//! # TiKV（先に `docker compose up -d --wait` でクラスタを起こしておくこと。
//! # 専有していないクラスタに対して走らせると、他の書き込みまでフィクスチャへ
//! # 混ざる恐れがある）
//! cargo test --no-default-features --features "backend-tikv,gen-fixtures" \
//!     --test gen_db_compat_fixture -- --nocapture
//! ```
#![cfg(feature = "gen-fixtures")]

#[path = "shared/db_compat.rs"]
mod db_compat;

fn write_fixture(backend: &str, fixture: &db_compat::Fixture) {
    let out_path = db_compat::fixture_path(backend, fixture.schema_version);
    std::fs::create_dir_all(out_path.parent().unwrap()).unwrap();
    std::fs::write(&out_path, serde_json::to_vec_pretty(fixture).unwrap()).unwrap();
    println!("wrote {}", out_path.display());
}

#[cfg(feature = "backend-lmdb")]
#[tokio::test]
async fn generate_lmdb_fixture() {
    let tmp = tempfile::TempDir::new().unwrap();
    let path = tmp.path().to_path_buf();

    let db = kasane::repositories::lmdb::initialize_database(path.to_str().unwrap()).unwrap();
    db_compat::populate(&db).await;
    let expected = db_compat::read_actual(&db).await;
    // raw dump 用に別の `Env` を開けるよう、型付きの `Env` を先に手放す。
    drop(db);

    let entries = db_compat::dump_lmdb(&path);
    assert!(!entries.is_empty(), "populate で何も書かれていない");

    let fixture = db_compat::Fixture {
        schema_version: kasane::repositories::SCHEMA_VERSION,
        entries,
        expected,
    };

    // 生成した生バイト列を、実際にもう一度読み直して自己検証する
    // （dump/load のバグをここで捕まえる。互換性テスト本体はこの往復を前提にしている）。
    {
        let verify_tmp = tempfile::TempDir::new().unwrap();
        db_compat::load_lmdb(verify_tmp.path(), &fixture.entries);
        let reopened =
            kasane::repositories::lmdb::initialize_database(verify_tmp.path().to_str().unwrap())
                .unwrap();
        let actual = db_compat::read_actual(&reopened).await;
        assert_eq!(
            actual, fixture.expected,
            "生成直後の自己往復検証が一致しない（dump/load 側のバグの可能性が高い）"
        );
    }

    write_fixture("lmdb", &fixture);
}

#[cfg(feature = "backend-tikv")]
#[tokio::test]
async fn generate_tikv_fixture() {
    use kasane::repositories::tikv::{TikvConfig, TikvDb};

    let config = TikvConfig::from_env();
    let db = TikvDb::connect(config.clone())
        .await
        .expect("cannot reach TiKV; start the cluster with `docker compose up -d --wait`");
    let raw = tikv_client::TransactionClient::new(config.pd_endpoints.clone())
        .await
        .expect("cannot open a raw client to the same cluster");

    let before = db_compat::tikv_full_scan(&raw).await;
    db_compat::populate(&db).await;
    let expected = db_compat::read_actual(&db).await;
    let after = db_compat::tikv_full_scan(&raw).await;

    let entries = db_compat::tikv_diff(&before, &after);
    assert!(
        !entries.is_empty(),
        "populate で何も書かれていない、あるいは差分が取れていない"
    );

    let fixture = db_compat::Fixture {
        schema_version: kasane::repositories::SCHEMA_VERSION,
        entries,
        expected,
    };

    write_fixture("tikv", &fixture);
}
