//! LMDB バックエンド向けの結合テスト。TiKV バックエンドのビルドでは対象外。
#![cfg(feature = "backend-lmdb")]

mod common;

use common::TestApp;
use common::builders::{merge, num, range_id, single_id, source};
use kasane::grpc::pb;
use tonic::{Code, Status};
use tonic_types::StatusExt;

fn bearer(token: &str) -> String {
    format!("Bearer {token}")
}

fn error_reason(status: &Status) -> String {
    status
        .get_error_details()
        .error_info()
        .map(|info| info.reason.clone())
        .unwrap_or_default()
}

async fn create_user_and_token(
    test_app: &TestApp,
    root_token: &str,
    username: &str,
    is_global_admin: bool,
) -> String {
    let privileges = if is_global_admin {
        vec![pb::PrivilegeRule {
            scope: Some(pb::privilege_rule::Scope::Global(
                pb::privilege_rule::Global {
                    role: pb::UserRole::Admin as i32,
                },
            )),
        }]
    } else {
        vec![]
    };

    test_app
        .user_as(Some(&bearer(root_token)))
        .create(pb::CreateUserRequest {
            username: username.to_string(),
            password: "password".to_string(),
            privileges,
        })
        .await
        .expect("create_user failed");

    let resp = test_app
        .auth()
        .login(pb::LoginRequest {
            username: username.to_string(),
            password: "password".to_string(),
        })
        .await
        .expect("login failed");
    resp.into_inner().token
}

/// 権限一覧を取得する。
async fn fetch_privileges(
    test_app: &TestApp,
    token: &str,
    username: &str,
) -> Vec<pb::PrivilegeRule> {
    test_app
        .user_as(Some(&bearer(token)))
        .get_privileges(pb::GetPrivilegesRequest {
            username: username.to_string(),
        })
        .await
        .expect("get_privileges failed")
        .into_inner()
        .privileges
}

fn data_role(role: &str) -> pb::DataRole {
    match role {
        "read" => pb::DataRole::Read,
        "write" => pb::DataRole::Write,
        "manage" => pb::DataRole::Manage,
        other => panic!("unknown data role: {other}"),
    }
}

fn user_role(role: &str) -> pb::UserRole {
    match role {
        "read" => pb::UserRole::Read,
        "write" => pb::UserRole::Write,
        "manage" => pb::UserRole::Manage,
        "admin" => pb::UserRole::Admin,
        other => panic!("unknown role: {other}"),
    }
}

/// 対象 1 件に権限を設定する（結果は呼び出し側が判断する）。
async fn put_privilege(
    test_app: &TestApp,
    token: &str,
    username: &str,
    db_name: Option<&str>,
    table_name: Option<&str>,
    role: &str,
) -> Result<(), Status> {
    let client_token = Some(bearer(token));
    let client_token = client_token.as_deref();
    match (db_name, table_name) {
        (None, _) => test_app
            .user_as(client_token)
            .set_global_privilege(pb::SetGlobalPrivilegeRequest {
                username: username.to_string(),
                role: user_role(role) as i32,
            })
            .await
            .map(|_| ()),
        (Some(db), None) => test_app
            .user_as(client_token)
            .set_database_privilege(pb::SetDatabasePrivilegeRequest {
                username: username.to_string(),
                db_name: db.to_string(),
                role: data_role(role) as i32,
            })
            .await
            .map(|_| ()),
        (Some(db), Some(table)) => test_app
            .user_as(client_token)
            .set_table_privilege(pb::SetTablePrivilegeRequest {
                username: username.to_string(),
                db_name: db.to_string(),
                table_name: table.to_string(),
                role: data_role(role) as i32,
            })
            .await
            .map(|_| ()),
    }
}

/// 対象 1 件の権限を剥奪する（結果は呼び出し側が判断する）。
async fn delete_privilege(
    test_app: &TestApp,
    token: &str,
    username: &str,
    db_name: Option<&str>,
    table_name: Option<&str>,
) -> Result<(), Status> {
    let client_token = Some(bearer(token));
    let client_token = client_token.as_deref();
    match (db_name, table_name) {
        (None, _) => test_app
            .user_as(client_token)
            .delete_global_privilege(pb::DeleteGlobalPrivilegeRequest {
                username: username.to_string(),
            })
            .await
            .map(|_| ()),
        (Some(db), None) => test_app
            .user_as(client_token)
            .delete_database_privilege(pb::DeleteDatabasePrivilegeRequest {
                username: username.to_string(),
                db_name: db.to_string(),
            })
            .await
            .map(|_| ()),
        (Some(db), Some(table)) => test_app
            .user_as(client_token)
            .delete_table_privilege(pb::DeleteTablePrivilegeRequest {
                username: username.to_string(),
                db_name: db.to_string(),
                table_name: table.to_string(),
            })
            .await
            .map(|_| ()),
    }
}

async fn grant_privilege(
    test_app: &TestApp,
    root_token: &str,
    username: &str,
    db_name: &str,
    role: &str,
) {
    put_privilege(test_app, root_token, username, Some(db_name), None, role)
        .await
        .expect("grant_privilege failed");
}

async fn grant_table_privilege(
    test_app: &TestApp,
    root_token: &str,
    username: &str,
    db_name: &str,
    table_name: &str,
    role: &str,
) {
    put_privilege(
        test_app,
        root_token,
        username,
        Some(db_name),
        Some(table_name),
        role,
    )
    .await
    .expect("grant_table_privilege failed");
}

/// root 権限でデータベースを作る。
async fn create_db(test_app: &TestApp, root_token: &str, name: &str) {
    test_app
        .database_as(Some(&bearer(root_token)))
        .create(pb::CreateDatabaseRequest {
            name: name.to_string(),
            description: None,
        })
        .await
        .expect("create_db failed");
}

/// テーブル作成を試みる（結果は呼び出し側が判断する）。
async fn post_table(
    test_app: &TestApp,
    token: &str,
    db_name: &str,
    table_name: &str,
) -> Result<(), Status> {
    test_app
        .table_as(Some(&bearer(token)))
        .create(pb::CreateTableRequest {
            db_name: db_name.to_string(),
            name: table_name.to_string(),
            data_type: pb::TableDataType::Int as i32,
            max_zoom_level: 5,
            constraints: None,
            description: None,
            value_index: false,
            is_temporal: true,
        })
        .await
        .map(|_| ())
}

/// テーブルを作り、成功することを確かめる。
async fn create_table(test_app: &TestApp, token: &str, db_name: &str, table_name: &str) {
    post_table(test_app, token, db_name, table_name)
        .await
        .expect("create_table failed");
}

async fn delete_table(test_app: &TestApp, token: &str, db_name: &str, table_name: &str) {
    test_app
        .table_as(Some(&bearer(token)))
        .delete(pb::DeleteTableRequest {
            db_name: db_name.to_string(),
            table_name: table_name.to_string(),
        })
        .await
        .expect("delete_table failed");
}

/// 指定ユーザーのトークンが有効か（データベース一覧を取得できるか）を検証する。
async fn token_is_valid(test_app: &TestApp, token: &str) -> bool {
    test_app
        .database_as(Some(&bearer(token)))
        .list(pb::ListDatabasesRequest {})
        .await
        .is_ok()
}

