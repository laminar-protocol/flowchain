//! Mocks for the margin protocol module.

#![cfg(test)]

use frame_support::{impl_outer_event, impl_outer_origin, ord_parameter_types, parameter_types};
use frame_system as system;
use orml_utilities::Fixed128;
use primitives::{Balance, CurrencyId, LiquidityPoolId, TradingPair};
use sp_core::H256;
use sp_runtime::{testing::Header, traits::IdentityLookup, PerThing, Perbill};
use sp_std::{cell::RefCell, collections::btree_map::BTreeMap};
use traits::LiquidityPools;

use super::*;

impl_outer_origin! {
	pub enum Origin for Runtime {}
}

mod margin_protocol {
	pub use crate::Event;
}

impl_outer_event! {
	pub enum TestEvent for Runtime {
		frame_system<T>, orml_tokens<T>, margin_protocol<T>,
	}
}

// Workaround for https://github.com/rust-lang/rust/issues/26925 . Remove when sorted.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Runtime;
parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const MaximumBlockWeight: u32 = 1024;
	pub const MaximumBlockLength: u32 = 2 * 1024;
	pub const AvailableBlockRatio: Perbill = Perbill::one();
}

type AccountId = u64;
impl frame_system::Trait for Runtime {
	type Origin = Origin;
	type Call = ();
	type Index = u64;
	type BlockNumber = u64;
	type Hash = H256;
	type Hashing = ::sp_runtime::traits::BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = TestEvent;
	type BlockHashCount = BlockHashCount;
	type MaximumBlockWeight = MaximumBlockWeight;
	type MaximumBlockLength = MaximumBlockLength;
	type AvailableBlockRatio = AvailableBlockRatio;
	type Version = ();
	type ModuleToIndex = ();
	type AccountData = ();
	type OnNewAccount = ();
	type OnKilledAccount = ();
}
pub type System = system::Module<Runtime>;

type Amount = i128;

parameter_types! {
	pub const ExistentialDeposit: u128 = 100;
}

impl orml_tokens::Trait for Runtime {
	type Event = TestEvent;
	type Balance = u128;
	type Amount = Amount;
	type CurrencyId = CurrencyId;
	type ExistentialDeposit = ExistentialDeposit;
	type DustRemoval = ();
}

pub type OrmlTokens = orml_tokens::Module<Runtime>;

thread_local! {
	static PRICES: RefCell<BTreeMap<CurrencyId, Price>> = RefCell::new(BTreeMap::new());
}

pub struct MockPrices;
impl MockPrices {
	pub fn set_mock_price(currency_id: CurrencyId, price: Option<Price>) {
		if let Some(p) = price {
			PRICES.with(|v| v.borrow_mut().insert(currency_id, p));
		} else {
			PRICES.with(|v| v.borrow_mut().remove(&currency_id));
		}
	}

	fn prices(currency_id: CurrencyId) -> Option<Price> {
		PRICES.with(|v| v.borrow_mut().get(&currency_id).map(|p| *p))
	}
}
impl PriceProvider<CurrencyId, Price> for MockPrices {
	fn get_price(base: CurrencyId, quote: CurrencyId) -> Option<Price> {
		let base_price = Self::prices(base)?;
		let quote_price = Self::prices(quote)?;

		quote_price.checked_div(&base_price)
	}
}

thread_local! {
	static SPREAD: RefCell<Permill> = RefCell::new(Permill::zero());
	static ACC_SWAP_RATES: RefCell<BTreeMap<TradingPair, Fixed128>> = RefCell::new(BTreeMap::new());
}

//TODO: implementation based on unit test requirements
pub struct MockLiquidityPools;
impl MockLiquidityPools {
	pub fn spread() -> Permill {
		SPREAD.with(|v| *v.borrow_mut())
	}

	pub fn set_mock_spread(spread: Permill) {
		SPREAD.with(|v| *v.borrow_mut() = spread);
	}

	pub fn accumulated_swap_rate(pair: TradingPair) -> Fixed128 {
		ACC_SWAP_RATES.with(|v| v.borrow_mut().get(&pair).map(|r| *r)).unwrap()
	}

	pub fn set_mock_accumulated_swap_rate(pair: TradingPair, rate: Fixed128) {
		ACC_SWAP_RATES.with(|v| v.borrow_mut().insert(pair, rate));
	}
}
impl LiquidityPools<AccountId> for MockLiquidityPools {
	type LiquidityPoolId = LiquidityPoolId;
	type CurrencyId = CurrencyId;
	type Balance = Balance;

	fn ensure_liquidity(pool_id: Self::LiquidityPoolId) -> bool {
		unimplemented!()
	}

	fn is_owner(pool_id: Self::LiquidityPoolId, who: &u64) -> bool {
		unimplemented!()
	}

	fn liquidity(pool_id: Self::LiquidityPoolId) -> Self::Balance {
		unimplemented!()
	}

	fn deposit_liquidity(source: &u64, pool_id: Self::LiquidityPoolId, amount: Self::Balance) -> DispatchResult {
		unimplemented!()
	}

