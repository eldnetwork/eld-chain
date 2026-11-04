use std::io::Result;

use crate::{
    async_api::Server as AsyncServer,
    sync_api::{
        async_impls::{
            AsyncConsensusImpl, AsyncContentImpl, AsyncInfoImpl, AsyncMempoolImpl,
            AsyncSnapshotImpl,
        },
        Consensus, Content, Info, Mempool, Snapshot,
    },
    Address,
};

/// ABCI Server
pub struct Server<C, M, I, S, T>
where
    C: Consensus + Send + Sync + 'static,
    M: Mempool + Send + Sync + 'static,
    I: Info + Send + Sync + 'static,
    S: Snapshot + Send + Sync + 'static,
    T: Content + Send + Sync + 'static,
{
    async_server: AsyncServer<
        AsyncConsensusImpl<C>,
        AsyncMempoolImpl<M>,
        AsyncInfoImpl<I>,
        AsyncSnapshotImpl<S>,
        AsyncContentImpl<T>,
    >,
}

impl<C, M, I, S, T> Server<C, M, I, S, T>
where
    C: Consensus + Send + Sync + 'static,
    M: Mempool + Send + Sync + 'static,
    I: Info + Send + Sync + 'static,
    S: Snapshot + Send + Sync + 'static,
    T: Content + Send + Sync + 'static,
{
    /// Creates a new instance of [`Server`](self::Server)
    pub fn new(consensus: C, mempool: M, info: I, snapshot: S, content: T) -> Self {
        Self {
            async_server: AsyncServer::new(
                AsyncConsensusImpl::new(consensus),
                AsyncMempoolImpl::new(mempool),
                AsyncInfoImpl::new(info),
                AsyncSnapshotImpl::new(snapshot),
                AsyncContentImpl::new(content),
            ),
        }
    }

    /// Starts ABCI server
    pub fn run<A>(&self, addr: A) -> Result<()>
    where
        A: Into<Address>,
    {
        cfg_if::cfg_if! {
            if #[cfg(feature = "use-async-std")] {
                async_std::task::block_on(async { self.async_server.run(addr).await })
            } else if #[cfg(feature = "use-smol")] {
                smol::block_on(async { self.async_server.run(addr).await })
            } else if #[cfg(feature = "use-tokio")] {
                let runtime = tokio::runtime::Runtime::new()?;
                runtime.block_on(async { self.async_server.run(addr).await })
            } else {
                unreachable!()
            }
        }
    }
}
