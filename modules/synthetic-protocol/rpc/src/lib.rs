use codec::Codec;
use jsonrpc_core::{Error as RpcError, ErrorCode, Result};
use jsonrpc_derive::rpc;
use module_primitives::{CurrencyId, LiquidityPoolId};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::{generic::BlockId, traits::Block as BlockT};
use std::sync::Arc;
pub use synthetic_protocol_rpc_runtime_api::{PoolInfo, SyntheticProtocolApi as SyntheticProtocolRuntimeApi};

#[rpc]
pub trait SyntheticProtocolApi<BlockHash, AccountId> {
	#[rpc(name = "synthetic_poolInfo")]
	fn pool_info(
		&self,
		pool_id: LiquidityPoolId,
		currency_id: CurrencyId,
		at: Option<BlockHash>,
	) -> Result<Option<PoolInfo>>;
}

/// A struct that implements the [`SyntheticProtocolApi`].
pub struct SyntheticProtocol<C, B> {
	client: Arc<C>,
	_marker: std::marker::PhantomData<B>,
}

impl<C, B> SyntheticProtocol<C, B> {
	/// Create new `SyntheticProtocol` with the given reference to the client.
	pub fn new(client: Arc<C>) -> Self {
		Self {
			client,
			_marker: Default::default(),
		}
	}
}

pub enum Error {
	RuntimeError,
}

impl From<Error> for i64 {
	fn from(e: Error) -> i64 {
		match e {
			Error::RuntimeError => 1,
		}
	}
}

impl<C, Block, AccountId> SyntheticProtocolApi<<Block as BlockT>::Hash, AccountId> for SyntheticProtocol<C, Block>
where
	Block: BlockT,
	C: Send + Sync + 'static + ProvideRuntimeApi<Block> + HeaderBackend<Block>,
	C::Api: SyntheticProtocolRuntimeApi<Block, AccountId>,
	AccountId: Codec,
{
	fn pool_info(
		&self,
		pool_id: LiquidityPoolId,
		currency_id: CurrencyId,
		at: Option<<Block as BlockT>::Hash>,
	) -> Result<Option<PoolInfo>> {
		let api = self.client.runtime_api();
		let at = BlockId::hash(at.unwrap_or_else(||
			// If the block hash is not supplied assume the best block.
			self.client.info().best_hash));
		api.pool_info(&at, pool_id, currency_id)
			.map_err(|e| RpcError {
				code: ErrorCode::ServerError(Error::RuntimeError.into()),
				message: "Unable to get pool info.".into(),
				data: Some(format!("{:?}", e).into()),
			})
			.into()
	}
}
