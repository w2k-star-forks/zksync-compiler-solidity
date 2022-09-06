//!
//! The LLVM builder arguments.
//!

use structopt::StructOpt;

///
/// The LLVM builder arguments.
///
#[derive(Debug, StructOpt)]
#[structopt(name = "llvm-builder", about = "The LLVM framework builder")]
pub struct Arguments {
    /// The LLVM framework tag.
    #[structopt(long = "tag")]
    pub tag: Option<String>,
}

impl Arguments {
    ///
    /// A shortcut constructor.
    ///
    pub fn new() -> Self {
        Self::from_args()
    }
}
