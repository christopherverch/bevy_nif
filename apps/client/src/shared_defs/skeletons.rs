use bevy::prelude::*;
#[derive(Resource, Default)]
pub struct NextAvailableSkeletonId(u64);
impl NextAvailableSkeletonId {
    pub fn next(&mut self) -> u64 {
        self.0 += 1;
        self.0
    }
}