#[tokio::test]
/// 認証・認可に関する各種エラーコードが正しく構造化されて返されるかを検証する。
async fn test_auth_error_codes_are_structured() {
    let test_app = TestApp::new().await;
    let root_token = test_app.root_token().to_string();

    // DB を1つ作っておく（権限不足コードの確認用）
    create_db(&test_app, &root_token, "code_db").await;

    // 一般ユーザー（権限なし）
    let user_token = create_user_and_token(&test_app, &root_token, "code_user", false).await;

    // 1. ヘッダ無し → missing_token
    let err = test_app
        .database_as(None)
        .list(pb::ListDatabasesRequest {})
        .await
        .unwrap_err();
    assert_eq!(err.code(), Code::Unauthenticated);
    assert_eq!(error_reason(&err), "missing_token");

    // 2. Bearer でない → malformed_header
    let err = test_app
        .database_as(Some("Basic abc"))
        .list(pb::ListDatabasesRequest {})
        .await
        .unwrap_err();
    assert_eq!(err.code(), Code::Unauthenticated);
    assert_eq!(error_reason(&err), "malformed_header");

    // 3. 不正なトークン → invalid_token
    let err = test_app
        .database_as(Some("Bearer not-a-jwt"))
        .list(pb::ListDatabasesRequest {})
        .await
        .unwrap_err();
    assert_eq!(err.code(), Code::Unauthenticated);
    assert_eq!(error_reason(&err), "invalid_token");

    // 4. ログイン失敗 → invalid_credentials
    let err = test_app
        .auth()
        .login(pb::LoginRequest {
            username: "code_user".to_string(),
            password: "wrong".to_string(),
        })
        .await
        .unwrap_err();
    assert_eq!(err.code(), Code::Unauthenticated);
    assert_eq!(error_reason(&err), "invalid_credentials");

    // 5. GlobalAdmin 専用エンドポイント → requires_global_admin
    let err = test_app
        .user_as(Some(&bearer(&user_token)))
        .list(pb::ListUsersRequest {
            after: None,
            limit: None,
        })
        .await
        .unwrap_err();
    assert_eq!(err.code(), Code::PermissionDenied);
    assert_eq!(error_reason(&err), "requires_global_admin");

    // 6. DB 権限不足 → insufficient_privilege
    let err = test_app
        .database_as(Some(&bearer(&user_token)))
        .get(pb::GetDatabaseRequest {
            db_name: "code_db".to_string(),
        })
        .await
        .unwrap_err();
    assert_eq!(err.code(), Code::PermissionDenied);
    assert_eq!(error_reason(&err), "insufficient_privilege");

    // 7. root の削除 → root_protected
    let err = test_app
        .user_as(Some(&bearer(&root_token)))
        .delete(pb::DeleteUserRequest {
            username: "root".to_string(),
        })
        .await
        .unwrap_err();
    assert_eq!(err.code(), Code::PermissionDenied);
    assert_eq!(error_reason(&err), "root_protected");
}

#[tokio::test]
/// Global Adminがデータベースを作成できるかを検証する。
async fn test_global_admin_privileges() {
    let test_app = TestApp::new().await;
    let root_token = test_app.root_token().to_string();

    let admin_token = create_user_and_token(&test_app, &root_token, "admin_user", true).await;

    // Admin should be able to create a DB
    test_app
        .database_as(Some(&bearer(&admin_token)))
        .create(pb::CreateDatabaseRequest {
            name: "admin_db".to_string(),
            description: None,
        })
        .await
        .expect("admin should be able to create a database");
}

#[tokio::test]
/// Manage権限を持つユーザーがDB作成はできず、テーブル作成はできるかを検証する。
async fn test_manage_privileges() {
    let test_app = TestApp::new().await;
    let root_token = test_app.root_token().to_string();

    create_db(&test_app, &root_token, "test_db").await;

    let user_token = create_user_and_token(&test_app, &root_token, "manage_user", false).await;
    grant_privilege(&test_app, &root_token, "manage_user", "test_db", "manage").await;

    // Manage user cannot create another DB
    let err = test_app
        .database_as(Some(&bearer(&user_token)))
        .create(pb::CreateDatabaseRequest {
            name: "test_db_2".to_string(),
            description: None,
        })
        .await
        .unwrap_err();
    assert_eq!(err.code(), Code::PermissionDenied);

    // Manage user CAN create a table in their DB
    create_table(&test_app, &user_token, "test_db", "t1").await;
}

#[tokio::test]
/// Write権限を持つユーザーがテーブル作成はできず、データ挿入はできるかを検証する。
async fn test_write_privileges() {
    let test_app = TestApp::new().await;
    let root_token = test_app.root_token().to_string();

    create_db(&test_app, &root_token, "test_db").await;
    create_table(&test_app, &root_token, "test_db", "t1").await;

    let user_token = create_user_and_token(&test_app, &root_token, "write_user", false).await;
    grant_privilege(&test_app, &root_token, "write_user", "test_db", "write").await;

    // Write user CANNOT create a table
    let err = post_table(&test_app, &user_token, "test_db", "t2")
        .await
        .unwrap_err();
    assert_eq!(err.code(), Code::PermissionDenied);

    // Write user CAN insert data
    test_app
        .data_as(Some(&bearer(&user_token)))
        .insert(pb::InsertDataRequest {
            db_name: "test_db".to_string(),
            table_name: "t1".to_string(),
            value: Some(num(10.0)),
            spatial_ids: vec![single_id(0, 0, 0, 0)],
            zoom_level_policy: pb::ZoomLevelPolicy::Error as i32,
        })
        .await
        .expect("write user should be able to insert data");
}

#[tokio::test]
/// Read権限を持つユーザーがデータ挿入はできず、データ取得はできるかを検証する。
async fn test_read_privileges() {
    let test_app = TestApp::new().await;
    let root_token = test_app.root_token().to_string();

    create_db(&test_app, &root_token, "test_db").await;
    create_table(&test_app, &root_token, "test_db", "t1").await;
    test_app
        .data_as(Some(&bearer(&root_token)))
        .insert(pb::InsertDataRequest {
            db_name: "test_db".to_string(),
            table_name: "t1".to_string(),
            value: Some(num(10.0)),
            spatial_ids: vec![single_id(0, 0, 0, 0)],
            zoom_level_policy: pb::ZoomLevelPolicy::Error as i32,
        })
        .await
        .unwrap();

    let user_token = create_user_and_token(&test_app, &root_token, "read_user", false).await;
    grant_privilege(&test_app, &root_token, "read_user", "test_db", "read").await;

    // Read user CANNOT insert data
    let err = test_app
        .data_as(Some(&bearer(&user_token)))
        .insert(pb::InsertDataRequest {
            db_name: "test_db".to_string(),
            table_name: "t1".to_string(),
            value: Some(num(20.0)),
            spatial_ids: vec![single_id(1, 0, 0, 0)],
            zoom_level_policy: pb::ZoomLevelPolicy::Error as i32,
        })
        .await
        .unwrap_err();
    assert_eq!(err.code(), Code::PermissionDenied);

    // Read user CAN get data
    test_app
        .data_as(Some(&bearer(&user_token)))
        .search(pb::SearchDataRequest {
            db_name: "test_db".to_string(),
            table_name: "t1".to_string(),
            spatial_ids: vec![single_id(0, 0, 0, 0)],
            zoom_level_policy: pb::ZoomLevelPolicy::Error as i32,
            format: pb::OutputFormat::SingleId as i32,
            limit: None,
        })
        .await
        .expect("read user should be able to search data");
}

