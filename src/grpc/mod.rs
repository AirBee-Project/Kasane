pub mod auth;
pub mod convert;
pub mod convert_data;
pub mod convert_query;
pub mod convert_users;
pub mod data;
pub mod database;
pub mod query;
#[cfg(feature = "server")]
mod server;
pub mod system;
pub mod table;
pub mod users;

#[cfg(feature = "server")]
pub use server::{BoundServer, bind};

pub mod pb {
    include!(concat!(env!("OUT_DIR"), "/kasane.rs"));
    pub const FILE_DESCRIPTOR_SET: &[u8] =
        include_bytes!(concat!(env!("OUT_DIR"), "/kasane_descriptor.bin"));
}
