[![Status badge](https://github.com/laminar-protocol/flowchain/workflows/Test/badge.svg)](https://github.com/laminar-protocol/flowchain/actions?workflow=Test)

# flowchain

A new SRML-based Substrate node, ready for hacking.

# Building

Install Rust:

```bash
curl https://sh.rustup.rs -sSf | sh
```

Install required tools:

```bash
make init
```

Build all native code:

```bash
make build
```

# Run

You can start a development chain with:

```bash
make run
```

# Development

To type check:

```bash
make check
```

To purge old chain data:

```bash
make purge
```

To purge old chain data and run

```bash
make restart
```

__Note:__ All build command from Makefile are designed for local development purpose and hence have `SKIP_WASM_BUILD` enabled to speed up build time and use `--execution native` to only run use native execution mode.
