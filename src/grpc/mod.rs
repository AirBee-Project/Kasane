//! gRPC の入口。サーバーでは `routes`、組み込みでは `handle` から [`Kasane`](crate::Kasane) を呼ぶ。

#[cfg(not(feature = "server"))]
mod embedded;
#[cfg(feature = "server")]
mod server;
#[cfg(not(feature = "server"))]
pub use embedded::handle;
#[cfg(feature = "server")]
pub use server::routes;

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

/// tonic のサービストレイトを実装する型。
///
/// トレイトの実装は隠せないので、`Kasane` に直接実装すると `create` や `get` が `Kasane` の
/// メソッドとして見えてしまう。それを避けるために包む。
#[repr(transparent)]
pub(crate) struct Service(pub(crate) crate::Kasane);

impl Service {
    /// 組み込みでは呼び出し側が `Kasane` を持ち続けるので、所有権を移さずに包む。
    #[cfg(not(feature = "server"))]
    fn from_ref(kasane: &crate::Kasane) -> &Self {
        // SAFETY: `#[repr(transparent)]` なので `Service` と `Kasane` は同じレイアウトを持つ。
        unsafe { &*(kasane as *const crate::Kasane).cast::<Self>() }
    }
}
