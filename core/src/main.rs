use actix_web::{middleware::Logger, post, web, App, HttpResponse, HttpServer, Responder};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{mpsc, oneshot, Mutex};
use uuid::Uuid;

use crate::{
    command::process,
    io::full::Storage,
    json::{
        input::{parser, Packet},
        output::Output,
    },
    r#type::{space_time_id::SpaceTimeId, space_time_id_set::SpaceTimeIdSet},
    user_error::UserError,
};

#[macro_use]
mod macros;
pub mod command;
pub mod io;
pub mod json;
pub mod r#type;
pub mod user_error;

fn main() {
    let id = SpaceTimeId::new(
        5,
        (Some(3), Some(10)),
        (Some(3), Some(7)),
        (None, Some(20)),
        0,
        (None, None),
    )
    .unwrap();

    let mut set = SpaceTimeIdSet::new();

    set.insert(id);

    for stid in set.get_all() {
        println!("{},", stid);
    }
}
