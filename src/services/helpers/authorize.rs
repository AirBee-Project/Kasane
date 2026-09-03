//! 認可と対象解決を 1 回の読み取りで済ませるための入口。
//!
//! 「認可のために名前を引く → サービス層が同じ名前を引き直す」を無くすためのもの。
//! 名前の解決がネットワーク往復になるバックエンドでは、この二重取得がそのまま
//! リクエストのレイテンシに乗る。

use crate::{
    AppState,
    error::{AppError, Resource},
    models::database::table::Table,
    models::users::{User, UserRole},
    repositories::{ReadRepository, Storage},
    services::auth::authorize_resolved,
};

/// **開いている読み取り**の中で、テーブルへの権限を確かめメタデータを返す。
///
/// 同じ断面で続けて別のものも読みたい呼び出し側（件数など）はこちらを使う。
pub async fn resolve_authorized_table<R: ReadRepository>(
    repo: &R,
    user: &User,
    db_name: &str,
    table_name: &str,
    required: UserRole,
) -> Result<Table, AppError> {
    let refs = [(db_name.to_string(), table_name.to_string())];
    let resolved = repo.resolve_tables(&refs).await?.pop().ok_or_else(|| {
        AppError::InternalError("resolve_tables dropped its only request".to_string())
    })?;

    // 「存在しない」より先に判定する。逆にすると、権限の無い利用者へ 404 で
    // 名前の存在有無を教えることになる。
    authorize_resolved(
        repo,
        user,
        resolved.db_id,
        resolved.table.as_ref().map(|t| t.id),
        db_name,
        Some(table_name),
        required,
    )
    .await?;

    resolved
        .table
        .ok_or_else(|| Resource::Table.not_found(table_name.to_string()))
}

/// 読み取りを 1 つ開いて [`resolve_authorized_table`] を行う。
///
/// 解決・認可・取得がすべて同じ断面で起きるので、判定した対象と操作する対象がずれない。
#[tracing::instrument(skip_all, fields(db_name = %db_name, table_name = %table_name))]
pub async fn authorized_table(
    app_state: &AppState,
    user: &User,
    db_name: &str,
    table_name: &str,
    required: UserRole,
) -> Result<Table, AppError> {
    let db_name = db_name.to_string();
    let table_name = table_name.to_string();
    let user = user.clone();

    app_state
        .db
        .read(async move |repo| {
            resolve_authorized_table(repo, &user, &db_name, &table_name, required).await
        })
        .await
}
