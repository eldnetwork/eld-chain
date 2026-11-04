//! Content connection. Same ABCI listener as info, consensus, mempool, and snapshot.

mod connection;
mod handler;

pub use connection::ContentConnection;
