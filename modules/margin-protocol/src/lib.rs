#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use frame_support::{decl_error, decl_event, decl_module, decl_storage};
use sp_arithmetic::Permill;
use sp_runtime::{traits::StaticLookup, DispatchError, DispatchResult, RuntimeDebug};
// FIXME: `pallet/frame-` prefix should be used for all pallet modules, but currently `frame_system`
// would cause compiling error in `decl_module!` and `construct_runtime!`
// #3295 https://github.com/paritytech/substrate/issues/3295
use frame_system as system;
use frame_system::ensure_signed;
use orml_traits::{MultiCurrency, PriceProvider};
use orml_utilities::{Fixed128, FixedU128};
use primitives::{price_from_balance, Balance, CurrencyId, Leverage, LiquidityPoolId, Price};
use sp_std::result;
use traits::{LiquidityPoolManager, LiquidityPools, MarginProtocolLiquidityPools};

mod mock;
mod tests;

pub trait Trait: frame_system::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
	type MultiCurrency: MultiCurrency<Self::AccountId, Balance = Balance, CurrencyId = CurrencyId>;
	type LiquidityPools: MarginProtocolLiquidityPools<
		Self::AccountId,
		CurrencyId = CurrencyId,
		Balance = Balance,
		LiquidityPoolId = LiquidityPoolId,
		TradingPair = TradingPair,
	>;
	type PriceProvider: PriceProvider<CurrencyId, Price>;
}

#[derive(Encode, Decode, Copy, Clone, RuntimeDebug, Eq, PartialEq)]
pub struct TradingPair {
	base: CurrencyId,
	quote: CurrencyId,
}

impl TradingPair {
	fn normalize() {
		// TODO: make the smaller priced currency id as base
		unimplemented!()
	}
}

pub type PositionId = u64;

#[derive(Encode, Decode, RuntimeDebug, Eq, PartialEq)]
pub struct Position<T: Trait> {
	owner: T::AccountId,
	pool: LiquidityPoolId,
	pair: TradingPair,
	leverage: Leverage,
	leveraged_holding: Balance,
	leveraged_debits: Balance,
	open_accumulated_swap_rate: Fixed128,
	open_margin: Balance,
}

//TODO: set this value
const MAX_POSITIONS_COUNT: u16 = u16::max_value();

#[derive(Encode, Decode, Copy, Clone, RuntimeDebug, Eq, PartialEq)]
pub struct RiskThreshold {
	margin_call: Permill,
	stop_out: Permill,
}

decl_storage! {
	trait Store for Module<T: Trait> as MarginProtocol {
		NextPositionId get(next_position_id): PositionId;
		Positions get(positions): map hasher(blake2_256) PositionId => Option<Position<T>>;
		PositionsByUser get(positions_by_user): double_map hasher(blake2_256) T::AccountId, hasher(blake2_256) LiquidityPoolId => Vec<PositionId>;
		PositionsByPool get(positions_by_pool): double_map hasher(blake2_256) LiquidityPoolId, hasher(blake2_256) (TradingPair, PositionId) => Option<()>;
		// SwapPeriods get(swap_periods): map hasher(black2_256) TradingPair => Option<SwapPeriod>;
		Balances get(balances): map hasher(blake2_256) T::AccountId => Balance;
		MinLiquidationPercent get(min_liquidation_percent): map hasher(blake2_256) TradingPair => Fixed128;
		MarginCalledTraders get(margin_called_traders): map hasher(blake2_256) T::AccountId => Option<()>;
		MarginCalledLiquidityPools get(margin_called_liquidity_pools): map hasher(blake2_256) LiquidityPoolId => Option<()>;
		TraderRiskThreshold get(trader_risk_threshold): map hasher(blake2_256) TradingPair => Option<RiskThreshold>;
		LiquidityPoolENPThreshold get(liquidity_pool_enp_threshold): map hasher(blake2_256) TradingPair => Option<RiskThreshold>;
		LiquidityPoolELLThreshold get(liquidity_pool_ell_threshold): map hasher(blake2_256) TradingPair => Option<RiskThreshold>;
	}
}

decl_event! {
	pub enum Event<T> where
		<T as frame_system::Trait>::AccountId,
		LiquidityPoolId = LiquidityPoolId,
		TradingPair = TradingPair,
		Amount = Balance
	{
		/// Position opened: (who, pool_id, trading_pair, leverage, leveraged_amount, price)
		PositionOpened(AccountId, LiquidityPoolId, TradingPair, Leverage, Amount, Price),
		/// Position closed: (who, position_id, price)
		PositionClosed(AccountId, PositionId, Price),
		/// Deposited: (who, amount)
		Deposited(AccountId, Amount),
		/// Withdrew: (who, amount)
		Withdrew(AccountId, Amount),
	}
}