	fn withdraw_liquidity(dest: &u64, pool_id: Self::LiquidityPoolId, amount: Self::Balance) -> DispatchResult {
		unimplemented!()
	}
}
impl MarginProtocolLiquidityPools<AccountId> for MockLiquidityPools {
	type TradingPair = TradingPair;

	fn is_allowed_position(pool_id: Self::LiquidityPoolId, pair: Self::TradingPair, leverage: Leverage) -> bool {
		unimplemented!()
	}

	fn get_bid_spread(_pool_id: Self::LiquidityPoolId, _pair: Self::TradingPair) -> Option<Permill> {
		Some(Self::spread())
	}

	fn get_ask_spread(_pool_id: Self::LiquidityPoolId, _pair: Self::TradingPair) -> Option<Permill> {
		Some(Self::spread())
	}

	fn get_swap_rate(pool_id: Self::LiquidityPoolId, pair: Self::TradingPair) -> Fixed128 {
		unimplemented!()
	}

	fn get_accumulated_swap_rate(pool_id: Self::LiquidityPoolId, pair: Self::TradingPair) -> Fixed128 {
		Self::accumulated_swap_rate(pair)
	}

	fn can_open_position(
		pool_id: Self::LiquidityPoolId,
		pair: Self::TradingPair,
		leverage: Leverage,
		leveraged_amount: Balance,
	) -> bool {
		unimplemented!()
	}
}

pub type MarginProtocol = Module<Runtime>;

pub const ALICE: AccountId = 0;
pub const MOCK_POOL: LiquidityPoolId = 100;

impl Trait for Runtime {
	type Event = TestEvent;
	type MultiCurrency = OrmlTokens;
	type LiquidityPools = MockLiquidityPools;
	type PriceProvider = MockPrices;
}

//TODO: more fields based on unit test requirements
pub struct ExtBuilder {
	endowed_accounts: Vec<(AccountId, CurrencyId, Balance)>,
	spread: Permill,
	prices: Vec<(CurrencyId, Price)>,
	swap_rates: Vec<(TradingPair, Fixed128)>,
	trader_risk_threshold: RiskThreshold,
	liquidity_pool_enp_threshold: RiskThreshold,
	liquidity_pool_ell_threshold: RiskThreshold,
}

impl Default for ExtBuilder {
	/// Spread - 1/1000
	fn default() -> Self {
		Self {
			endowed_accounts: vec![],
			spread: Permill::from_rational_approximation(1, 1000u32),
			prices: vec![(CurrencyId::AUSD, FixedU128::from_rational(1, 1))],
			swap_rates: vec![],
			trader_risk_threshold: RiskThreshold::default(),
			liquidity_pool_enp_threshold: RiskThreshold::default(),
			liquidity_pool_ell_threshold: RiskThreshold::default(),
		}
	}
}

impl ExtBuilder {
	pub fn balances(mut self, endowed_accounts: Vec<(AccountId, CurrencyId, Balance)>) -> Self {
		self.endowed_accounts = endowed_accounts;
		self
	}

	pub fn spread(mut self, spread: Permill) -> Self {
		self.spread = spread;
		self
	}

	/// `price`: rational(x, y)
	pub fn price(mut self, currency_id: CurrencyId, price: (u128, u128)) -> Self {
		self.prices
			.push((currency_id, FixedU128::from_rational(price.0, price.1)));
		self
	}

	pub fn accumulated_swap_rate(mut self, pair: TradingPair, rate: Fixed128) -> Self {
		self.swap_rates.push((pair, rate));
		self
	}

	pub fn trader_risk_threshold(mut self, threshold: RiskThreshold) -> Self {
		self.trader_risk_threshold = threshold;
		self
	}

	pub fn liquidity_pool_enp_threshold(mut self, threshold: RiskThreshold) -> Self {
		self.liquidity_pool_enp_threshold = threshold;
		self
	}

	pub fn liquidity_pool_ell_threshold(mut self, threshold: RiskThreshold) -> Self {
		self.liquidity_pool_ell_threshold = threshold;
		self
	}

	fn set_mocks(&self) {
		self.prices
			.iter()
			.for_each(|(c, p)| MockPrices::set_mock_price(*c, Some(*p)));
		MockLiquidityPools::set_mock_spread(self.spread);
		self.swap_rates
			.iter()
			.for_each(|(p, r)| MockLiquidityPools::set_mock_accumulated_swap_rate(*p, *r));
	}

	pub fn build(self) -> sp_io::TestExternalities {
		self.set_mocks();

		let mut t = frame_system::GenesisConfig::default()
			.build_storage::<Runtime>()
			.unwrap();

		orml_tokens::GenesisConfig::<Runtime> {
			endowed_accounts: self.endowed_accounts,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		GenesisConfig {
			trader_risk_threshold: self.trader_risk_threshold,
			liquidity_pool_enp_threshold: self.liquidity_pool_enp_threshold,
			liquidity_pool_ell_threshold: self.liquidity_pool_ell_threshold,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		t.into()
	}
}