#[tokio::test]
/// データベース一覧および詳細取得が、ユーザーの権限に応じて正しくフィルタリングされるかを検証する。
async fn test_database_list_and_info_authorization() {
    let test_app = TestApp::new().await;
    let root_token = test_app.root_token().to_string();

    // root creates two databases
    for name in ["visible_db", "hidden_db"] {
        create_db(&test_app, &root_token, name).await;
    }

    // A non-admin user with Read on visible_db only
    let user_token = create_user_and_token(&test_app, &root_token, "viewer", false).await;
    grant_privilege(&test_app, &root_token, "viewer", "visible_db", "read").await;

    // ListDatabases returns only the database the user can access
    let dbs = test_app
        .database_as(Some(&bearer(&user_token)))
        .list(pb::ListDatabasesRequest {})
        .await
        .expect("list should succeed")
        .into_inner()
        .databases;
    let names: Vec<String> = dbs.iter().map(|d| d.name.clone()).collect();
    assert_eq!(names, vec!["visible_db".to_string()]);

    // Get allowed for the database the user can read
    test_app
        .database_as(Some(&bearer(&user_token)))
        .get(pb::GetDatabaseRequest {
            db_name: "visible_db".to_string(),
        })
        .await
        .expect("visible_db should be readable");

    // Get forbidden for a database the user has no privilege on
    let err = test_app
        .database_as(Some(&bearer(&user_token)))
        .get(pb::GetDatabaseRequest {
            db_name: "hidden_db".to_string(),
        })
        .await
        .unwrap_err();
    assert_eq!(err.code(), Code::PermissionDenied);

    // GlobalAdmin (root) sees all databases
    let dbs = test_app
        .database_as(Some(&bearer(&root_token)))
        .list(pb::ListDatabasesRequest {})
        .await
        .expect("list should succeed")
        .into_inner()
        .databases;
    assert_eq!(dbs.len(), 2);
}

/// データベースのManage権限を持つユーザーが、他ユーザーへ権限を付与しようとすると拒否されることを検証する。
#[tokio::test]
async fn test_manage_user_can_set_privileges() {
    let test_app = TestApp::new().await;
    let root_token = test_app.root_token().to_string();

    // 1. データベースを作成する
    create_db(&test_app, &root_token, "test_db").await;

    // 2. manage_user と normal_user を作成する
    let manage_token = create_user_and_token(&test_app, &root_token, "manage_user", false).await;
    let _normal_token = create_user_and_token(&test_app, &root_token, "normal_user", false).await;

    // 3. manage_user に Manage 権限を付与する（rootとして）
    grant_privilege(&test_app, &root_token, "manage_user", "test_db", "manage").await;

    // 4. manage_user が normal_user に Read 権限を付与しようとする（global の admin のみ可能なため失敗するはず）
    let err = put_privilege(
        &test_app,
        &manage_token,
        "normal_user",
        Some("test_db"),
        None,
        "read",
    )
    .await
    .unwrap_err();
    assert_eq!(err.code(), Code::PermissionDenied);

    // 5. normal_user が Read 権限を持っていないことを検証する
    let user = test_app
        .user_as(Some(&bearer(&root_token)))
        .get(pb::GetUserRequest {
            username: "normal_user".to_string(),
        })
        .await
        .expect("get user failed")
        .into_inner();
    assert!(user.privileges.is_empty());
}

#[tokio::test]
/// 権限を持たないユーザーがデータベース内のデータにアクセスできないかを検証する。
async fn test_no_privileges() {
    let test_app = TestApp::new().await;
    let root_token = test_app.root_token().to_string();

    create_db(&test_app, &root_token, "test_db").await;
    create_table(&test_app, &root_token, "test_db", "t1").await;

    let user_token = create_user_and_token(&test_app, &root_token, "no_user", false).await;

    // No user CANNOT get data
    let err = test_app
        .data_as(Some(&bearer(&user_token)))
        .search(pb::SearchDataRequest {
            db_name: "test_db".to_string(),
            table_name: "t1".to_string(),
            spatial_ids: vec![single_id(0, 0, 0, 0)],
            zoom_level_policy: pb::ZoomLevelPolicy::Error as i32,
            format: pb::OutputFormat::SingleId as i32,
            limit: None,
        })
        .await
        .unwrap_err();
    assert_eq!(err.code(), Code::PermissionDenied);
}

#[tokio::test]
/// `Query` はクエリ式が参照する**すべてのデータベース**に Read 以上の権限を要求する。
/// 一部のソースにしか権限が無い場合は、他のソースにデータがあっても Forbidden で拒否されなければならない。
async fn test_query_authorization() {
    let test_app = TestApp::new().await;
    let root_token = test_app.root_token().to_string();

    // Setup two DBs, each with a table and data, using root.
    for (db, table) in [("db_a", "t_a"), ("db_b", "t_b")] {
        create_db(&test_app, &root_token, db).await;
        create_table(&test_app, &root_token, db, table).await;
        test_app
            .data_as(Some(&bearer(&root_token)))
            .insert(pb::InsertDataRequest {
                db_name: db.to_string(),
                table_name: table.to_string(),
                value: Some(num(10.0)),
                spatial_ids: vec![single_id(0, 0, 0, 0)],
                zoom_level_policy: pb::ZoomLevelPolicy::Error as i32,
            })
            .await
            .unwrap();
    }

    // User only has Read on db_a.
    let user_token = create_user_and_token(&test_app, &root_token, "query_user", false).await;
    grant_privilege(&test_app, &root_token, "query_user", "db_a", "read").await;

    // Querying db_a alone is allowed.
    test_app
        .query_as(Some(&bearer(&user_token)))
        .execute(pb::ExecuteQueryRequest {
            value_type: None,
            spatial_ids: vec![single_id(0, 0, 0, 0)],
            query: Some(source("db_a", "t_a")),
            format: pb::OutputFormat::SingleId as i32,
            limit: None,
        })
        .await
        .expect("querying db_a alone should be allowed");

    // A query merging db_a (readable) with db_b (no privilege) must be rejected,
    // even though the db_a source alone would be allowed.
    let err = test_app
        .query_as(Some(&bearer(&user_token)))
        .execute(pb::ExecuteQueryRequest {
            value_type: None,
            spatial_ids: vec![single_id(0, 0, 0, 0)],
            query: Some(merge(
                source("db_a", "t_a"),
                source("db_b", "t_b"),
                num(0.0),
                pb::MergePolicyKind::Sum,
            )),
            format: pb::OutputFormat::SingleId as i32,
            limit: None,
        })
        .await
        .unwrap_err();
    assert_eq!(err.code(), Code::PermissionDenied);
}

#[tokio::test]
/// パスワード変更時に、既存のセッション（トークン）が失効し再ログインが要求されるかを検証する。
async fn test_password_change_revokes_tokens() {
    let test_app = TestApp::new().await;
    let root_token = test_app.root_token().to_string();

    let user_token = create_user_and_token(&test_app, &root_token, "rotate_user", false).await;
    assert!(token_is_valid(&test_app, &user_token).await);

    // root がパスワードを変更すると、既存トークンは失効する
    test_app
        .user_as(Some(&bearer(&root_token)))
        .update_password(pb::UpdatePasswordRequest {
            username: "rotate_user".to_string(),
            password: "newpassword".to_string(),
        })
        .await
        .expect("update_password failed");

    assert!(!token_is_valid(&test_app, &user_token).await);
}

#[tokio::test]
/// 管理者権限の剥奪時にトークンが失効し、rootの権限は変更できないことを検証する。
async fn test_admin_demotion_and_root_protection() {
    let test_app = TestApp::new().await;
    let root_token = test_app.root_token().to_string();

    // 管理者ユーザーを作成
    let admin_token = create_user_and_token(&test_app, &root_token, "demo_admin", true).await;

    // 管理者は DB を作成できる
    test_app
        .database_as(Some(&bearer(&admin_token)))
        .create(pb::CreateDatabaseRequest {
            name: "admin_db".to_string(),
            description: None,
        })
        .await
        .expect("admin should be able to create a database");

    // root が管理者権限を剥奪 → DB 作成は Forbidden
    delete_privilege(&test_app, &root_token, "demo_admin", None, None)
        .await
        .expect("revoke should succeed");

    let err = test_app
        .database_as(Some(&bearer(&admin_token)))
        .create(pb::CreateDatabaseRequest {
            name: "admin_db_fail".to_string(),
            description: None,
        })
        .await
        .unwrap_err();
    assert_eq!(err.code(), Code::PermissionDenied);

    // 再ログインすると管理者ではなくなっている（DB 作成は Forbidden）
    create_user_and_token(&test_app, &root_token, "demo_admin2", true).await;
    delete_privilege(&test_app, &root_token, "demo_admin2", None, None)
        .await
        .expect("revoke should succeed");
    // 再ログイン
    let new_token = test_app
        .auth()
        .login(pb::LoginRequest {
            username: "demo_admin2".to_string(),
            password: "password".to_string(),
        })
        .await
        .expect("login failed")
        .into_inner()
        .token;
    let err = test_app
        .database_as(Some(&bearer(&new_token)))
        .create(pb::CreateDatabaseRequest {
            name: "should_fail".to_string(),
            description: None,
        })
        .await
        .unwrap_err();
    assert_eq!(err.code(), Code::PermissionDenied);

    // root の管理者権限は変更できない
    let err = delete_privilege(&test_app, &root_token, "root", None, None)
        .await
        .unwrap_err();
    assert_eq!(err.code(), Code::PermissionDenied);
}

