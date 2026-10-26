#[cfg(feature = "server")]
mod server;
#[cfg(feature = "server")]
pub use server::BoundServer;

pub mod auth;
pub mod convert;
pub mod convert_data;
pub mod convert_query;
#[cfg(feature = "server")]
pub mod convert_users;
mod data;
mod database;
mod query;
mod system;
mod table;
#[cfg(feature = "server")]
mod users;

pub mod pb {
    include!(concat!(env!("OUT_DIR"), "/kasane.rs"));
    pub const FILE_DESCRIPTOR_SET: &[u8] =
        include_bytes!(concat!(env!("OUT_DIR"), "/kasane_descriptor.bin"));
}
