use crate::models::table::TableDataType;
use crate::{error::AppError, models::table::InfoTableResponse};
use axum::{Json, extract::Path};

pub async fn info(Path(name): Path<String>) -> Result<Json<InfoTableResponse>, AppError> {
    let res = InfoTableResponse {
        name: name.clone(),
        r#type: TableDataType::Boolean,
    };
    Ok(Json(res))
}