#[tokio::test]
/// ユーザー削除後に同名ユーザーを再作成した場合、旧ユーザーのトークンが無効になるかを検証する。
async fn test_username_reuse_rejects_old_token() {
    let test_app = TestApp::new().await;
    let root_token = test_app.root_token().to_string();

    // ユーザーを作成しトークンを取得
    let old_token = create_user_and_token(&test_app, &root_token, "reuse_user", false).await;
    assert!(token_is_valid(&test_app, &old_token).await);

    // ユーザーを削除
    test_app
        .user_as(Some(&bearer(&root_token)))
        .delete(pb::DeleteUserRequest {
            username: "reuse_user".to_string(),
        })
        .await
        .expect("delete_user failed");
    assert!(!token_is_valid(&test_app, &old_token).await);

    // 同名ユーザーを再作成 → 旧トークンは別 UUID のため無効のまま
    create_user_and_token(&test_app, &root_token, "reuse_user", false).await;
    assert!(!token_is_valid(&test_app, &old_token).await);
}

#[tokio::test]
/// ログイン済みユーザーがサーバーのステータスとバージョン情報を取得できるか検証する。
/// 未ログインの場合は Unauthenticated (missing_token) で拒否されること。
async fn test_get_system_info() {
    let test_app = TestApp::new().await;
    let root_token = test_app.root_token().to_string();

    // 1. 未ログイン（ヘッダーなし）→ Unauthenticated (missing_token)
    let err = test_app.system_as(None).get_info(()).await.unwrap_err();
    assert_eq!(err.code(), Code::Unauthenticated);
    assert_eq!(error_reason(&err), "missing_token");

    // 2. ログイン済み（rootユーザー）→ 成功
    let info = test_app
        .system_as(Some(&bearer(&root_token)))
        .get_info(())
        .await
        .expect("get_info failed")
        .into_inner();
    assert_eq!(info.status, "ok");
    assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
}

#[tokio::test]
/// `GetUser` のエンドポイントが、本人またはGlobal Adminのみに許可されているかを検証する。
async fn test_get_user_info_authorization() {
    let test_app = TestApp::new().await;
    let root_token = test_app.root_token().to_string();

    // ユーザーA（一般ユーザー）を作成
    let user_a_token = create_user_and_token(&test_app, &root_token, "user_a", false).await;
    // ユーザーB（一般ユーザー）を作成
    let user_b_token = create_user_and_token(&test_app, &root_token, "user_b", false).await;
    // ユーザーC（管理者）を作成
    let admin_token = create_user_and_token(&test_app, &root_token, "admin_user", true).await;

    // 1. 本人が自分自身の情報を取得できるか
    let user = test_app
        .user_as(Some(&bearer(&user_a_token)))
        .get(pb::GetUserRequest {
            username: "user_a".to_string(),
        })
        .await
        .expect("user_a should be able to read their own info")
        .into_inner();
    assert_eq!(user.username, "user_a");
    assert!(user.privileges.is_empty());

    // 2. 他人（非管理者）が情報を取得しようとすると失敗するか
    let err = test_app
        .user_as(Some(&bearer(&user_b_token)))
        .get(pb::GetUserRequest {
            username: "user_a".to_string(),
        })
        .await
        .unwrap_err();
    assert_eq!(err.code(), Code::PermissionDenied);

    // 3. 管理者が他人の情報を取得できるか
    test_app
        .user_as(Some(&bearer(&admin_token)))
        .get(pb::GetUserRequest {
            username: "user_a".to_string(),
        })
        .await
        .expect("admin should be able to read other users' info");

    // 4. 存在しないユーザーの情報を取得しようとすると失敗するか
    let err = test_app
        .user_as(Some(&bearer(&admin_token)))
        .get(pb::GetUserRequest {
            username: "non_existent_user".to_string(),
        })
        .await
        .unwrap_err();
    assert_eq!(err.code(), Code::NotFound);
}

#[tokio::test]
/// データベースのリネーム、コピー、およびテーブルコピーにおける権限検証をテストする。
async fn test_copy_and_rename_permissions() {
    let test_app = TestApp::new().await;
    let root_token = test_app.root_token().to_string();

    // 1. データベース src_db を作成し、一般ユーザー user_a に Read 権限、user_b に Manage 権限を付与する。
    create_db(&test_app, &root_token, "src_db").await;

    let user_a_token = create_user_and_token(&test_app, &root_token, "user_a", false).await;
    let user_b_token = create_user_and_token(&test_app, &root_token, "user_b", false).await;

    // user_a に Read 権限を付与
    grant_privilege(&test_app, &root_token, "user_a", "src_db", "read").await;

    // user_b に Manage 権限を付与
    grant_privilege(&test_app, &root_token, "user_b", "src_db", "manage").await;

    // 2. データベースのRename権限テスト
    // user_a (Read) はリネームできないはず
    let err = test_app
        .database_as(Some(&bearer(&user_a_token)))
        .update(pb::UpdateDatabaseRequest {
            db_name: "src_db".to_string(),
            new_name: Some("renamed_db".to_string()),
            description: None,
        })
        .await
        .unwrap_err();
    assert_eq!(err.code(), Code::PermissionDenied);

    // user_b (Manage) はリネームできるはず
    test_app
        .database_as(Some(&bearer(&user_b_token)))
        .update(pb::UpdateDatabaseRequest {
            db_name: "src_db".to_string(),
            new_name: Some("renamed_db".to_string()),
            description: None,
        })
        .await
        .expect("user_b (manage) should be able to rename");

    // renamed_db に名前が変更されたので、以降は renamed_db を対象とする。
    // user_a (Read) の権限が renamed_db に引き継がれていることを確認
    // user_a が renamed_db/copy を叩くと、Global Admin ではないので Forbidden になる。
    let err = test_app
        .database_as(Some(&bearer(&user_a_token)))
        .copy(pb::CopyDatabaseRequest {
            db_name: "renamed_db".to_string(),
            copy_name: "copied_db".to_string(),
        })
        .await
        .unwrap_err();
    assert_eq!(err.code(), Code::PermissionDenied);

    // root が renamed_db/copy を叩いて成功させる。
    test_app
        .database_as(Some(&bearer(&root_token)))
        .copy(pb::CopyDatabaseRequest {
            db_name: "renamed_db".to_string(),
            copy_name: "copied_db".to_string(),
        })
        .await
        .expect("root should be able to copy");

    // テスト継続のため root (Global Admin) が user_a に copied_db の Manage 権限を付与する。
    grant_privilege(&test_app, &root_token, "user_a", "copied_db", "manage").await;

    // 3. コピー先データベース (copied_db) に対する user_a の Manage 権限を検証
    // 手動で付与した Manage 権限により、user_a は copied_db にテーブルを作成できるはず。
    create_table(&test_app, &user_a_token, "copied_db", "new_table").await;

    // 4. テーブルコピーの権限テスト
    // user_b (元src_dbのManageだったがリネームされてrenamed_dbのManage) は copied_db に対する権限を持たないため、
    // renamed_db から copied_db へのテーブルコピーは失敗するはず（copied_db の Manage 権限がないため）。
    let err = test_app
        .table_as(Some(&bearer(&user_b_token)))
        .copy(pb::CopyTableRequest {
            db_name: "renamed_db".to_string(),
            table_name: "new_table".to_string(),
            copy_db_name: Some("copied_db".to_string()),
            copy_table_name: "copied_table".to_string(),
        })
        .await
        .unwrap_err();
    assert_eq!(err.code(), Code::PermissionDenied);
}

