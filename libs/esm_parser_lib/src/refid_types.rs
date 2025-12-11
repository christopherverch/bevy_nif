use crate::{EsmError, EsmReader, Load, esm_reader::ConsumptionType, name_defs::RecNameBytes};
use lazy_static::lazy_static;
use std::{fmt, hash::Hash, sync::Mutex};
use string_interner::{DefaultStringInterner, symbol::SymbolU32};

#[derive(Clone, PartialEq, Eq, Hash)]
pub enum RefId {
    Empty(EmptyRefId),
    SizedString(StringRefId),
    UnsizedString(StringRefId),
    FormId(FormId),
    Generated(GeneratedRefId),
    Index(IndexRefId),
    ESM3ExteriorCell(ESM3ExteriorCellRefId),
}
impl fmt::Debug for RefId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RefId::Empty(v) => write!(f, "{}", v.to_debug_string()),
            RefId::SizedString(v) => write!(f, "{}", v.to_debug_string()),
            RefId::UnsizedString(v) => write!(f, "{}", v.to_debug_string()),
            RefId::FormId(v) => write!(f, "{}", v.to_debug_string()),
            RefId::Generated(v) => write!(f, "{}", v.to_debug_string()),
            RefId::Index(v) => write!(f, "{}", v.to_debug_string()),
            RefId::ESM3ExteriorCell(v) => write!(f, "{}", v.to_debug_string()),
        }
    }
}

impl Default for RefId {
    fn default() -> Self {
        RefId::Empty(EmptyRefId::default())
    }
}

impl fmt::Display for RefId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RefId::Empty(_empty_ref_id) => write!(f, ""),
            RefId::SizedString(string_ref_id) | RefId::UnsizedString(string_ref_id) => {
                let interner_guard = INTERNER.lock().unwrap();
                let contained_string = interner_guard.resolve(string_ref_id.0);
                write!(f, "{:?}", contained_string)
            }
            RefId::FormId(form_id) => write!(f, "FormId({:?})", form_id),
            RefId::Generated(generated_ref_id) => write!(f, "Generated({:?})", generated_ref_id),
            RefId::Index(index_ref_id) => write!(f, "Index({:?})", index_ref_id),
            RefId::ESM3ExteriorCell(cell_ref_id) => write!(f, "Cell({:?})", cell_ref_id),
        }
    }
}
impl TryFrom<&str> for RefId {
    type Error = EsmError;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        if s.starts_with("FormId:") {
            let hex_value = &s["FormId:".len()..];
            let v = u64::from_str_radix(hex_value, 16)
                .map_err(|e| EsmError::Custom(format!("Failed to parse FormId hex: {}", e)))?;

            let index: u32 = (v & 0x00ffffff) as u32;
            let content_file: i32 = (v >> 24) as i32;

            return Ok(RefId::FormId(FormId::new(index, Some(content_file as i16))));
        }
        if s.starts_with("Generated:") {
            let hex_value = &s["Generated:".len()..];
            let value = u64::from_str_radix(hex_value, 16)
                .map_err(|e| EsmError::Custom(format!("Failed to parse Generated hex: {}", e)))?;
            return Ok(RefId::Generated(GeneratedRefId::new(value)));
        }
        if s.starts_with("Index:") {
            let parts: Vec<&str> = s["Index:".len()..].split(':').collect();
            if parts.len() == 2 {
                let (rec_name_str, value_str) = (parts[0], parts[1]);

                let rec_name = RecNameBytes::try_from(rec_name_str)
                    .map_err(|e| EsmError::Custom(format!("Invalid Index record name: {}", e)))?;

                let value = u32::from_str_radix(value_str, 16)
                    .map_err(|e| EsmError::Custom(format!("Failed to parse Index value: {}", e)))?;

                return Ok(RefId::Index(IndexRefId::new(rec_name, value)));
            }
            return Err(EsmError::Custom("Invalid Index RefId format".to_string()));
        }
        if s.starts_with("Esm3ExteriorCell:") {
            let parts: Vec<&str> = s["Esm3ExteriorCell:".len()..].split(':').collect();
            if parts.len() == 2 {
                let x = parts[0].parse::<i32>().map_err(|e| {
                    EsmError::Custom(format!("Failed to parse x coordinate: {}", e))
                })?;
                let y = parts[1].parse::<i32>().map_err(|e| {
                    EsmError::Custom(format!("Failed to parse y coordinate: {}", e))
                })?;
                return Ok(RefId::ESM3ExteriorCell(ESM3ExteriorCellRefId::new(x, y)));
            }
        }

