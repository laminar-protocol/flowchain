#![cfg(test)]

use super::*;

use frame_support::{impl_outer_origin, ord_parameter_types, parameter_types, traits::OnInitialize, weights::Weight};
use frame_system::EnsureSignedBy;
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
	Perbill,
};

use orml_currencies::Currency;
use orml_traits::parameter_type_with_key;

use primitives::{Balance, CurrencyId, LiquidityPoolId};
use traits::{BaseLiquidityPoolManager, MarginProtocolLiquidityPoolsManager};

pub type BlockNumber = u64;
pub type AccountId = u64;

ord_parameter_types! {
	pub const UpdateOrigin: AccountId = 0;
}

impl_outer_origin! {
	pub enum Origin for Runtime {}
}

// For testing the module, we construct most of a mock runtime. This means
// first constructing a configuration type (`Runtime`) which `impl`s each of the
// configuration traits of modules we want to use.
#[derive(Clone, Eq, PartialEq)]
pub struct Runtime;
parameter_types! {
	pub const BlockHashCount: u64 = 250;
}

impl frame_system::Config for Runtime {
	type Origin = Origin;
	type Call = ();
	type Index = u64;
	type BlockNumber = BlockNumber;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = ();
	type BlockHashCount = BlockHashCount;
	type BlockWeights = ();
	type BlockLength = ();
	type DbWeight = ();
	type Version = ();
	type PalletInfo = ();
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type BaseCallFilter = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
}

pub type System = frame_system::Module<Runtime>;

parameter_types! {
	pub const ExistentialDeposit: u128 = 50;
	pub const GetNativeCurrencyId: CurrencyId = CurrencyId::LAMI;
	pub const GetLiquidityCurrencyId: CurrencyId = CurrencyId::AUSD;
	pub MaxSwap: FixedI128 = FixedI128::saturating_from_integer(2);
}

impl pallet_balances::Config for Runtime {
	type Balance = Balance;
	type DustRemoval = ();
	type Event = ();
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = frame_system::Module<Runtime>;
	type MaxLocks = ();
	type WeightInfo = ();
}

type NativeCurrency = Currency<Runtime, GetNativeCurrencyId>;
pub type LiquidityCurrency = orml_currencies::Currency<Runtime, GetLiquidityCurrencyId>;

impl orml_currencies::Config for Runtime {
	type Event = ();
	type MultiCurrency = orml_tokens::Module<Runtime>;
	type NativeCurrency = NativeCurrency;
	type GetNativeCurrencyId = GetNativeCurrencyId;
	type WeightInfo = ();
}

parameter_type_with_key! {
	pub ExistentialDeposits: |currency_id: CurrencyId| -> Balance {
		Zero::zero()
	};
}

parameter_types! {
	pub TreasuryAccount: AccountId = 1;
}

type Amount = i128;
impl orml_tokens::Config for Runtime {
	type Event = ();
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = CurrencyId;
	type WeightInfo = ();
	type ExistentialDeposits = ExistentialDeposits;
	type OnDust = orml_tokens::TransferDust<Runtime, TreasuryAccount>;
}

pub struct PoolManager;
impl BaseLiquidityPoolManager<LiquidityPoolId, Balance> for PoolManager {
	fn can_remove(_pool_id: LiquidityPoolId) -> bool {
		true
	}
	fn ensure_can_withdraw(_pool: LiquidityPoolId, _amount: Balance) -> DispatchResult {
		Ok(())
	}
}

parameter_types! {
	pub const MarginLiquidityPoolsModuleId: ModuleId = MODULE_ID;
	pub const IdentityDeposit: Balance = 1000;
}

pub type MarginInstance = module_base_liquidity_pools::Instance1;

impl module_base_liquidity_pools::Config<MarginInstance> for Runtime {
	type Event = ();
	type LiquidityCurrency = LiquidityCurrency;
	type PoolManager = PoolManager;
	type ExistentialDeposit = ExistentialDeposit;
	type IdentityDeposit = IdentityDeposit;
	type IdentityDepositCurrency = pallet_balances::Module<Self>;
	type ModuleId = MarginLiquidityPoolsModuleId;
	type OnDisableLiquidityPool = ModuleLiquidityPools;
	type OnRemoveLiquidityPool = ModuleLiquidityPools;
	type UpdateOrigin = EnsureSignedBy<UpdateOrigin, AccountId>;
	type WeightInfo = ();
}
pub type BaseLiquidityPools = module_base_liquidity_pools::Module<Runtime, MarginInstance>;

pub struct DummyPoolManager;
impl MarginProtocolLiquidityPoolsManager for DummyPoolManager {
	fn ensure_can_enable_trading_pair(_pool_id: LiquidityPoolId, _pair: TradingPair) -> DispatchResult {
		Ok(())
	}
}

parameter_types! {
	pub const MinimumPeriod: u64 = 5;
}
impl pallet_timestamp::Config for Runtime {
	type Moment = u64;
	type OnTimestampSet = ();
	type MinimumPeriod = MinimumPeriod;
	type WeightInfo = ();
}
pub type Timestamp = pallet_timestamp::Module<Runtime>;

impl Config for Runtime {
	type Event = ();
	type BaseLiquidityPools = module_base_liquidity_pools::Module<Runtime, MarginInstance>;
	type PoolManager = DummyPoolManager;
	type UpdateOrigin = EnsureSignedBy<UpdateOrigin, AccountId>;
	type MaxSwapRate = MaxSwap;
	type UnixTime = Timestamp;
	type Moment = u64;
	type WeightInfo = ();
}
pub type ModuleLiquidityPools = Module<Runtime>;

// This function basically just builds a genesis storage key/value store according to
// our desired mockup.
pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut t = frame_system::GenesisConfig::default()
		.build_storage::<Runtime>()
		.unwrap()
		.into();

	orml_tokens::GenesisConfig::<Runtime> {
		endowed_accounts: vec![(ALICE, CurrencyId::AUSD, 100_000), (BOB, CurrencyId::AUSD, 100_000)],
	}
	.assimilate_storage(&mut t)
	.unwrap();

	t.into()
}

pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;

pub fn execute_time(sec: u64) {
	System::set_block_number(sec);
	Timestamp::set_timestamp(sec * 1000);
	<ModuleLiquidityPools as OnInitialize<u64>>::on_initialize(sec);
}
