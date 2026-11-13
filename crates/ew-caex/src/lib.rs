// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The neutral PLC exchange image and its AutomationML form (doc 10 §4.2, §4.3).
//!
//! An [`ExchangeImage`] is what a PLC exchange file says, as plain data: stations with their modules and channels,
//! the tags with the channels they name, and the bus interfaces with their subnets. [`write_aml`] writes one as
//! AutomationML AR APC on CAEX 2.15 (FR-PLC-029, FR-PLC-030), [`read_aml`] reads CAEX 2.15 and 3.0 back into one
//! under the limits of doc 14 §11 (FR-PLC-032). The crate knows no model: matching an image to a project and merging
//! it are the application's.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod image;
mod read;
mod write;

pub use ew_interop_api::{ImportError, Limits};
pub use image::{
    ChannelType, ExchangeImage, IecTags, IoType, XAddr, XChannel, XInterface, XItem, XNetwork, XNode, XPort,
    XProject, XStation, XTag,
};
pub use read::read_aml;
pub use write::{AmlOptions, ApcVersion, write_aml};