        // Fallback to StringRefId
        if let Some(string_ref_id) = StringRefId::deserialize_existing(s) {
            // The C++ code returns an empty ID if the string doesn't exist
            Ok(RefId::SizedString(string_ref_id))
        } else {
            Ok(RefId::Empty(EmptyRefId))
        }
    }
}

// Implementing PartialEq<str> for RefId for convenience
impl PartialEq<str> for RefId {
    fn eq(&self, other: &str) -> bool {
        match self {
            RefId::SizedString(string_ref_id) | RefId::UnsizedString(string_ref_id) => {
                let interner_guard = match INTERNER.lock() {
                    Ok(guard) => guard,
                    Err(_) => return false, // Mutex is poisoned, cannot proceed
                };

                let contained_string = interner_guard.resolve(string_ref_id.0);

                // Use eq_ignore_ascii_case for a case-insensitive comparison
                contained_string.map_or(false, |s| s.eq_ignore_ascii_case(other))
            }
            _ => false,
        }
    }
}

// Implement PartialEq<&str> via the existing PartialEq<str> implementation
impl PartialEq<&str> for RefId {
    fn eq(&self, other: &&str) -> bool {
        self == *other
    }
}
impl RefId {
    // Other methods like `to_debug_string` go here...

    /// Checks if the RefId is an empty variant.
    pub fn is_empty(&self) -> bool {
        matches!(self, RefId::Empty(_))
    }

    /// Creates a RefId from an owned String, using a StringRefId variant.
    pub fn from_str(value: &str) -> Self {
        RefId::SizedString(StringRefId::new(&value))
    }

    /// Creates a RefId from a u64, using a GeneratedRefId variant.
    pub fn from_generated(value: u64) -> Self {
        RefId::Generated(GeneratedRefId::new(value))
    }

    /// Creates a RefId from a RecNameBytes and u32, using an IndexRefId variant.
    pub fn from_index(record_type: RecNameBytes, value: u32) -> Self {
        RefId::Index(IndexRefId::new(record_type, value))
    }

