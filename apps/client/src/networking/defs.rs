use bevy::ecs::resource::Resource;

#[derive(Resource)]
pub struct ServerIP(pub String);
