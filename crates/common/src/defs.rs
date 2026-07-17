use bevy_ecs::resource::Resource;

#[derive(Resource, Default)]
pub struct NextAvailableSkeletonId(u64);
impl NextAvailableSkeletonId {
    pub fn next(&mut self) -> u64 {
        self.0 += 1;
        self.0
    }
}
#[derive(Resource)]
pub struct ServerIP(pub String);
