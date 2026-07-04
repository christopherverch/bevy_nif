use crate::RefId;

pub trait EsmRecord {
    fn id(&self) -> &RefId;
    // The 4 bytes of the record id, i.e. WEAP
    const RECORD_ID: u32;
}