#[tokio::test]
/// データベースを削除して同じ名前で作り直しても、旧データベースへの権限が
/// 新しいデータベースに効かないことを検証する（権限は名前ではなく ID に紐づく）。
async fn test_privileges_do_not_survive_delete_and_recreate() {
    let test_app = TestApp::new().await;
    let root_token = test_app.root_token().to_string();

    create_db(&test_app, &root_token, "target_db").await;
    let user_token = create_user_and_token(&test_app, &root_token, "grantee", false).await;
    grant_privilege(&test_app, &root_token, "grantee", "target_db", "manage").await;

    // 付与直後はアクセスできる
    test_app
        .database_as(Some(&bearer(&user_token)))
        .get(pb::GetDatabaseRequest {
            db_name: "target_db".to_string(),
        })
        .await
        .expect("should be accessible right after granting");

    // データベースを削除し、同じ名前で作り直す
    test_app
        .database_as(Some(&bearer(&root_token)))
        .delete(pb::DeleteDatabaseRequest {
            db_name: "target_db".to_string(),
        })
        .await
        .expect("delete_database failed");
    create_db(&test_app, &root_token, "target_db").await;

    // 旧権限は新しい target_db には効かない
    let err = test_app
        .database_as(Some(&bearer(&user_token)))
        .get(pb::GetDatabaseRequest {
            db_name: "target_db".to_string(),
        })
        .await
        .unwrap_err();
    assert_eq!(err.code(), Code::PermissionDenied);

    // 解決できなくなったルールは権限一覧にも現れない
    let privileges = fetch_privileges(&test_app, &root_token, "grantee").await;
    assert!(privileges.is_empty());
}

#[tokio::test]
/// データベースを改名しても権限が追従することを検証する。
async fn test_privileges_follow_database_rename() {
    let test_app = TestApp::new().await;
    let root_token = test_app.root_token().to_string();

    create_db(&test_app, &root_token, "before_db").await;
    let user_token = create_user_and_token(&test_app, &root_token, "follower", false).await;
    grant_privilege(&test_app, &root_token, "follower", "before_db", "read").await;

    test_app
        .database_as(Some(&bearer(&root_token)))
        .update(pb::UpdateDatabaseRequest {
            db_name: "before_db".to_string(),
            new_name: Some("after_db".to_string()),
            description: None,
        })
        .await
        .expect("rename should succeed");

    // 改名先に権限が追従している
    test_app
        .database_as(Some(&bearer(&user_token)))
        .get(pb::GetDatabaseRequest {
            db_name: "after_db".to_string(),
        })
        .await
        .expect("privilege should follow the rename");

    // 権限一覧の表示も新しい名前になっている
    let privileges = fetch_privileges(&test_app, &root_token, "follower").await;
    assert_eq!(privileges.len(), 1);
    match &privileges[0].scope {
        Some(pb::privilege_rule::Scope::Database(d)) => assert_eq!(d.db_name, "after_db"),
        other => panic!("expected a Database-scoped rule, got {other:?}"),
    }
}

#[tokio::test]
/// 全データベースへの Manage（global/manage）と、サーバー管理者（global/admin）が
/// 別の権限であることを検証する。
async fn test_global_manage_is_not_server_admin() {
    let test_app = TestApp::new().await;
    let root_token = test_app.root_token().to_string();

    create_db(&test_app, &root_token, "shared_db").await;
    let token = create_user_and_token(&test_app, &root_token, "data_manager", false).await;
    put_privilege(&test_app, &root_token, "data_manager", None, None, "manage")
        .await
        .expect("grant should succeed");

    // データ面: 全データベースを Manage できる
    create_table(&test_app, &token, "shared_db", "managed").await;

    // 制御面: ユーザー一覧も、権限付与もできない
    let err = test_app
        .user_as(Some(&bearer(&token)))
        .list(pb::ListUsersRequest {
            after: None,
            limit: None,
        })
        .await
        .unwrap_err();
    assert_eq!(err.code(), Code::PermissionDenied);

    let err = put_privilege(&test_app, &token, "data_manager", None, None, "admin")
        .await
        .unwrap_err();
    assert_eq!(err.code(), Code::PermissionDenied);
}

#[tokio::test]
/// `admin` ロールは global スコープ以外では受け付けられないことを検証する。
///
/// データベース・テーブルスコープのリクエストは `DataRole`（`admin` を持たない）
/// しか表現できないため、gRPC 化後はこの制約が実行時の検証ではなく型で保証される
/// （`pb::DataRole` に `Admin` バリアントが無いので、そもそも該当リクエストを構築できない）。
/// そのため以前あった「データ/テーブルスコープへの `admin` 指定は 422」という検証対象は
/// 消滅し、global スコープでだけ通ることの確認だけが残る。
async fn test_admin_role_is_rejected_outside_global_scope() {
    let test_app = TestApp::new().await;
    let root_token = test_app.root_token().to_string();

    create_db(&test_app, &root_token, "scoped_db").await;
    create_table(&test_app, &root_token, "scoped_db", "scoped_table").await;
    create_user_and_token(&test_app, &root_token, "climber", false).await;

    // global スコープでだけ通る。
    put_privilege(&test_app, &root_token, "climber", None, None, "admin")
        .await
        .expect("admin should be settable on the global scope");

    // 何も保存されていないこと（データスコープ側）を確認する。
    let privileges = fetch_privileges(&test_app, &root_token, "climber").await;
    assert_eq!(privileges.len(), 1);
    assert!(matches!(
        &privileges[0].scope,
        Some(pb::privilege_rule::Scope::Global(_))
    ));
}

#[tokio::test]
/// 存在しないデータベース・テーブルへの権限付与が拒否されることを検証する
/// （タイポの黙殺と、将来作られる同名オブジェクトへの事前付与の両方を防ぐ）。
async fn test_privileges_on_unknown_targets_are_rejected() {
    let test_app = TestApp::new().await;
    let root_token = test_app.root_token().to_string();

    create_db(&test_app, &root_token, "real_db").await;
    create_user_and_token(&test_app, &root_token, "typo_user", false).await;

    let err = put_privilege(
        &test_app,
        &root_token,
        "typo_user",
        Some("raal_db"),
        None,
        "read",
    )
    .await
    .unwrap_err();
    assert_eq!(err.code(), Code::NotFound);
    assert_eq!(error_reason(&err), "database_not_found");

    let err = put_privilege(
        &test_app,
        &root_token,
        "typo_user",
        Some("real_db"),
        Some("ghost"),
        "read",
    )
    .await
    .unwrap_err();
    assert_eq!(err.code(), Code::NotFound);
    assert_eq!(error_reason(&err), "table_not_found");

    // 何も保存されていない
    assert!(
        fetch_privileges(&test_app, &root_token, "typo_user")
            .await
            .is_empty()
    );
}

#[tokio::test]
/// 同じ対象への再設定が「2 件目の追加」ではなく「置き換え」になることを検証する。
///
/// 対象がパスのキーなので、同一対象に複数のルールが並ぶこと自体が表現できない。
async fn test_setting_same_target_twice_replaces_the_rule() {
    let test_app = TestApp::new().await;
    let root_token = test_app.root_token().to_string();

    create_db(&test_app, &root_token, "dup_db").await;
    create_user_and_token(&test_app, &root_token, "dup_user", false).await;

    grant_privilege(&test_app, &root_token, "dup_user", "dup_db", "manage").await;
    grant_privilege(&test_app, &root_token, "dup_user", "dup_db", "read").await;

    let privileges = fetch_privileges(&test_app, &root_token, "dup_user").await;
    assert_eq!(privileges.len(), 1);
    match &privileges[0].scope {
        Some(pb::privilege_rule::Scope::Database(d)) => {
            assert_eq!(d.role, pb::DataRole::Read as i32)
        }
        other => panic!("expected a Database-scoped rule, got {other:?}"),
    }
}

