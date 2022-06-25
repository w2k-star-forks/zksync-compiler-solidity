//!
//! The LLVM build script.
//!

use std::process::Command;

///
/// The entry.
///
fn main() {
    let status = Command::new("cmake")
        .args(&[
            "-S",
            "./compiler-llvm/llvm/",
            "-B",
            "./compiler-llvm/build/",
            "-G",
            "Unix Makefiles",
            "-DCMAKE_INSTALL_PREFIX='./llvm_build/'",
            "-DCMAKE_BUILD_TYPE='Release'",
            "-DCMAKE_C_COMPILER='clang'",
            "-DCMAKE_CXX_COMPILER='clang++'",
            "-DLLVM_TARGETS_TO_BUILD='X86'",
            "-DLLVM_EXPERIMENTAL_TARGETS_TO_BUILD='SyncVM'",
            "-DLLVM_OPTIMIZED_TABLEGEN='On'",
            "-DLLVM_USE_LINKER='lld'",
            "-DLLVM_BUILD_DOCS='Off'",
            "-DLLVM_INCLUDE_DOCS='Off'",
            "-DLLVM_ENABLE_ASSERTIONS='On'",
            "-DLLVM_ENABLE_DOXYGEN='Off'",
            "-DLLVM_ENABLE_SPHINX='Off'",
            "-DLLVM_ENABLE_OCAMLDOC='Off'",
            "-DLLVM_ENABLE_BINDINGS='Off'",
        ])
        .status()
        .expect("LLVM building cmake process error");
    if !status.success() {
        panic!("LLVM building cmake error");
    }

    let threads = Command::new("nproc")
        .output()
        .expect("LLVM building nproc process error");
    let status = Command::new("make")
        .args(&[
            "-j",
            String::from_utf8_lossy(threads.stdout.as_slice()).trim(),
            "-C",
            "./compiler-llvm/build/",
        ])
        .status()
        .expect("LLVM building make process error");
    if !status.success() {
        panic!("LLVM building make error");
    }

    let status = Command::new("make")
        .args(&["-C", "./compiler-llvm/build/", "install"])
        .status()
        .expect("LLVM building make install process error");
    if !status.success() {
        panic!("LLVM building make install error");
    }
}
