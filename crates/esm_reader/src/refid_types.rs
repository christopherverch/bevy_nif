// refid_types.rs

use crate::{EsmError, common_types::Name}; // Assuming common_types.rs is in the same crate/module
use std::fmt;
// For potential text deserialization

// --- Forward declare/stub dependent types for RefId variants ---
// These would be fully defined in their own modules or here if simple enough.

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ESM3ExteriorCellRefId {
    // Presumed fields based on RefId::esm3ExteriorCell(x, y)
    pub x: i32,
    pub y: i32,
}

impl ESM3ExteriorCellRefId {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
    pub fn to_debug_string(&self) -> String {
        format!("ESM3ExteriorCell{{x: {}, y: {}}}", self.x, self.y)
    }
    pub fn to_string(&self) -> String {
        // Simplified toString for now
        format!("{},{}", self.x, self.y)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct StringRefId {
    // Presumed to hold a string. The C++ version mentions "pointer to a static set of strings"
    // which implies it might be an interned string or similar. For now, an owned String.
    pub value: String,
}

impl StringRefId {
    pub fn new(s: &str) -> Self {
        Self {
            value: s.to_string(),
        }
    }
    pub fn from_string(s: String) -> Self {
        Self { value: s }
    }
    pub fn to_debug_string(&self) -> String {
        format!("StringRefId{{\"{}\"}}", self.value)
    }
    pub fn as_str(&self) -> &str {
        &self.value
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct GeneratedRefId {
    pub value: u64,
}
impl GeneratedRefId {
    pub fn to_debug_string(&self) -> String {
        format!("GeneratedRefId{{{}}}", self.value)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct IndexRefId {
    // RecNameInts in C++ is likely Name (FixedString<4>)
    pub record_type: Name,
    pub index: u32,
}
impl IndexRefId {
    pub fn new(record_type: Name, index: u32) -> Self {
        Self { record_type, index }
    }
    pub fn to_debug_string(&self) -> String {
        format!(
            "IndexRefId{{type: {}, index: {}}}",
            self.record_type.to_string_lossy(),
            self.index
        )
    }
}

// --- FormId definition ---
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FormId {
    pub index: u32,
    // In C++, mContentFile is an int, often representing an index into master files.
    // -1 or another sentinel might mean "current file" or "not set".
    // Using Option<u32> or a sentinel like u32::MAX could work.
    // The C++ `RefId(FormId value)` constructor logic checks `hasContentFile()`.
    // And `ESMReader::applyContentFileMapping` also checks it.
    // Let's use u32::MAX as "unset" or "generated for current file".
    pub content_file_index: u32, // 0 for current file, 1+ for masters, u32::MAX for unset/generated
}

impl FormId {
    pub const UNSET_CONTENT_FILE: u32 = u32::MAX;

    // The second parameter name `content_file_idx_param` is used here
    // to clearly distinguish from the field name `content_file_index`.
    // If your parameter is named `content_file_index`, the assignment would be:
    // content_file_index: content_file_index,
    pub fn new(index: u32, content_file_idx_param: u32) -> Self {
        Self {
            index,
            content_file_index: content_file_idx_param, // Correct assignment
        }
    }

    // ... (is_zero_or_unset, has_content_file, to_debug_string methods remain the same) ...
    /// Checks if the FormId is considered zero or effectively unset.
    pub fn is_zero_or_unset(&self) -> bool {
        self.index == 0 && self.content_file_index == Self::UNSET_CONTENT_FILE
    }

    /// Checks if the FormId refers to a specific content file (master or plugin).
    pub fn has_content_file(&self) -> bool {
        self.content_file_index != Self::UNSET_CONTENT_FILE
    }
    pub fn to_debug_string(&self) -> String {
        format!(
            "FormId{{idx:0x{:08X}, file_idx:{}}}",
            self.index,
            if self.content_file_index == Self::UNSET_CONTENT_FILE {
                "UNSET".to_string()
            } else {
                self.content_file_index.to_string()
            }
        )
    }
}

// --- RefId and related types from refid.hpp ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct EmptyRefId;

impl EmptyRefId {
    pub fn to_string(&self) -> String {
        String::new()
    }
    pub fn to_debug_string(&self) -> String {
        "Empty{}".to_string()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum RefIdType {
    Empty = 0,
    SizedString = 1,   // String prefix- Polizeid by StringSizeType
    UnsizedString = 2, // String fills the rest of the subrecord
    FormId = 3,
    Generated = 4,
    Index = 5,
    ESM3ExteriorCell = 6,
}
impl TryFrom<u8> for RefIdType {
    type Error = EsmError; // Use the error type defined above

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(RefIdType::Empty),
            1 => Ok(RefIdType::SizedString),
            2 => Ok(RefIdType::UnsizedString),
            3 => Ok(RefIdType::FormId),
            4 => Ok(RefIdType::Generated),
            5 => Ok(RefIdType::Index),
            6 => Ok(RefIdType::ESM3ExteriorCell),
            other => Err(EsmError::Custom(other.to_string())), // Value doesn't match any variant
        }
    }
}

impl Default for RefIdType {
    fn default() -> Self {
        RefIdType::Empty
    }
}
// The main RefId structure, using a Rust enum to represent the C++ std::variant
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RefIdValue {
    Empty(EmptyRefId),
    Esm3ExteriorCell(ESM3ExteriorCellRefId),
    String(StringRefId), // Wraps the actual string logic
    Form(FormId),
    Generated(GeneratedRefId),
    Index(IndexRefId),
}

impl Default for RefIdValue {
    fn default() -> Self {
        RefIdValue::Empty(EmptyRefId::default())
    }
}

#[derive(Clone, Default, PartialEq, Eq, Hash)]
pub struct RefId {
    pub(crate) m_value: RefIdValue, // Made crate-public for ESMReader to construct
}

impl fmt::Debug for RefId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_debug_string())
    }
}

impl RefId {
    /// Constructs an empty RefId.
    pub fn new() -> Self {
        Self {
            m_value: RefIdValue::Empty(EmptyRefId),
        }
    }

    /// Constructs RefId from a string.
    /// In C++, `stringRefId` implies a specific handling, possibly interning.
    /// For now, it creates a `StringRefId` variant.
    pub fn string_ref_id(value: &str) -> Self {
        if value.is_empty() {
            // Optimization or rule: empty string means EmptyRefId
            Self::new()
        } else {
            Self {
                m_value: RefIdValue::String(StringRefId::new(value)),
            }
        }
    }

    pub fn from_string_owned(value: String) -> Self {
        if value.is_empty() {
            Self::new()
        } else {
            Self {
                m_value: RefIdValue::String(StringRefId::from_string(value)),
            }
        }
    }

    /// Constructs RefId from FormId.
    pub fn from_form_id(value: FormId) -> Result<Self, String> {
        // Changed to Result
        if value.is_zero_or_unset() {
            Ok(Self {
                m_value: RefIdValue::Empty(EmptyRefId),
            })
        } else if value.has_content_file() {
            Ok(Self {
                m_value: RefIdValue::Form(value),
            })
        } else {
            // This case ("RefId can't be a generated FormId") implies that a FormId
            // without a content file index (other than 0 for current or UNSET)
            // is considered "generated" in the context of FormId itself,
            // and such FormIds aren't directly wrapped by RefId::Form.
            // They might become RefId::Generated through a different path.
            Err("RefId can't be a FormId that is 'generated' (i.e. has no specific content file index and is not zero/unset)".to_string())
        }
    }

    /// Alias for from_form_id for C++ parity, assuming successful creation
    pub fn form_id_ref_id(value: FormId) -> Self {
        Self::from_form_id(value).unwrap_or_else(|_| {
            // This path should ideally not be taken if FormId is well-formed before this call
            // or the error indicates a logic flaw. For now, default to Empty on error.
            // The C++ throws std::logic_error.
            // Consider how to handle this panic/error more gracefully.
            // For direct translation of behavior that might throw, panic is an option.
            panic!("RefId can't be a FormId that is 'generated'");
        })
    }

    /// Constructs RefId from a u64 for generated records.
    pub fn generated(value: u64) -> Self {
        Self {
            m_value: RefIdValue::Generated(GeneratedRefId { value }),
        }
    }

    /// Constructs RefId from record type and 32-bit index.
    pub fn index(record_type: Name, value: u32) -> Self {
        Self {
            m_value: RefIdValue::Index(IndexRefId::new(record_type, value)),
        }
    }

    /// Constructs RefId for an ESM3 exterior cell.
    pub fn esm3_exterior_cell(x: i32, y: i32) -> Self {
        Self {
            m_value: RefIdValue::Esm3ExteriorCell(ESM3ExteriorCellRefId::new(x, y)),
        }
    }

    pub fn get_ref_id_string(&self) -> Result<&str, String> {
        match &self.m_value {
            RefIdValue::String(sr_id) => Ok(sr_id.as_str()),
            _ => Err("RefId does not hold a StringRefId".to_string()),
        }
    }

    pub fn to_string(&self) -> String {
        match &self.m_value {
            RefIdValue::Empty(e) => e.to_string(),
            RefIdValue::Esm3ExteriorCell(e) => e.to_string(), // Needs proper formatting
            RefIdValue::String(s) => s.as_str().to_string(),
            RefIdValue::Form(f) => f.to_debug_string(), // Needs proper formatting
            RefIdValue::Generated(g) => g.value.to_string(),
            RefIdValue::Index(i) => format!("{}:{}", i.record_type.to_string_lossy(), i.index),
        }
    }

    pub fn to_debug_string(&self) -> String {
        match &self.m_value {
            RefIdValue::Empty(e) => e.to_debug_string(),
            RefIdValue::Esm3ExteriorCell(e) => e.to_debug_string(),
            RefIdValue::String(s) => s.to_debug_string(),
            RefIdValue::Form(f) => f.to_debug_string(),
            RefIdValue::Generated(g) => g.to_debug_string(),
            RefIdValue::Index(i) => i.to_debug_string(),
        }
    }

    pub fn empty(&self) -> bool {
        matches!(self.m_value, RefIdValue::Empty(_))
    }

    pub fn starts_with(&self, prefix: &str) -> bool {
        match &self.m_value {
            RefIdValue::String(s) => s.as_str().starts_with(prefix),
            _ => false,
        }
    }

    pub fn ends_with(&self, suffix: &str) -> bool {
        match &self.m_value {
            RefIdValue::String(s) => s.as_str().ends_with(suffix),
            _ => false,
        }
    }

    pub fn contains(&self, sub_string: &str) -> bool {
        match &self.m_value {
            RefIdValue::String(s) => s.as_str().contains(sub_string),
            _ => false,
        }
    }

    // getIf<T> and is<T> can be implemented using pattern matching
    pub fn get_if_form_id(&self) -> Option<&FormId> {
        if let RefIdValue::Form(ref form_id) = self.m_value {
            Some(form_id)
        } else {
            None
        }
    }
    pub fn is_form_id(&self) -> bool {
        matches!(self.m_value, RefIdValue::Form(_))
    }

    pub fn get_if_string(&self) -> Option<&StringRefId> {
        if let RefIdValue::String(ref str_id) = self.m_value {
            Some(str_id)
        } else {
            None
        }
    }
    pub fn is_string(&self) -> bool {
        matches!(self.m_value, RefIdValue::String(_))
    }
    // ... and so on for other types

    // Serialization/Deserialization:
    // These are complex and depend on the exact binary/text format.
    // For now, they are placeholders. `ESMReader::getRefIdImpl` handles the direct reading.
    #[allow(dead_code)]
    pub fn serialize(&self) -> Vec<u8> {
        // Placeholder: Actual binary serialization of m_value
        self.to_debug_string().into_bytes()
    }
    #[allow(dead_code)]
    pub fn deserialize(_value: &[u8]) -> Result<Self, String> {
        // Placeholder: Actual binary deserialization
        Err("deserialize not implemented".to_string())
    }
    #[allow(dead_code)]
    pub fn serialize_text(&self) -> String {
        // Placeholder: Actual text serialization
        self.to_debug_string()
    }
    #[allow(dead_code)]
    pub fn deserialize_text(_value: &str) -> Result<Self, String> {
        // Placeholder: Actual text deserialization
        Err("deserialize_text not implemented".to_string())
    }
}

// Implementing PartialEq<str> for RefId for convenience (as in C++)
impl PartialEq<str> for RefId {
    fn eq(&self, other: &str) -> bool {
        match &self.m_value {
            RefIdValue::String(s_ref_id) => s_ref_id.as_str() == other,
            // Potentially other variants could be compared to string representations if defined
            _ => false,
        }
    }
}
impl PartialEq<&str> for RefId {
    fn eq(&self, other: &&str) -> bool {
        match &self.m_value {
            RefIdValue::String(s_ref_id) => s_ref_id.as_str() == *other,
            _ => false,
        }
    }
}

// std::hash for RefId is provided in C++.
// Rust's Hash trait would be implemented for RefId (and RefIdValue and its variants).
// The auto-derive `Hash` for the enum and its fields should work if all fields are Hash.
