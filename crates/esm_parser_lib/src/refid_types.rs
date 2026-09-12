use crate::{
    EsmError, EsmReader, Load,
    esm_reader::ConsumptionType,
    name_defs::RecNameBytes,
    string_interning::{find_interned, get_or_intern},
};
use std::{
    cmp::Ordering,
    fmt,
    hash::{DefaultHasher, Hash, Hasher},
};

#[derive(Clone, Eq)]
pub enum RefId {
    Empty(EmptyRefId),
    SizedString(StringRefId),
    UnsizedString(StringRefId),
    FormId(FormId),
    Generated(GeneratedRefId),
    Index(IndexRefId),
    ESM3ExteriorCell(ESM3ExteriorCellRefId),
}
#[macro_export]
macro_rules! refid {
    ($s:literal) => {{ RefId::from_str($s) }};
}

impl Hash for RefId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            // We treat Sized and Unsized as the same "category"
            // so they generate the same hash for the same characters.
            RefId::SizedString(s) | RefId::UnsizedString(s) => {
                state.write_u8(1); // Shared discriminant for ALL strings
                for b in s.0.as_bytes() {
                    // Morrowind uses case-insensitive strings
                    state.write_u8(b.to_ascii_lowercase());
                }
            }
            RefId::Empty(_) => {
                state.write_u8(0);
            }
            RefId::FormId(f) => {
                state.write_u8(3);
                f.index.hash(state);
                f.content_file.hash(state);
            }
            RefId::Generated(g) => {
                state.write_u8(4);
                g.value.hash(state);
            }
            RefId::Index(i) => {
                state.write_u8(5);
                // Hash the record type bytes and the numeric index
                i.record_type.hash(state);
                i.value.hash(state);
            }
            RefId::ESM3ExteriorCell(c) => {
                state.write_u8(6);
                c.x.hash(state);
                c.y.hash(state);
            }
        }
    }
}
impl fmt::Debug for RefId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RefId::Empty(v) => write!(f, "{}", v.to_debug_string()),
            RefId::SizedString(v) => write!(f, "SizedString: {}", v.to_debug_string()),
            RefId::UnsizedString(v) => write!(f, "UnsizedString {}", v.to_debug_string()),
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
            RefId::SizedString(str_ref) | RefId::UnsizedString(str_ref) => {
                let contained_str = str_ref.0;
                write!(f, "{:?}", contained_str)
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
            // empty RefId if the string doesn't exist
            Ok(RefId::SizedString(string_ref_id))
        } else {
            Ok(RefId::Empty(EmptyRefId))
        }
    }
}
impl PartialEq for RefId {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            // This handles Sized == Sized, Unsized == Unsized, and Sized == Unsized
            (RefId::SizedString(s1), RefId::SizedString(s2))
            | (RefId::UnsizedString(s1), RefId::UnsizedString(s2))
            | (RefId::SizedString(s1), RefId::UnsizedString(s2))
            | (RefId::UnsizedString(s1), RefId::SizedString(s2)) => s1.0.eq_ignore_ascii_case(s2.0),

            (RefId::Empty(_), RefId::Empty(_)) => true,
            (RefId::FormId(a), RefId::FormId(b)) => a == b,
            (RefId::Generated(a), RefId::Generated(b)) => a == b,
            (RefId::Index(a), RefId::Index(b)) => a == b,
            (RefId::ESM3ExteriorCell(a), RefId::ESM3ExteriorCell(b)) => a == b,
            _ => false,
        }
    }
}

impl PartialEq<str> for RefId {
    fn eq(&self, other: &str) -> bool {
        match self {
            RefId::SizedString(str_ref) | RefId::UnsizedString(str_ref) => {
                // Use eq_ignore_ascii_case for a case-insensitive comparison
                dbg!(str_ref.0.eq_ignore_ascii_case(other));
                dbg!(str_ref.0);
                dbg!(other);
                str_ref.0.eq_ignore_ascii_case(other)
            }
            _ => false,
        }
    }
}

