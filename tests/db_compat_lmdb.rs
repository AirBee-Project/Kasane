//! **LMDBのディスク形式が、バージョンアップやPRで壊れていないことの検証。**
//!
//! `tests/fixtures/db_compat/lmdb/vN/fixture.json` は、過去のスキーマ版 `N` で実際に
//! 書き込まれた生バイト列と、そこから読めるべき値を記録したもの。ここでは、その生バイト列を
//! 新規の LMDB 環境へそのまま注入し、現在のコードで開いたときに:
//!
//! - フィクスチャの版が現在の [`SCHEMA_VERSION`] と同じなら、記録どおりに読めること
//!   （＝意図せず読み方が変わっていないこと）
//! - 版が違うなら、`SchemaVersionMismatch` としてはっきり拒否されること
//!   （＝黙って誤読しないこと。破壊的変更をした版はここに落ちるので、そのフィクスチャに
//!   ついては新しい版のものを別途追加しない限り「読めなくなった」以上のことは検証しない）
//!
//! フィクスチャの追加・更新は手動。`docs/test.md` および
//! `cargo test --no-default-features --features "backend-lmdb,gen-fixtures" \
//! --test gen_db_compat_fixture` を参照。
#![cfg(feature = "backend-lmdb")]

use kasane::error::AppError;
use kasane::repositories::SCHEMA_VERSION;
use kasane::repositories::lmdb::initialize_database;

#[path = "shared/db_compat.rs"]
mod db_compat;

#[tokio::test]
async fn historical_fixtures_stay_readable_or_are_cleanly_rejected() {
    let fixtures = db_compat::load_all_fixtures("lmdb");
    assert!(
        !fixtures.is_empty(),
        "tests/fixtures/db_compat/lmdb/ にフィクスチャが1つも無い。\
         gen_db_compat_fixture で生成してコミットすること"
    );

    for fixture in fixtures {
        let version = fixture.schema_version;
        let tmp = tempfile::TempDir::new().unwrap();

        db_compat::load_lmdb(tmp.path(), &fixture.entries);

        let opened = initialize_database(tmp.path().to_str().unwrap());

        if version == SCHEMA_VERSION {
            let db = match opened {
                Ok(db) => db,
                Err(e) => panic!(
                    "現行スキーマ版(v{SCHEMA_VERSION})のフィクスチャ(v{version})が開けない: {e}\n\
                     読み方を変えたなら SCHEMA_VERSION を上げ、新しい版のフィクスチャを追加すること"
                ),
            };
            let actual = db_compat::read_actual(&db).await;
            assert_eq!(
                actual, fixture.expected,
                "v{version} のフィクスチャを読んだ結果が記録と食い違っている。\
                 意図した変更なら SCHEMA_VERSION を上げること"
            );
        } else {
            let err = match opened {
                Err(e) => e,
                Ok(_) => panic!(
                    "版が違うはずのフィクスチャ(v{version}、現行はv{SCHEMA_VERSION})が\
                     エラー無く開けてしまった。黙って誤読している可能性がある"
                ),
            };
            assert!(
                matches!(err, AppError::SchemaVersionMismatch { .. }),
                "版違いのフィクスチャ(v{version})が SchemaVersionMismatch 以外の形で失敗した: {err:?}"
            );
        }
    }
}
