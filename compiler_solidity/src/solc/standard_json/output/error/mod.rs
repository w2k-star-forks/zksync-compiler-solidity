//!
//! The `solc --standard-json` output error.
//!

pub mod source_location;

use std::str::FromStr;

use serde::Deserialize;
use serde::Serialize;

use self::source_location::SourceLocation;

///
/// The `solc --standard-json` output error.
///
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Error {
    /// The component type.
    pub component: String,
    /// The error code.
    pub error_code: Option<String>,
    /// The formatted error message.
    pub formatted_message: String,
    /// The non-formatted error message.
    pub message: String,
    /// The error severity.
    pub severity: String,
    /// The error location data.
    pub source_location: Option<SourceLocation>,
    /// The error type.
    pub r#type: String,
}

impl Error {
    ///
    /// Returns the `ecrecover` usage warning.
    ///
    pub fn warning_ecrecover(src: Option<&str>) -> Self {
        let message = r#"
┌──────────────────────────────────────────────────────────────────────────────────────────────────┐
│ Warning: It looks like you are using 'ecrecover' to validate a signature of a user account.      │
│ zkSync 2.0 will come with the native account abstraction support. It is highly recommended NOT   │
│ to rely on the fact that the account has an ECDSA private key attached to it, since they may be  │
│ ruled by a multisig and use another signature scheme. You can read more about how you can get    │
│ ready for the future launch of AA here:                                                          │
│ https://v2-docs.zksync.io/dev/zksync-v2/aa.html#important-account-abstraction-support            │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘"#
            .to_owned();

        Self {
            component: "general".to_owned(),
            error_code: None,
            formatted_message: message.clone(),
            message,
            severity: "warning".to_owned(),
            source_location: src.map(SourceLocation::from_str).and_then(Result::ok),
            r#type: "Warning".to_owned(),
        }
    }

    ///
    /// Returns the `<address payable>.send(0)` usage warning.
    ///
    pub fn warning_send_zero_ether(src: Option<&str>) -> Self {
        let message = r#"
┌──────────────────────────────────────────────────────────────────────────────────────────────────┐
│ Warning: It looks like you are using '<address payable>.send(0)'.                                │
│ Such calls may fail depending or the pubdata costs, or if the receiver is a smart contract or    │
│ account abstraction.                                                                             │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘"#
            .to_owned();

        Self {
            component: "general".to_owned(),
            error_code: None,
            formatted_message: message.clone(),
            message,
            severity: "warning".to_owned(),
            source_location: src.map(SourceLocation::from_str).and_then(Result::ok),
            r#type: "Warning".to_owned(),
        }
    }

    ///
    /// Returns the `extcodesize` usage warning.
    ///
    pub fn warning_extcodesize(src: Option<&str>) -> Self {
        let message = r#"
┌──────────────────────────────────────────────────────────────────────────────────────────────────┐
│ Warning: It looks like your code or one of its dependencies uses the 'extcodesize' instruction.  │
│ It is usually used in the following cases:                                                       │
│     1. To detect whether an address belongs to a smart contract.                                 │
│     2. To detect whether the deploy code execution has finished.                                 │
│ zkSync 2.0 will support the account abstraction by default (so every account will be a smart     │
│ contract, even private-key controlled EOAs). So none of the use-cases above will work.           │
│ Please do your best to avoid differentiating between contracts and non-contracts.                │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘"#
            .to_owned();

        Self {
            component: "general".to_owned(),
            error_code: None,
            formatted_message: message.clone(),
            message,
            severity: "warning".to_owned(),
            source_location: src.map(SourceLocation::from_str).and_then(Result::ok),
            r#type: "Warning".to_owned(),
        }
    }

    ///
    /// Appends the contract path to the message..
    ///
    pub fn push_contract_path(&mut self, path: &str) {
        self.formatted_message
            .push_str(format!("\n--> {}\n", path).as_str());
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.formatted_message)
    }
}
