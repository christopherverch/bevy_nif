#![allow(dead_code)]
use super::animation::TextKey;
use super::base::RecordLink;
use bevy::prelude::*;
use bevy::reflect::TypePath;
use std::fmt::Debug;
use std::ops::Deref;
// Represents the base NiExtraData object (which just has a name in this version)
#[derive(Asset, Clone, Debug, Default, TypePath)]
pub struct NiExtraData {
    pub name: String,
    pub next_extra_data: RecordLink, // *** ADDED this field ***
                                     // Unknown Int 1 is NOT read for v4.0.0.2
}
#[derive(Debug, Clone, Default)]
pub struct ExtraFields {
    /// Link to the next NiExtraData in the chain attached to the owner node.
    pub next_extra_data_link: RecordLink,
    /// An integer field often shown as "Bytes Remaining" or similar in NifSkope.
    /// Corresponds to mRecordSize in some C++ NIF libraries for older versions.
    pub bytes_remaining_or_record_size: u32,
}
// --- NiTextKeyExtraData now uses composition ---
#[derive(Asset, Debug, Clone, Default, TypePath)]
pub struct NiTextKeyExtraData {
    pub extra_base: ExtraFields, // Compose the base fields
    // Fields specific to NiTextKeyExtraData
    pub num_text_keys: u32, // Store the count
    pub text_keys: Vec<TextKey>,
}

// Optional: Implement Deref to access base fields easily (e.g., data.next_extra_data_link)
impl Deref for NiTextKeyExtraData {
    type Target = ExtraFields;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.extra_base
    }
}

#[derive(Asset, Debug, Clone, Default, TypePath)]
pub struct NiStringExtraData {
    pub extra_base: ExtraFields, // Compose the base fields
    // Field specific to NiStringExtraData (mData in C++)
    pub string_data: String,
}

// Optional: Implement Deref to access base fields easily
impl Deref for NiStringExtraData {
    type Target = ExtraFields;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.extra_base
    }
}
