# The `zksolc` changelog

## [Unreleased]

## [1.1.6] - 2022-09-02

### Added

- Better compiler errors for the Yul mode

### Changed

- Unsupported instructions `PC`, `EXTCODECOPY`, `SELFDESTRUCT` now produce compiler errors

### Fixed

- Bloating the array of immutables with zero values

## [1.1.5] - 2022-08-16

### Added

- Support for the `BASEFEE` instruction
- Support for solc v0.8.16

## [1.1.4] - 2022-08-08

### Added

- Better compatibility of opcodes `GASLIMIT`, `GASPRICE`, `CHAINID`, `DIFFICULTY`, `COINBASE` etc.

### Fixed

- The check for reserved function names in variable names
- An EVMLA stack inconsistency issue with the `GASPRICE` opcode

## [1.1.3] - 2022-07-16

### Added

- The extcodesize check before the method selector
- The check for the latest supportable version of `solc`
- A lot of LLVM optimizations

### Changed

- The default memory allocator for MUSL to `mimalloc`

### Fixed

- Overwriting the return data size during non-EVM far calls
- The incorrect behavior of immutables in some cases

## [1.1.2] - 2022-07-01

### Changed

- The exponentiation algorithm from linear to binary

## [1.1.1] - 2022-06-24

### Fixed

- The evaluation order of event indexed fields

## [1.1.0] - 2022-06-21

### Added

- Initial release
