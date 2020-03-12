#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use frame_support::{decl_error, decl_event, decl_module, decl_storage};
use sp_arithmetic::{
	traits::{Bounded, Saturating},
	Permill,
};
use sp_runtime::{traits::StaticLookup, DispatchError, DispatchResult, RuntimeDebug};
// FIXME: `pallet/frame-` prefix should be used for all pallet modules, but currently `frame_system`
// would cause compiling error in `decl_module!` and `construct_runtime!`
// #3295 https://github.com/paritytech/substrate/issues/3295
use frame_system as system;
use frame_system::ensure_signed;
use orml_traits::{MultiCurrency, PriceProvider};
use orml_utilities::{Fixed128, FixedU128};
use primitives::{
	arithmetic::{fixed_128_from_fixed_u128, fixed_128_from_u128},
	Balance, CurrencyId, Leverage, LiquidityPoolId, Price,
};
use sp_std::{cmp, prelude::*, result};
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
	pub base: CurrencyId,
	pub quote: CurrencyId,
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
	leveraged_held: Fixed128,
	leveraged_debits: Fixed128,
	/// USD value of leveraged held on open position.
	leveraged_held_in_usd: Fixed128,
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

//TODO: Refactor `PositionsByPool` to `double_map LiquidityPoolId, (TradingPair, PositionId) => Option<()>`
// once iteration on key and values of `StorageDoubleMap` ready.
decl_storage! {
	trait Store for Module<T: Trait> as MarginProtocol {
		NextPositionId get(next_position_id): PositionId;
		Positions get(positions): map hasher(blake2_256) PositionId => Option<Position<T>>;
		PositionsByTrader get(positions_by_trader): double_map hasher(twox_64_concat) T::AccountId, hasher(twox_64_concat) LiquidityPoolId => Vec<PositionId>;
		PositionsByPool get(positions_by_pool): double_map hasher(twox_64_concat) LiquidityPoolId, hasher(twox_64_concat) TradingPair => Vec<PositionId>;
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
		NumOutOfBound,
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
		unimplemented!()
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

type PriceResult = result::Result<Price, DispatchError>;
type Fixed128Result = result::Result<Fixed128, DispatchError>;
type BalanceResult = result::Result<Balance, DispatchError>;

// Price helpers
impl<T: Trait> Module<T> {
	/// The price from oracle.
	fn _price(base: CurrencyId, quote: CurrencyId) -> PriceResult {
		T::PriceProvider::get_price(base, quote).ok_or(Error::<T>::NoPrice.into())
	}

	/// ask_price = price * (1 + ask_spread)
	fn _ask_price(pool: LiquidityPoolId, held: CurrencyId, debit: CurrencyId, max: Option<Price>) -> PriceResult {
		let price = Self::_price(debit, held)?;
		//FIXME: liquidity pools should provide spread based on trading pair
		let spread: Price = T::LiquidityPools::get_ask_spread(pool, held)
			.ok_or(Error::<T>::NoAskSpread)?
			.into();
		let ask_price: Price = Price::from_natural(1).saturating_add(spread).saturating_mul(price);

		if let Some(m) = max {
			if ask_price > m {
				return Err(Error::<T>::MarketPriceTooHigh.into());
			}
		}

		Ok(ask_price)
	}

	/// bid_price = price * (1 - bid_spread)
	fn _bid_price(pool: LiquidityPoolId, held: CurrencyId, debit: CurrencyId) -> PriceResult {
		let price = Self::_price(debit, held)?;
		//FIXME: liquidity pools should provide spread based on trading pair
		let spread: Price = T::LiquidityPools::get_bid_spread(pool, held)
			.ok_or(Error::<T>::NoAskSpread)?
			.into();

		Ok(Price::from_natural(1).saturating_sub(spread).saturating_mul(price))
	}

	fn _usd_value(currency_id: CurrencyId, amount: Fixed128) -> Fixed128Result {
		let price = {
			let p = Self::_price(CurrencyId::AUSD, currency_id)?;
			fixed_128_from_fixed_u128(p)
		};
		amount.checked_mul(&price).ok_or(Error::<T>::NumOutOfBound.into())
	}
}

// Trader helpers
impl<T: Trait> Module<T> {
	/// Unrealized profit and loss of a position(USD value).
	///
	/// unrealized_pl_of_position = (curr_price - open_price) * leveraged_held
	fn _unrealized_pl_of_position(position: &Position<T>) -> Fixed128Result {
		// open_price = abs(leveraged_debits / leveraged_held)
		let open_price = position
			.leveraged_debits
			.checked_div(&position.leveraged_held)
			.expect("ensured safe on open position")
			.saturating_abs();
		let curr_price = {
			let p = Self::_bid_price(position.pool, position.pair.quote, position.pair.base)?;
			fixed_128_from_fixed_u128(p)
		};
		let price_delta = curr_price
			.checked_sub(&open_price)
			.expect("Non-negative integers sub can't overflow; qed");
		let unrealized = position
			.leveraged_held
			.checked_mul(&price_delta)
			.ok_or(Error::<T>::NumOutOfBound)?;
		Self::_usd_value(position.pair.quote, unrealized)
	}

	/// Unrealized profit and loss of a given trader(USD value). It is the sum of unrealized profit and loss of all positions
	/// opened by a trader.
	fn _unrealized_pl_of_trader(who: &T::AccountId) -> Fixed128Result {
		<PositionsByTrader<T>>::iter_prefix(who)
			.flatten()
			.filter_map(|position_id| Self::positions(position_id))
			.try_fold(Fixed128::zero(), |acc, p| {
				let unrealized = Self::_unrealized_pl_of_position(&p)?;
				acc.checked_add(&unrealized).ok_or(Error::<T>::NumOutOfBound.into())
			})
	}

	/// Sum of all open margin of a given trader.
	fn _margin_held(who: &T::AccountId) -> Balance {
		<PositionsByTrader<T>>::iter_prefix(who)
			.flatten()
			.filter_map(|position_id| Self::positions(position_id))
			.map(|p| p.open_margin)
			.sum()
	}

	/// Free balance: the balance available for withdraw.
	///
	/// free_balance = balance - margin_held
	fn _free_balance(who: &T::AccountId) -> Balance {
		Self::balances(who)
			.checked_sub(Self::_margin_held(who))
			.expect("ensured enough open margin on open position; qed")
	}

	/// Accumulated swap rate of a position(USD value).
	///
	/// accumulated_swap_rate_of_position = (current_accumulated - open_accumulated) * leveraged_held * price
	fn _accumulated_swap_rate_of_position(position: &Position<T>) -> Fixed128Result {
		let rate = T::LiquidityPools::get_accumulated_swap_rate(position.pool, position.pair)
			.checked_sub(&position.open_accumulated_swap_rate)
			.ok_or(Error::<T>::NumOutOfBound)?;
		let rate_amount = position
			.leveraged_held
			.saturating_abs()
			.checked_mul(&rate)
			.ok_or(Error::<T>::NumOutOfBound)?;
		Self::_usd_value(position.pair.quote, rate_amount)
	}

	/// Accumulated swap of all open positions of a given trader(USD value).
	fn _accumulated_swap_rate_of_trader(who: &T::AccountId) -> Fixed128Result {
		<PositionsByTrader<T>>::iter_prefix(who)
			.flatten()
			.filter_map(|position_id| Self::positions(position_id))
			.try_fold(Fixed128::zero(), |acc, p| {
				let rate_of_p = Self::_accumulated_swap_rate_of_position(&p)?;
				acc.checked_add(&rate_of_p).ok_or(Error::<T>::NumOutOfBound.into())
			})
	}

	/// equity_of_trader = balance + unrealized_pl - accumulated_swap_rate
	fn _equity_of_trader(who: &T::AccountId) -> Fixed128Result {
		let unrealized = Self::_unrealized_pl_of_trader(who)?;
		let with_unrealized = fixed_128_from_u128(Self::balances(who))
			.checked_add(&unrealized)
			.ok_or(Error::<T>::NumOutOfBound)?;
		let accumulated_swap_rate = Self::_accumulated_swap_rate_of_trader(who)?;
		with_unrealized
			.checked_sub(&accumulated_swap_rate)
			.ok_or(Error::<T>::NumOutOfBound.into())
	}

	/// Margin level of a given user.
	///
	/// If `new_position` is `None`, return the margin level based on current positions,
	/// else based on current positions plus this new one.
	fn _margin_level(who: &T::AccountId, new_position: Option<Position<T>>) -> Fixed128Result {
		let equity = Self::_equity_of_trader(who)?;
		let leveraged_held_in_usd = <PositionsByTrader<T>>::iter_prefix(who)
			.flatten()
			.filter_map(|position_id| Self::positions(position_id))
			.chain(new_position.map_or(vec![], |p| vec![p]))
			.try_fold(Fixed128::zero(), |acc, p| {
				acc.checked_add(&p.leveraged_held_in_usd.saturating_abs())
					.ok_or(Error::<T>::NumOutOfBound)
			})?;
		Ok(equity
			.checked_div(&leveraged_held_in_usd)
			// if no leveraged held, margin level is max
			.unwrap_or(Fixed128::max_value()))
	}
}

// Liquidity pool helpers
impl<T: Trait> Module<T> {
	/// equity_of_pool = liquidity - all_unrealized_pl + all_accumulated_swap_rate
	fn _equity_of_pool(pool: LiquidityPoolId) -> Fixed128Result {
		let liquidity = {
			let l = <T::LiquidityPools as LiquidityPools<T::AccountId>>::liquidity(pool);
			fixed_128_from_u128(l)
		};

		// -all_unrealized_pl + all_accumulated_swap_rate
		let unrealized_pl_and_rate = PositionsByPool::iter_prefix(pool)
			.flatten()
			.filter_map(|position_id| Self::positions(position_id))
			.try_fold::<_, _, Fixed128Result>(Fixed128::zero(), |acc, p| {
				let rate = Self::_accumulated_swap_rate_of_position(&p)?;
				let unrealized = Self::_unrealized_pl_of_position(&p)?;
				let sum = rate.checked_sub(&unrealized).ok_or(Error::<T>::NumOutOfBound)?;
				acc.checked_add(&sum).ok_or(Error::<T>::NumOutOfBound.into())
			})?;

		liquidity
			.checked_add(&unrealized_pl_and_rate)
			.ok_or(Error::<T>::NumOutOfBound.into())
	}

	/// Equity to Net Position Ratio (ENP) of a liquidity pool.
	///
	/// If `new_position` is `None`, return the ENP based on current positions,
	/// else based on current positions plus this new one.
	fn _enp(pool: LiquidityPoolId, new_position: Option<Position<T>>) -> Fixed128Result {
		let equity = Self::_equity_of_pool(pool)?;
		let net_position = PositionsByPool::iter_prefix(pool)
			.flatten()
			.filter_map(|position_id| Self::positions(position_id))
			.chain(new_position.map_or(vec![], |p| vec![p]))
			.fold(Fixed128::zero(), |acc, p| {
				acc.checked_add(&p.leveraged_held_in_usd)
					.expect("ensured safe on open position; qed")
			});
		Ok(equity
			.checked_div(&net_position)
			// if `net_position` is zero, ENP is max
			.unwrap_or(Fixed128::max_value()))
	}

	/// Equity to Longest Leg Ratio (ELL) of a liquidity pool.
	///
	/// If `new_position` is `None`, return the ELL based on current positions,
	/// else based on current positions plus this new one.
	fn _ell(pool: LiquidityPoolId, new_position: Option<Position<T>>) -> Fixed128Result {
		let equity = Self::_equity_of_pool(pool)?;
		let (positive_leg, non_positive_leg) = PositionsByPool::iter_prefix(pool)
			.flatten()
			.filter_map(|position_id| Self::positions(position_id))
			.chain(new_position.map_or(vec![], |p| vec![p]))
			.fold((Fixed128::zero(), Fixed128::zero()), |(positive, non_positive), p| {
				if p.leveraged_held_in_usd.is_positive() {
					(
						positive
							.checked_add(&p.leveraged_held_in_usd)
							.expect("ensured safe on open position; qed"),
						non_positive,
					)
				} else {
					(
						positive,
						non_positive
							.checked_add(&p.leveraged_held_in_usd)
							.expect("ensured safe on open position; qed"),
					)
				}
			});
		let longest_leg = cmp::max(positive_leg, non_positive_leg.saturating_abs());
		Ok(equity
			.checked_div(&longest_leg)
			// if `longest_leg` is zero, ELL is max
			.unwrap_or(Fixed128::max_value()))
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