    /// Creates a RefId for an exterior cell, using an ESM3ExteriorCellRefId variant.
    pub fn from_esm3_exterior_cell(x: i32, y: i32) -> Self {
        RefId::ESM3ExteriorCell(ESM3ExteriorCellRefId::new(x, y))
    }
    /// Resolves the contained string ID into a String using the global INTERNER.
    /// Returns an empty string for EmptyRefId, as required for the region name logic.
    pub fn resolve_string(&self) -> String {
        match self {
            // RefId::Empty is the C++ ESM::RefId() default, which is treated as an empty region name.
            RefId::Empty(_) => "".to_string(),

            // If it's a string ID, lock the global interner and resolve the symbol.
            RefId::SizedString(v) | RefId::UnsizedString(v) => {
                let interner_guard = INTERNER.lock().unwrap();
                interner_guard
                    .resolve(v.0) // Assuming v.0 is the SymbolU32, based on to_debug_string
                    .unwrap_or("<Missing String>") // Fallback for safety
                    .to_string()
            }
            // For other variants (FormId, Index, etc.), which shouldn't happen for a region name,
            // we return the debug representation for safety and debugging.
            _ => format!("{:?}", self),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct EmptyRefId;

impl EmptyRefId {
    pub fn to_debug_string(&self) -> String {
        "Empty{}".to_string()
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ESM3ExteriorCellRefId {
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
}
lazy_static! {
    pub static ref INTERNER: Mutex<DefaultStringInterner> =
        Mutex::new(DefaultStringInterner::new());
}

/// This is the Rust equivalent of the C++ `StringRefId` class.
/// It holds a shared reference (`Arc`) to a unique string in the global pool.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StringRefId(pub SymbolU32);

impl StringRefId {
    /// Creates a new StringRefId, interning the string if it doesn't already exist.
    pub fn new(s: &str) -> Self {
        // Lock the global interner and get the symbol.
        let symbol = INTERNER.lock().unwrap().get_or_intern(s);

        // Return a new StringRefId containing the symbol.
        Self(symbol)
    }
    pub fn deserialize_existing(s: &str) -> Option<Self> {
        // Lock the global interner and get the symbol.
        let symbol = INTERNER.lock().unwrap().get(s);

        // If a symbol was found, return a new StringRefId with it.
        symbol.map(|sym| Self(sym))
    }

    pub fn to_debug_string(&self) -> String {
        let interner_guard = INTERNER.lock().unwrap();
        let contained_string = interner_guard.resolve(self.0);

        format!("StringRefId: {:?}", contained_string)
    }
}
pub type FormId32 = u32;
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct FormId {
    pub index: u32,
    // Corresponds to C++ 'int32_t mContentFile = -1;'
    pub content_file: Option<i16>,
}
impl Load for FormId {
    fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        let index: u32 = reader.read_typed(ConsumptionType::SubrecordPayload)?;
        let content_file: i32 = reader.read_typed(ConsumptionType::SubrecordPayload)?;

        // The C++ `int32_t` can be converted to an Option<i16>.
        let rust_content_file = if content_file == -1 {
            None
        } else {
            // We know from the C++ code that a valid content file will fit in i16
            Some(content_file as i16)
        };

        Ok(FormId::new(index, rust_content_file))
    }
}
impl FormId {
    pub fn new(index: u32, content_file: Option<i16>) -> Self {
        Self {
            index,
            content_file,
        }
    }
    pub fn has_content_file(&self) -> bool {
        self.content_file.is_some()
    }
    pub fn is_set(&self) -> bool {
        self.index != 0 || self.content_file.is_some()
    }
    pub fn is_zero_or_unset(&self) -> bool {
        if self.index != 0 {
            return false;
        }
        match self.content_file {
            Some(v) => v == 0,
            None => true,
        }
    }
    pub fn to_u32(&self) -> Result<FormId32, EsmError> {
        if self.is_set() && !self.has_content_file() {
            return Err(EsmError::InvalidFormIdConversion(
                "Generated FormId can not be converted to 32bit format".to_string(),
            ));
        }
        if let Some(v) = self.content_file {
            if v > 0xfe {
                return Err(EsmError::InvalidFormIdConversion(
                    "FormId with mContentFile > 0xFE can not be converted to 32bit format"
                        .to_string(),
                ));
            }
        }

        let content_file_part = match self.content_file {
            Some(v) => v as u32,
            None => 0xff,
        };
        Ok((self.index & 0xffffff) | (content_file_part << 24))
    }

    pub const fn from_u32(v: FormId32) -> Self {
        let content_file = (v >> 24) as i16;
        // TODO:: Legacy from C++ code, not sure how this will be saved or loaded, this still uses
        // -1 sentinel value
        let content_file = if content_file == -1 {
            None
        } else {
            Some(content_file)
        };

        Self {
            index: v & 0xffffff,
            content_file,
        }
    }

    pub fn to_string_with_prefix(&self, prefix: &str) -> Result<String, EsmError> {
        let mut res = String::with_capacity(prefix.len() + 20);
        res.push_str(prefix);

        let value_str = if self.has_content_file() {
            if (self.index & 0xff000000) != 0 {
                return Err(EsmError::InvalidFormIdConversion(format!(
                    "Invalid FormId index value: {}",
                    self.index
                )));
            }
            let value = (self.index as u64) | ((self.content_file.unwrap() as u64) << 24);
            format!("0x{:x}", value)
        } else {
            let content_file_val = if let Some(v) = self.content_file {
                v
            } else {
                -1
            };
            let value = (self.index as u64) | (((-content_file_val - 1) as u64) << 32);
            format!("@0x{:x}", value)
        };

        res.push_str(&value_str);
        Ok(res)
    }
    pub fn to_debug_string(&self) -> String {
        format!("FormId: {}, {:?}", self.index, self.content_file)
    }
}
impl fmt::Display for FormId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let result = self.to_string_with_prefix("");
        match result {
            Ok(s) => write!(f, "{}", s),
            Err(e) => write!(f, "{}", e),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct GeneratedRefId {
    pub value: u64,
}

impl GeneratedRefId {
    pub fn new(value: u64) -> Self {
        Self { value }
    }
    pub fn to_debug_string(&self) -> String {
        format!("Generated:0x{:X}", self.value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IndexRefId {
    pub record_type: RecNameBytes,
    pub value: u32,
}

impl IndexRefId {
    pub fn new(record_type: RecNameBytes, value: u32) -> Self {
        Self { record_type, value }
    }
    pub fn to_debug_string(&self) -> String {
        format!(
            "Index:{}:0x{:X}",
            self.record_type.to_string().to_uppercase(),
            self.value
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RefIdType {
    Empty,
    SizedString,
    UnsizedString,
    FormId,
    Generated,
    Index,
    ESM3ExteriorCell,
}
impl TryFrom<u8> for RefIdType {
    type Error = EsmError;

    fn try_from(v: u8) -> Result<Self, Self::Error> {
        match v {
            0 => Ok(RefIdType::Empty),
            1 => Ok(RefIdType::SizedString),
            2 => Ok(RefIdType::UnsizedString),
            3 => Ok(RefIdType::FormId),
            4 => Ok(RefIdType::Generated),
            5 => Ok(RefIdType::Index),
            6 => Ok(RefIdType::ESM3ExteriorCell),
            _ => Err(EsmError::InvalidRefIdType(v)),
        }
    }
}