#[tokio::test]
/// 同じ対象への再設定によるロールの降格が実際に効くことを検証する。
async fn test_role_downgrade_takes_effect() {
    let test_app = TestApp::new().await;
    let root_token = test_app.root_token().to_string();

    create_db(&test_app, &root_token, "demote_db").await;
    let token = create_user_and_token(&test_app, &root_token, "demoted", false).await;
    grant_privilege(&test_app, &root_token, "demoted", "demote_db", "manage").await;

    // Manage のうちはテーブルを作れる
    create_table(&test_app, &token, "demote_db", "before_demote").await;

    // Read へ降格する
    grant_privilege(&test_app, &root_token, "demoted", "demote_db", "read").await;

    // 降格が効いており、テーブルを作れない
    let err = post_table(&test_app, &token, "demote_db", "after_demote")
        .await
        .unwrap_err();
    assert_eq!(err.code(), Code::PermissionDenied);

    // 権限は 1 件だけで、ロールは read
    let privileges = fetch_privileges(&test_app, &root_token, "demoted").await;
    assert_eq!(privileges.len(), 1);
    match &privileges[0].scope {
        Some(pb::privilege_rule::Scope::Database(d)) => {
            assert_eq!(d.role, pb::DataRole::Read as i32)
        }
        other => panic!("expected a Database-scoped rule, got {other:?}"),
    }
}

#[tokio::test]
/// 別々の対象に対する操作が互いに干渉しないことを検証する。
///
/// 権限セット全体を送る API では、古い一覧をもとにした付与が他者の剥奪を巻き戻し得た。
/// 対象ごとの操作なら、そもそも現在の一覧を知らずに書けるのでその事故が起きない。
async fn test_operations_on_distinct_targets_do_not_interfere() {
    let test_app = TestApp::new().await;
    let root_token = test_app.root_token().to_string();

    create_db(&test_app, &root_token, "db_one").await;
    create_db(&test_app, &root_token, "db_two").await;
    create_user_and_token(&test_app, &root_token, "shared", false).await;

    grant_privilege(&test_app, &root_token, "shared", "db_one", "read").await;
    grant_privilege(&test_app, &root_token, "shared", "db_two", "manage").await;

    // db_two を剥奪したあとに db_one を触っても、剥奪は巻き戻らない。
    delete_privilege(&test_app, &root_token, "shared", Some("db_two"), None)
        .await
        .expect("revoke should succeed");
    grant_privilege(&test_app, &root_token, "shared", "db_one", "manage").await;

    let privileges = fetch_privileges(&test_app, &root_token, "shared").await;
    assert_eq!(privileges.len(), 1);
    match &privileges[0].scope {
        Some(pb::privilege_rule::Scope::Database(d)) => {
            assert_eq!(d.db_name, "db_one");
            assert_eq!(d.role, pb::DataRole::Manage as i32);
        }
        other => panic!("expected a Database-scoped rule, got {other:?}"),
    }
}

#[tokio::test]
/// 剥奪はロールを問わず対象ごと落ちること、権限が無ければ NotFound になることを検証する。
async fn test_revoke_targets_the_object_not_the_role() {
    let test_app = TestApp::new().await;
    let root_token = test_app.root_token().to_string();

    create_db(&test_app, &root_token, "rev_db").await;
    create_user_and_token(&test_app, &root_token, "revokee", false).await;

    grant_privilege(&test_app, &root_token, "revokee", "rev_db", "read").await;

    // ロールを指定しないので、Read だろうが Manage だろうが確実に落ちる。
    delete_privilege(&test_app, &root_token, "revokee", Some("rev_db"), None)
        .await
        .expect("revoke should succeed");
    assert!(
        fetch_privileges(&test_app, &root_token, "revokee")
            .await
            .is_empty()
    );

    // 持っていない権限の剥奪は NotFound（黙って成功しない）。
    let err = delete_privilege(&test_app, &root_token, "revokee", Some("rev_db"), None)
        .await
        .unwrap_err();
    assert_eq!(err.code(), Code::NotFound);
}

#[tokio::test]
/// テーブルスコープの Manage が、そのテーブル以外を作る踏み台にならないことを検証する。
async fn test_table_scope_manage_cannot_create_other_tables() {
    let test_app = TestApp::new().await;
    let root_token = test_app.root_token().to_string();

    create_db(&test_app, &root_token, "box_db").await;
    create_table(&test_app, &root_token, "box_db", "scratch").await;

    let token = create_user_and_token(&test_app, &root_token, "boxed", false).await;
    grant_table_privilege(
        &test_app,
        &root_token,
        "boxed",
        "box_db",
        "scratch",
        "manage",
    )
    .await;

    // 直接の新規テーブル作成はデータベースレベルの Manage が要るので拒否される
    let err = post_table(&test_app, &token, "box_db", "sneaked")
        .await
        .unwrap_err();
    assert_eq!(err.code(), Code::PermissionDenied);

    // コピー経由でも同じ。コピー先の権限判定にコピー元テーブル名を使ってはならない。
    let err = test_app
        .table_as(Some(&bearer(&token)))
        .copy(pb::CopyTableRequest {
            db_name: "box_db".to_string(),
            table_name: "scratch".to_string(),
            copy_db_name: None,
            copy_table_name: "sneaked".to_string(),
        })
        .await
        .unwrap_err();
    assert_eq!(err.code(), Code::PermissionDenied);

    // 自分のテーブルの管理（削除）はできる
    test_app
        .table_as(Some(&bearer(&token)))
        .delete(pb::DeleteTableRequest {
            db_name: "box_db".to_string(),
            table_name: "scratch".to_string(),
        })
        .await
        .expect("should be able to delete their own table");
}

#[tokio::test]
/// テーブルスコープの権限しか持たないユーザーが、自分のテーブルまで辿り着けることを検証する。
/// データベース一覧・テーブル一覧に現れ、かつ他のテーブルは見えない。
async fn test_table_scope_user_can_discover_own_table() {
    let test_app = TestApp::new().await;
    let root_token = test_app.root_token().to_string();

    create_db(&test_app, &root_token, "visible_db").await;
    create_db(&test_app, &root_token, "hidden_db").await;
    create_table(&test_app, &root_token, "visible_db", "mine").await;
    create_table(&test_app, &root_token, "visible_db", "yours").await;

    let token = create_user_and_token(&test_app, &root_token, "narrow", false).await;
    grant_table_privilege(
        &test_app,
        &root_token,
        "narrow",
        "visible_db",
        "mine",
        "read",
    )
    .await;

    // データベース一覧には visible_db だけが出る
    let dbs = test_app
        .database_as(Some(&bearer(&token)))
        .list(pb::ListDatabasesRequest {})
        .await
        .expect("list should succeed")
        .into_inner()
        .databases;
    let names: Vec<&str> = dbs.iter().map(|d| d.name.as_str()).collect();
    assert_eq!(names, vec!["visible_db"]);

    // データベース情報も取得できる
    test_app
        .database_as(Some(&bearer(&token)))
        .get(pb::GetDatabaseRequest {
            db_name: "visible_db".to_string(),
        })
        .await
        .expect("visible_db should be readable");
    let err = test_app
        .database_as(Some(&bearer(&token)))
        .get(pb::GetDatabaseRequest {
            db_name: "hidden_db".to_string(),
        })
        .await
        .unwrap_err();
    assert_eq!(err.code(), Code::PermissionDenied);

    // テーブル一覧は自分が読めるものだけに絞られる
    let tables = test_app
        .table_as(Some(&bearer(&token)))
        .list(pb::ListTablesRequest {
            db_name: "visible_db".to_string(),
        })
        .await
        .expect("list should succeed")
        .into_inner()
        .tables;
    let table_names: Vec<&str> = tables.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(table_names, vec!["mine"]);

    // 権限のないテーブルの詳細は取得できない
    test_app
        .table_as(Some(&bearer(&token)))
        .get(pb::GetTableRequest {
            db_name: "visible_db".to_string(),
            table_name: "mine".to_string(),
        })
        .await
        .expect("mine should be readable");
    let err = test_app
        .table_as(Some(&bearer(&token)))
        .get(pb::GetTableRequest {
            db_name: "visible_db".to_string(),
            table_name: "yours".to_string(),
        })
        .await
        .unwrap_err();
    assert_eq!(err.code(), Code::PermissionDenied);
}

