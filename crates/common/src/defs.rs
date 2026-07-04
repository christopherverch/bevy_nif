use bevy_ecs::resource::Resource;

#[derive(Resource, Default)]
pub struct NextAvailableSkeletonId(pub u64);
#[derive(Resource)]
pub struct ServerIP(pub String);
