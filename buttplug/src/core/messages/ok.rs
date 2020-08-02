// Buttplug Rust Source Code File - See https://buttplug.io for more info.
//
// Copyright 2016-2020 Nonpolynomial Labs LLC. All rights reserved.
//
// Licensed under the BSD 3-Clause license. See LICENSE file in the project root
// for full license information.

use super::*;
#[cfg(feature = "serialize-json")]
use serde::{Deserialize, Serialize};

/// Represents the Buttplug Protocol Ok message, as documented in the [Buttplug
/// Protocol Spec](https://buttplug-spec.docs.buttplug.io/status.html#ok).
#[derive(Debug, PartialEq, ButtplugMessage, Clone)]
#[cfg_attr(feature = "serialize-json", derive(Serialize, Deserialize))]
pub struct Ok {
  /// Message Id, used for matching message pairs in remote connection instances.
  #[cfg_attr(feature = "serialize-json", serde(rename = "Id"))]
  pub(super) id: u32,
}

impl Ok {
  /// Creates a new Ok message with the given Id.
  pub fn new(id: u32) -> Self {
    Self { id }
  }
}

impl Default for Ok {
  fn default() -> Self {
    Self { id: 1 }
  }
}

#[cfg(feature = "serialize-json")]
#[cfg(test)]
mod test {
  use crate::core::messages::{ButtplugCurrentSpecServerMessage, Ok};

  const OK_STR: &str = "{\"Ok\":{\"Id\":0}}";

  #[test]
  fn test_ok_serialize() {
    let ok = ButtplugCurrentSpecServerMessage::Ok(Ok::new(0));
    let js = serde_json::to_string(&ok).unwrap();
    assert_eq!(OK_STR, js);
  }

  #[test]
  fn test_ok_deserialize() {
    let union: ButtplugCurrentSpecServerMessage = serde_json::from_str(&OK_STR).unwrap();
    assert_eq!(ButtplugCurrentSpecServerMessage::Ok(Ok::new(0)), union);
  }
}