#[tokio::test]
/// `Query` がテーブル単位で認可されることを検証する。
/// テーブルスコープの権限しか無くても、そのテーブルだけを参照するクエリは実行できる。
async fn test_query_authorization_is_table_scoped() {
    let test_app = TestApp::new().await;
    let root_token = test_app.root_token().to_string();

    create_db(&test_app, &root_token, "q_db").await;
    create_table(&test_app, &root_token, "q_db", "allowed").await;
    create_table(&test_app, &root_token, "q_db", "denied").await;

    let token = create_user_and_token(&test_app, &root_token, "querier", false).await;
    grant_table_privilege(&test_app, &root_token, "querier", "q_db", "allowed", "read").await;

    let request = |table: &str| pb::ExecuteQueryRequest {
        value_type: Some(pb::TableDataType::Int as i32),
        spatial_ids: vec![range_id(5, Some((0, 0)), Some((0, 1)), Some((0, 1)))],
        query: Some(source("q_db", table)),
        format: pb::OutputFormat::Unspecified as i32,
        limit: None,
    };

    // 権限のあるテーブルへのクエリは通る
    test_app
        .query_as(Some(&bearer(&token)))
        .execute(request("allowed"))
        .await
        .expect("querying the allowed table should succeed");

    // 権限のないテーブルは同じデータベース内でも拒否される
    let err = test_app
        .query_as(Some(&bearer(&token)))
        .execute(request("denied"))
        .await
        .unwrap_err();
    assert_eq!(err.code(), Code::PermissionDenied);
}

#[tokio::test]
/// 存在しないテーブルを参照したときの応答が、単一テーブル経路と `Query` で一致することを検証する。
///
/// データベースレベルの権限を持つユーザーには「権限がない」ではなく
/// 「テーブルが無い」と伝わらなければならない。
async fn test_missing_table_reports_not_found_consistently() {
    let test_app = TestApp::new().await;
    let root_token = test_app.root_token().to_string();

    create_db(&test_app, &root_token, "consistent_db").await;
    let token = create_user_and_token(&test_app, &root_token, "dbmanager", false).await;
    grant_privilege(
        &test_app,
        &root_token,
        "dbmanager",
        "consistent_db",
        "manage",
    )
    .await;

    let request = pb::ExecuteQueryRequest {
        value_type: Some(pb::TableDataType::Int as i32),
        spatial_ids: vec![range_id(5, Some((0, 0)), Some((0, 1)), Some((0, 1)))],
        query: Some(source("consistent_db", "ghost")),
        format: pb::OutputFormat::Unspecified as i32,
        limit: None,
    };

    // 単一テーブル経路: テーブルが無いので NotFound
    let err = test_app
        .table_as(Some(&bearer(&token)))
        .get(pb::GetTableRequest {
            db_name: "consistent_db".to_string(),
            table_name: "ghost".to_string(),
        })
        .await
        .unwrap_err();
    assert_eq!(err.code(), Code::NotFound);

    // `Query` 経路も同じ NotFound でなければならない
    let err = test_app
        .query_as(Some(&bearer(&token)))
        .execute(request.clone())
        .await
        .unwrap_err();
    assert_eq!(err.code(), Code::NotFound);

    // グローバル権限（root）でも NotFound
    let err = test_app
        .query_as(Some(&bearer(&root_token)))
        .execute(request)
        .await
        .unwrap_err();
    assert_eq!(err.code(), Code::NotFound);
}

/// 保存されている生の ACL 行数を数える。
///
/// `GetPrivileges` は解決できない行を隠すため、残留の有無は API からは観測できない。
/// ここでは行を直接数える。
async fn stored_privilege_count(app_state: &kasane::AppState, username: &str) -> usize {
    use kasane::repositories::{CatalogRepository, Storage};
    let username = username.to_string();
    app_state
        .db
        .read(async move |repo| {
            let record = repo.require_user_record(&username).await?;
            Ok(repo.acl_entries(record.id).await?.len())
        })
        .await
        .unwrap()
}

#[tokio::test]
/// 「テーブルの作成 → 権限付与 → 削除」を繰り返しても、削除済みリソースを指す行が
/// 残らないことを検証する。
///
/// 権限は対象ごとの行として持ち、対象を消すときに逆引き索引からその行を落とす。
/// よって残留は繰り返し回数によらず**常に 0 件**になる。
async fn test_stale_privileges_do_not_accumulate_over_cycles() {
    let test_app = TestApp::new().await;
    let root_token = test_app.root_token().to_string();

    create_db(&test_app, &root_token, "churn_db").await;
    create_user_and_token(&test_app, &root_token, "churner", false).await;

    for i in 0..10 {
        let table = format!("tmp_{}", i);
        create_table(&test_app, &root_token, "churn_db", &table).await;
        grant_table_privilege(
            &test_app,
            &root_token,
            "churner",
            "churn_db",
            &table,
            "read",
        )
        .await;
        delete_table(&test_app, &root_token, "churn_db", &table).await;

        // テーブルの削除がその行を落とすので、残留は 0 件。
        assert_eq!(
            stored_privilege_count(test_app.app_state(), "churner").await,
            0,
            "サイクル {} で削除済みテーブルを指す行が残った",
            i
        );
    }

    // 解決できないルールは API 上には現れない。
    assert!(
        fetch_privileges(&test_app, &root_token, "churner")
            .await
            .is_empty()
    );
}

