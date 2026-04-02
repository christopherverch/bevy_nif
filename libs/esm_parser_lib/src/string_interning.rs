use lazy_static::lazy_static;
use std::borrow::Borrow;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::sync::Mutex;

#[derive(Debug, Eq)]
struct CiString(Box<str>);

// Implement Hash/Eq to be case-insensitive
impl Hash for CiString {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for b in self.0.as_bytes() {
            state.write_u8(b.to_ascii_lowercase());
        }
    }
}

impl PartialEq for CiString {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq_ignore_ascii_case(&other.0)
    }
}

// This allows us to look up in the HashSet using a simple &str
// instead of allocating a Box<str> just to check if it exists.
impl Borrow<str> for CiString {
    fn borrow(&self) -> &str {
        &self.0
    }
}

lazy_static! {
    // We store the CiString directly. The HashSet owns the Boxed str.
    static ref REF_IDS: Mutex<HashSet<CiString>> = Mutex::new(HashSet::new());
}

pub fn get_or_intern(s: &str) -> &'static str {
    let mut locked = REF_IDS.lock().unwrap();

    if let Some(existing) = locked.get(s) {
        // We cast the reference to 'static. This is safe because
        // elements are never removed from the global HashSet.
        return unsafe { std::mem::transmute::<&str, &'static str>(&existing.0) };
    }

    // Only allocate and leak if the string wasn't found.
    let boxed: Box<str> = s.to_owned().into_boxed_str();
    let static_ref: &'static str = Box::leak(boxed);

    // Wrap the leaked str in the case-insensitive wrapper and insert.
    locked.insert(CiString(Box::from(static_ref)));

    static_ref
}
pub fn find_interned(s: &str) -> Option<&'static str> {
    let ref_ids = REF_IDS.lock().unwrap();
    ref_ids
        .get(s)
        .map(|existing| unsafe { std::mem::transmute::<&str, &'static str>(&existing.0) })
}