decl_error! {
	pub enum Error for Module<T: Trait> {
		NoPrice,
		NoAskSpread,
		MarketPriceTooHigh,
		NumOverflow,
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		type Error = Error<T>;

		fn deposit_event() = default;

		pub fn open_position(origin, pool: LiquidityPoolId, pair: TradingPair, leverage: Leverage, #[compact] leveraged_amount: Balance, price: Price) {
			let who = ensure_signed(origin)?;
			Self::_open_position(&who, pool, pair, leverage, leveraged_amount, price)?;

			Self::deposit_event(RawEvent::PositionOpened(who, pool, pair, leverage, leveraged_amount, price));
		}

		pub fn close_position(origin, position_id: PositionId, price: Price) {
			let who = ensure_signed(origin)?;
			Self::_close_position(&who, position_id, price)?;

			Self::deposit_event(RawEvent::PositionClosed(who, position_id, price));
		}

		pub fn deposit(origin, #[compact] amount: Balance) {
			let who = ensure_signed(origin)?;
			Self::_deposit(&who, amount)?;

			Self::deposit_event(RawEvent::Deposited(who, amount));
		}

		pub fn withdraw(origin, #[compact] amount: Balance) {
			let who = ensure_signed(origin)?;
			Self::_withdraw(&who, amount)?;

			Self::deposit_event(RawEvent::Withdrew(who, amount));
		}

		// TODO: implementations
		pub fn trader_margin_call(origin, who: <T::Lookup as StaticLookup>::Source) {}
		pub fn trader_become_safe(origin, who: <T::Lookup as StaticLookup>::Source) {}
		pub fn trader_liquidate(origin, who: <T::Lookup as StaticLookup>::Source) {}
		pub fn liquidity_pool_margin_call(origin, pool: LiquidityPoolId) {}
		pub fn liquidity_pool_become_safe(origin, pool: LiquidityPoolId) {}
		pub fn liquidity_pool_liquidate(origin, pool: LiquidityPoolId) {}
	}
}

// Dispatchable functions impl

impl<T: Trait> Module<T> {
	fn _open_position(
		who: &T::AccountId,
		pool: LiquidityPoolId,
		pair: TradingPair,
		leverage: Leverage,
		leveraged_amount: Balance,
		price: Price,
	) -> DispatchResult {
		// TODO: implementation
		// ensure enough usd
		let ask_price = Self::_ask_price_in_usd(pool, pair.quote, price)?;
		let open_margin = price_from_balance(leveraged_amount)
			.checked_div(&ask_price)
			.ok_or(Error::<T>::NumOverflow)?;

		Ok(())
	}

	fn _close_position(who: &T::AccountId, position_id: PositionId, price: Price) -> DispatchResult {
		// TODO: implementation
		unimplemented!()
	}

	fn _deposit(who: &T::AccountId, amount: Balance) -> DispatchResult {
		// TODO: implementation
		unimplemented!()
	}

	fn _withdraw(who: &T::AccountId, amount: Balance) -> DispatchResult {
		// TODO: implementation
		unimplemented!()
	}
}

// Price helpers
impl<T: Trait> Module<T> {
	/// ask_price_in_usd =
	fn _ask_price_in_usd(
		pool: LiquidityPoolId,
		currency_id: CurrencyId,
		max_price: Price,
	) -> result::Result<Price, DispatchError> {
		let oracle_price = T::PriceProvider::get_price(CurrencyId::AUSD, currency_id).ok_or(Error::<T>::NoPrice)?;
		let ask_spread: Price = T::LiquidityPools::get_ask_spread(pool, currency_id)
			.ok_or(Error::<T>::NoAskSpread)?
			.into();
		let market_price: Price = ask_spread
			.checked_mul(&oracle_price)
			.expect("ask spread < 1; qed")
			.checked_add(&oracle_price)
			.ok_or(Error::<T>::NumOverflow)?;
		if market_price > max_price {
			Ok(market_price)
		} else {
			Err(Error::<T>::MarketPriceTooHigh.into())
		}
	}
}

// Trader helpers
impl<T: Trait> Module<T> {
	/// Unrealized profit and loss.
	fn _unrealized_pl(who: &T::AccountId) {
		unimplemented!()
	}

	/// Sum of all open margin under a given trader.
	fn _margin_held(who: &T::AccountId) {
		unimplemented!()
	}

	/// Free balance: the balance available for withdraw.
	///
	/// free_balance = balance - margin_held
	fn _free_balance(who: &T::AccountId) {
		unimplemented!()
	}

	/// Free margin: the margin available for opening new positions.
	///
	/// free_margin = balance + unrealized_pl - margin_held
	fn _free_margin(who: &T::AccountId) {
		unimplemented!()
	}

	/// Accumulated swap of all open positions of a given trader.
	fn _accumulated_swap(who: &T::AccountId) {
		unimplemented!()
	}

	/// equity = balance + unrealized_pl + accumulated swap
	fn _equity(who: &T::AccountId) {
		unimplemented!()
	}

	/// Margin level of a given user. If `new_position` is `None`, return the margin level based on current positions,
	/// else based on current positions plus this new one.
	fn _margin_level(who: &T::AccountId, new_position: Option<Position<T>>) -> FixedU128 {
		unimplemented!()
	}
}

//TODO: implementations, prevent open new position for margin called pools
impl<T: Trait> LiquidityPoolManager<LiquidityPoolId, Balance> for Module<T> {
	fn can_remove(pool: LiquidityPoolId) -> bool {
		unimplemented!()
	}

	fn get_required_deposit(pool: LiquidityPoolId) -> Balance {
		unimplemented!()
	}
}