#[tokio::test]
/// `global` スコープの `read` が「全データベース・全テーブルを読めるが一切書けない」
/// 権限として機能することを検証する。
async fn test_global_read_can_read_everything_but_write_nothing() {
    let test_app = TestApp::new().await;
    let root_token = test_app.root_token().to_string();

    create_db(&test_app, &root_token, "alpha_db").await;
    create_db(&test_app, &root_token, "beta_db").await;
    create_table(&test_app, &root_token, "alpha_db", "t_one").await;
    create_table(&test_app, &root_token, "beta_db", "t_two").await;

    let token = create_user_and_token(&test_app, &root_token, "reader", false).await;
    put_privilege(&test_app, &root_token, "reader", None, None, "read")
        .await
        .expect("grant should succeed");

    // データベースは全件見える
    let dbs = test_app
        .database_as(Some(&bearer(&token)))
        .list(pb::ListDatabasesRequest {})
        .await
        .expect("list should succeed")
        .into_inner()
        .databases;
    let names: Vec<String> = dbs.iter().map(|d| d.name.clone()).collect();
    assert_eq!(names, vec!["alpha_db", "beta_db"]);

    // テーブルも全件見える（後から作られたデータベースにも及ぶ）
    create_db(&test_app, &root_token, "gamma_db").await;
    create_table(&test_app, &root_token, "gamma_db", "t_three").await;
    let tables = test_app
        .table_as(Some(&bearer(&token)))
        .list(pb::ListTablesRequest {
            db_name: "gamma_db".to_string(),
        })
        .await
        .expect("list should succeed")
        .into_inner()
        .tables;
    assert_eq!(tables.len(), 1);

    // テーブル詳細もクエリも通る
    test_app
        .table_as(Some(&bearer(&token)))
        .get(pb::GetTableRequest {
            db_name: "alpha_db".to_string(),
            table_name: "t_one".to_string(),
        })
        .await
        .expect("t_one should be readable");

    test_app
        .query_as(Some(&bearer(&token)))
        .execute(pb::ExecuteQueryRequest {
            value_type: Some(pb::TableDataType::Int as i32),
            spatial_ids: vec![range_id(5, Some((0, 0)), Some((0, 1)), Some((0, 1)))],
            query: Some(source("beta_db", "t_two")),
            format: pb::OutputFormat::Unspecified as i32,
            limit: None,
        })
        .await
        .expect("query should succeed");

    // 書き込みはすべて拒否される
    let err = test_app
        .data_as(Some(&bearer(&token)))
        .insert(pb::InsertDataRequest {
            db_name: "alpha_db".to_string(),
            table_name: "t_one".to_string(),
            value: Some(num(1.0)),
            spatial_ids: vec![single_id(5, 0, 0, 0)],
            zoom_level_policy: pb::ZoomLevelPolicy::Error as i32,
        })
        .await
        .unwrap_err();
    assert_eq!(err.code(), Code::PermissionDenied);

    // テーブル作成・削除も不可
    let err = post_table(&test_app, &token, "alpha_db", "nope")
        .await
        .unwrap_err();
    assert_eq!(err.code(), Code::PermissionDenied);

    let err = test_app
        .table_as(Some(&bearer(&token)))
        .delete(pb::DeleteTableRequest {
            db_name: "alpha_db".to_string(),
            table_name: "t_one".to_string(),
        })
        .await
        .unwrap_err();
    assert_eq!(err.code(), Code::PermissionDenied);

    // データベース作成も、ユーザー管理も不可
    let err = test_app
        .database_as(Some(&bearer(&token)))
        .create(pb::CreateDatabaseRequest {
            name: "nope_db".to_string(),
            description: None,
        })
        .await
        .unwrap_err();
    assert_eq!(err.code(), Code::PermissionDenied);

    let err = test_app
        .user_as(Some(&bearer(&token)))
        .list(pb::ListUsersRequest {
            after: None,
            limit: None,
        })
        .await
        .unwrap_err();
    assert_eq!(err.code(), Code::PermissionDenied);
}

/// `table_id_index` に残っているテーブルの総数。到達不能になったテーブルの検出に使う。
///
/// ストレージ内部の索引を直接数えるため、抽象 API ではなく LMDB のハンドルを直接使う。
fn indexed_table_count(app_state: &kasane::AppState) -> usize {
    let rtxn = app_state.db.env.read_txn().unwrap();
    app_state.db.table_id_index.len(&rtxn).unwrap() as usize
}

#[tokio::test]
/// データベースを削除すると、配下のテーブルが 1 つ残らず消えることを検証する。
///
/// 列挙と削除が別トランザクションに分かれていると、その隙間に作られたテーブルが
/// 親を失って到達不能なまま残る。ここでは削除の完全性そのものを固定する。
async fn test_database_remove_leaves_no_orphan_tables() {
    let test_app = TestApp::new().await;
    let root_token = test_app.root_token().to_string();

    create_db(&test_app, &root_token, "doomed_db").await;
    create_db(&test_app, &root_token, "kept_db").await;
    for i in 0..3 {
        create_table(&test_app, &root_token, "doomed_db", &format!("t_{}", i)).await;
    }
    create_table(&test_app, &root_token, "kept_db", "survivor").await;
    assert_eq!(indexed_table_count(test_app.app_state()), 4);

    test_app
        .database_as(Some(&bearer(&root_token)))
        .delete(pb::DeleteDatabaseRequest {
            db_name: "doomed_db".to_string(),
        })
        .await
        .expect("delete_database failed");

    // 残るのは kept_db の 1 件だけ。doomed_db 配下は索引ごと消えている。
    assert_eq!(indexed_table_count(test_app.app_state()), 1);

    // 同名で作り直しても、以前のテーブルは見えない。
    create_db(&test_app, &root_token, "doomed_db").await;
    let tables = test_app
        .table_as(Some(&bearer(&root_token)))
        .list(pb::ListTablesRequest {
            db_name: "doomed_db".to_string(),
        })
        .await
        .expect("list should succeed")
        .into_inner()
        .tables;
    assert!(tables.is_empty());
}

#[tokio::test]
/// 上限を超える権限ルールが、名前解決を走らせる前に件数だけで拒否されることを検証する。
///
/// gRPC 越しではなくリポジトリ層で確かめる。上限ぶんのルールを送るとメッセージサイズの
/// 上限に先に当たってしまい、件数チェックまで届かないため。
async fn test_privilege_rules_are_capped_before_resolution() {
    use kasane::models::users::{DataRole, MAX_PRIVILEGES_PER_USER, PrivilegeRule};
    use kasane::repositories::{CatalogRepository, Storage};

    let test_app = TestApp::new().await;

    // 実在しないデータベースを指すルールを上限超えの件数だけ並べる。
    // 名前解決が先に走るなら database_not_found になるはずだが、件数チェックが先なので
    // invalid_privilege で落ちる。
    let rules: Vec<PrivilegeRule> = (0..MAX_PRIVILEGES_PER_USER as usize + 1)
        .map(|i| PrivilegeRule::Database {
            db_name: format!("ghost_{i}"),
            role: DataRole::Read,
        })
        .collect();

    let err = test_app
        .app_state()
        .db
        .read(async move |r| CatalogRepository::resolve_rules(r, &rules).await)
        .await
        .expect_err("上限超えが受理された");

    assert!(
        matches!(err, kasane::error::AppError::InvalidPrivilege { .. }),
        "件数ではなく名前解決で落ちている: {err:?}"
    );
}

#[tokio::test]
/// `ListUsers` が利用者名の辞書順でページングされること。
///
/// 1 リクエストの読み取りを利用者数に比例させないための仕組みなので、
/// 「続きの有無」と「境界の重複・欠落が無いこと」を確かめる。
async fn test_user_listing_is_paginated() {
    let test_app = TestApp::new().await;
    let root_token = test_app.root_token().to_string();

    for i in 0..5 {
        create_user_and_token(&test_app, &root_token, &format!("pager{i}"), false).await;
    }

    // root + pager0..4 の 6 人を 4 件ずつ辿る。
    let mut seen: Vec<String> = Vec::new();
    let mut after: Option<String> = None;
    loop {
        let resp = test_app
            .user_as(Some(&bearer(&root_token)))
            .list(pb::ListUsersRequest {
                after: after.clone(),
                limit: Some(4),
            })
            .await
            .expect("list should succeed")
            .into_inner();

        assert!(resp.users.len() <= 4, "limit を超えて返している");
        assert!(!resp.users.is_empty(), "空ページが返った");
        for user in &resp.users {
            seen.push(user.username.clone());
        }

        match resp.next {
            Some(next) => after = Some(next),
            None => break,
        }
    }

    let mut want: Vec<String> = (0..5).map(|i| format!("pager{i}")).collect();
    want.push("root".to_string());
    want.sort();

    assert_eq!(seen, want, "ページの境界で重複または欠落がある");

    // 概要に権限そのものは入らない（入れると読み取りが利用者数×保持数に比例する）。
    // `UserSummary` は `privileges` フィールドを持たない型なので、これは型で保証されている。
    let resp = test_app
        .user_as(Some(&bearer(&root_token)))
        .list(pb::ListUsersRequest {
            after: None,
            limit: Some(1),
        })
        .await
        .expect("list should succeed")
        .into_inner();
    assert!(!resp.users.is_empty());
}
