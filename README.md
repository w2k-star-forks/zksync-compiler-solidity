# Solidity compiler for zkEVM

The compiler from Solidity to zkEVM.

## Building (only for developers)

1. Get the access to the private [LLVM repository](https://github.com/matter-labs/compiler-llvm).
2. Run `cargo run --release --bin llvm-builder` to build the LLVM framework.
3. Run `cargo build --verbose --release` to build the compiler executable.

## Usage

```
zksolc ERC20.sol --asm --bin --optimize --output-dir './build/'
```

The latest patch of the **solc v0.8** must be available through `PATH`.

**Do not use the former patches of *solc*, as each version introduces important bug fixes!**
