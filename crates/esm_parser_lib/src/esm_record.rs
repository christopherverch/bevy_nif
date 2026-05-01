use crate::RefId;

pub trait EsmRecord {
    fn id(&self) -> &RefId;
}
