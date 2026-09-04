//! ベンチマーク共通のセットアップ。
use std::collections::HashMap;
use std::sync::OnceLock;

use kasane::models::database::table::TableDataType;
use kasane::models::database::table::data::ZoomLevelPolicy;
use kasane::models::database::table::{CreateTableRequest, data::GetDataQuery};
use kasane::models::spatial_id::SpatialId;
use kasane::models::users::User;
use kasane::repositories::{ReadRepository, Storage};
use kasane::{AppState, backend::Db};
use kasane_logic::{AllowedIntervals, FlexId, SpatialIdSet, SpatialIdTable};

pub const DB_NAME: &str = "bench";

/// ベンチ用アプリの土台。`_temp_dir` を保持し続けて LMDB ファイルを生存させる。
pub struct BenchEnv {
    pub app_state: AppState,
    pub user: User,
    _temp_dir: tempfile::TempDir,
}

/// LMDB バックエンドの `AppState` を作り、`bench` データベースと root ユーザーを揃える。
pub fn build_env(rt: &tokio::runtime::Runtime) -> BenchEnv {
    let temp_dir = tempfile::TempDir::new().expect("failed to create temp dir for bench LMDB");
    let db: Db = kasane::repositories::lmdb::initialize_database(
        temp_dir
            .path()
            .to_str()
            .expect("temp dir path is not valid UTF-8"),
    )
    .expect("failed to initialize LMDB");
    let app_state = AppState::new(db);

    rt.block_on(async {
        kasane::services::database::create(&app_state, DB_NAME, None)
            .await
            .expect("failed to create bench database");

        let user = app_state
            .db
            .read(async move |r| r.get_user("root").await)
            .await
            .expect("failed to read root user")
            .expect("root user should exist right after initialize_database");

        BenchEnv {
            app_state,
            user,
            _temp_dir: temp_dir,
        }
    })
}

/// 建物リスクデータ
pub struct RiskData {
    pub by_value: HashMap<i64, Vec<SpatialId>>,
    pub coverage: Vec<SpatialId>,
}

static RISK_DATA: OnceLock<RiskData> = OnceLock::new();

/// `sample/bldg_risk.json`を読み込む。
pub fn risk_data() -> &'static RiskData {
    RISK_DATA.get_or_init(|| {
        let json_str = std::fs::read_to_string("sample/bldg_risk.json").expect(
            "sample/bldg_risk.json が読めない。ワークスペースルートから実行しているか確認する",
        );
        let table: SpatialIdTable<i64> =
            serde_json::from_str(&json_str).expect("bldg_risk.json のパースに失敗した");

        let mut by_value: HashMap<i64, Vec<SpatialId>> = HashMap::new();
        let mut coverage_set = SpatialIdSet::new();
        for (flex_id, value) in table.iter() {
            by_value
                .entry(*value)
                .or_default()
                .push(flex_id_to_spatial_id(&flex_id));
            coverage_set.insert(flex_id);
        }

        let coverage = coverage_set
            .range_ids_in(AllowedIntervals::calendar())
            .map(SpatialId::RangeId)
            .collect();

        RiskData { by_value, coverage }
    })
}

fn flex_id_to_spatial_id(id: &FlexId) -> SpatialId {
    SpatialId::FlexId(*id)
}

/// 建物リスクデータを実際に書き込み経路（`services::database::table::data::insert`）で
/// 投入する。値ごとにまとめて 1 回の呼び出しにするので、往復は値の種類数（数十〜百程度）
/// で済む。
pub fn load_risk_table(
    rt: &tokio::runtime::Runtime,
    env: &BenchEnv,
    table_name: &str,
    max_zoom_level: u8,
) {
    let data = risk_data();

    rt.block_on(async {
        kasane::services::database::table::create::create(
            &env.app_state,
            &env.user,
            DB_NAME,
            table_name,
            CreateTableRequest {
                name: table_name.to_string(),
                data_type: TableDataType::Int,
                max_zoom_level,
                constraints: None,
                description: None,
                value_index: false,
                // サンプルデータは時間軸を持たない。
                is_temporal: false,
            },
        )
        .await
        .expect("failed to create bench table");

        for (value, ids) in &data.by_value {
            kasane::services::database::table::data::insert::insert(
                &env.app_state,
                &env.user,
                DB_NAME,
                table_name,
                ids,
                (*value).into(),
                &ZoomLevelPolicy::Error,
            )
            .await
            .unwrap_or_else(|e| panic!("failed to insert bench data (value={value}): {e}"));
        }
    });
}

/// クエリ結果のフォーマット指定。中身は問わないので既定（`RangeId`）を使う。
pub fn default_query_params() -> GetDataQuery {
    GetDataQuery {
        format: kasane::models::database::table::data::OutputFormat::RangeId,
        limit: None,
    }
}
