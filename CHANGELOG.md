# The `zksolc` changelog

## Unreleased

### Added

- better compatibility of opcodes `GASLIMIT`, `GASPRICE`, `CHAINID`, `DIFFICULTY`, `COINBASE` etc.

### Fixed

- the check for reserved function names in variable names
- an EVMLA stack inconsistency issue with the `GASPRICE` opcode

## Version 1.1.3 [2022-07-16]

### Added

- the extcodesize check before the method selector
- the check for the latest supportable version of `solc`
- a lot of LLVM optimizations

### Changed

- the default memory allocator for MUSL to `mimalloc`

### Fixed

- overwriting the return data size during non-EVM far calls
- the incorrect behavior of immutables in some cases

## Version 1.1.2 [2022-07-01]

### Changed

- the exponentiation algorithm from linear to binary

## Version 1.1.1 [2022-06-24]

### Fixed

- the evaluation order of event indexed fields

## Version 1.1.0 [2022-06-21]

*Initial release*
