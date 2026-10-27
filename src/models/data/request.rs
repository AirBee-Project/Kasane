#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    SingleId,
    #[default]
    RangeId,
    FlexId,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
/// テーブルの max_zoom_level よりも大きなズームレベル値の空間IDが入力された場合の処理方針
pub enum ZoomLevelPolicy {
    #[default]
    /// テーブルの max_zoom_level を超える空間IDが入力された場合に、エラー（`InvalidArgument`）を返す
    Error,
    /// テーブルの max_zoom_level を超える空間IDを処理対象から除外し、無視して続行する
    Ignore,
    /// テーブルの max_zoom_level を超える空間IDを、その範囲を内包する max_zoom_level の親空間IDに自動的に正規化して処理する
    Normalize,
}