impl PartialEq<&str> for RefId {
    fn eq(&self, other: &&str) -> bool {
        self == *other
    }
}
impl RefId {
    pub fn debug_hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.hash(&mut hasher);
        hasher.finish()
    }
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
    /// Resolves the contained string ID into a String
    /// Returns an empty string for EmptyRefId, as required for the region name logic.
    pub fn resolve_string(&self) -> String {
        match self {
            // RefId::Empty is the default, which is treated as an empty region name.
            RefId::Empty(_) => "".to_string(),
            RefId::SizedString(v) | RefId::UnsizedString(v) => v.0.to_string(),
            // For other variants (FormId, Index, etc.), which shouldn't happen for a region name,
            // we return the debug representation for safety and debugging.
            _ => format!("{:?}", self),
        }
    }
    pub fn starts_with(&self, prefix: &str) -> bool {
        match self {
            RefId::SizedString(s) | RefId::UnsizedString(s) => {
                let text = s.0;
                if prefix.len() > text.len() {
                    return false;
                }
                text[..prefix.len()].eq_ignore_ascii_case(prefix)
            }
            _ => false, // Not a string type, so it can't "start with" a string
        }
    }

    pub fn ends_with(&self, suffix: &str) -> bool {
        match self {
            RefId::SizedString(s) | RefId::UnsizedString(s) => {
                let text = s.0;
                if suffix.len() > text.len() {
                    return false;
                }
                text[text.len() - suffix.len()..].eq_ignore_ascii_case(suffix)
            }
            _ => false,
        }
    }

    pub fn contains(&self, sub_string: &str) -> bool {
        match self {
            RefId::SizedString(s) | RefId::UnsizedString(s) => {
                let text = s.0.to_ascii_lowercase();
                let search = sub_string.to_ascii_lowercase();
                text.contains(&search)
            }
            _ => false,
        }
    }
    fn variant_index(&self) -> u8 {
        match self {
            RefId::Empty(_) => 0,
            RefId::SizedString(_) => 1,
            RefId::UnsizedString(_) => 2,
            RefId::FormId(_) => 3,
            RefId::Generated(_) => 4,
            RefId::Index(_) => 5,
            RefId::ESM3ExteriorCell(_) => 6,
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
#[derive(Debug, Copy, Clone, Eq, Hash)]
pub struct StringRefId(pub &'static str);
impl PartialEq for StringRefId {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq_ignore_ascii_case(other.0)
    }
}
impl StringRefId {
    /// Creates a new StringRefId, interning the string if it doesn't already exist.
    pub fn new(s: &str) -> Self {
        Self(get_or_intern(s))
    }

    pub fn deserialize_existing(s: &str) -> Option<Self> {
        find_interned(s).map(Self)
    }

    pub fn to_debug_string(&self) -> String {
        format!("StringRefId: {:?}", self.0)
    }
    pub fn ends_with(&self, suffix: &str) -> bool {
        let s = self.0;
        if suffix.len() > s.len() {
            return false;
        }

        // Take a slice of the end of the string equal to the suffix length
        let actual_suffix = &s[s.len() - suffix.len()..];

        // case-insensitive comparison
        actual_suffix.eq_ignore_ascii_case(suffix)
    }
}

pub type FormId32 = u32;
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct FormId {
    pub index: u32,
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
impl Ord for RefId {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (RefId::Empty(a), RefId::Empty(b)) => a.cmp(b),
            (RefId::SizedString(a), RefId::SizedString(b))
            | (RefId::UnsizedString(a), RefId::UnsizedString(b)) => a.cmp(b),
            (RefId::FormId(a), RefId::FormId(b)) => a.cmp(b),
            (RefId::Generated(a), RefId::Generated(b)) => a.cmp(b),
            (RefId::Index(a), RefId::Index(b)) => a.cmp(b),
            (RefId::ESM3ExteriorCell(a), RefId::ESM3ExteriorCell(b)) => a.cmp(b),
            // If variants are different, compare based on their position in the enum
            _ => self.variant_index().cmp(&other.variant_index()),
        }
    }
}
impl PartialOrd for RefId {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for ESM3ExteriorCellRefId {
    fn cmp(&self, other: &Self) -> Ordering {
        // Compare X, then Y
        self.x.cmp(&other.x).then(self.y.cmp(&other.y))
    }
}
impl PartialOrd for ESM3ExteriorCellRefId {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for FormId {
    fn cmp(&self, other: &Self) -> Ordering {
        self.content_file
            .cmp(&other.content_file)
            .then(self.index.cmp(&other.index))
    }
}
impl PartialOrd for FormId {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for IndexRefId {
    fn cmp(&self, other: &Self) -> Ordering {
        // Compare record type bytes, then the value
        self.record_type
            .cmp(&other.record_type)
            .then(self.value.cmp(&other.value))
    }
}
impl PartialOrd for IndexRefId {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

// Ensure GeneratedRefId and EmptyRefId also get Ord
impl Ord for GeneratedRefId {
    fn cmp(&self, other: &Self) -> Ordering {
        self.value.cmp(&other.value)
    }
}
impl PartialOrd for GeneratedRefId {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for EmptyRefId {
    fn cmp(&self, _other: &Self) -> Ordering {
        Ordering::Equal
    }
}
impl PartialOrd for EmptyRefId {
    fn partial_cmp(&self, _other: &Self) -> Option<Ordering> {
        Some(Ordering::Equal)
    }
}

// --- The String Logic ---

impl Ord for StringRefId {
    fn cmp(&self, other: &Self) -> Ordering {
        // Case-insensitive comparison without allocation
        let a = self.0.chars().map(|c| c.to_ascii_lowercase());
        let b = other.0.chars().map(|c| c.to_ascii_lowercase());
        a.cmp(b)
    }
}
impl PartialOrd for StringRefId {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
