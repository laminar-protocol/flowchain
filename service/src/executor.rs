//! A `CodeExecutor` specialization which uses natively compiled runtime when
//! the wasm to be executed is equivalent to the natively compiled code.

use sc_executor::native_executor_instance;
pub use sc_executor::NativeExecutor;

// Declare an instance of the native executor named `Executor`. Include the wasm
// binary as the equivalent wasm code.
native_executor_instance!(
	pub DevExecutor,
	dev_runtime::api::dispatch,
	dev_runtime::native_version,
	frame_benchmarking::benchmarking::HostFunctions,
);
