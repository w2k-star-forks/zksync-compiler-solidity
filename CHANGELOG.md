# The `zksolc` changelog

## Version 1.1.4 (2022-XX-XX)



## Version 1.1.3 (2022-07-16)

- added the extcodesize check before the method selector
- added the check for the latest supportable version of `solc`
- added a lot of LLVM optimizations
- fixed overwriting the return data size during non-EVM far calls
- fixed the incorrect behavior of immutables in some cases
- changed the default memory allocator for MUSL to `mimalloc`

## Version 1.1.2 (2022-07-01)

- changed the linear exponentiation algorithm with the binary one

## Version 1.1.1 (2022-06-24)

- fixed the evaluation order of event indexed fields

## Version 1.1.0 (2022-06-21)

*Initial release*
