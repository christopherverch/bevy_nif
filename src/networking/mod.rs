use std::{collections::HashMap, f32::consts::PI, time::Duration};

use bevy::prelude::*;
use bevy_renet::renet::{ChannelConfig, ConnectionConfig, SendType};
use serde::{Deserialize, Serialize};

use crate::{EntityId, NetworkedActorState, serializer::SerializableTransform};

pub const PRIVATE_KEY: &[u8; bevy_renet::netcode::NETCODE_KEY_BYTES] =
    b"an example very very secret key."; // 32-bytes
pub const PROTOCOL_ID: u64 = 7;

pub enum ClientChannel {
    EntityTransforms,
    CriticalActions,
    State,
}
pub enum ServerChannel {
    CriticalAcknowledgments,
    CriticalRelay,
    ServerMessages,
    NetworkedEntities,
    StateUpdates,
}

#[derive(Debug, Default, Component)]
pub struct Velocity(pub Vec3);

#[derive(Debug, Serialize, Deserialize, Component)]
pub enum ServerMessages {
    PlayerCreate {
        entity_id: EntityId,
        translation: [f32; 3],
    },
    PlayerRemove {
        entity_id: EntityId,
    },

    /// Relays the animation state of a specific player.
    StateUpdate {
        /// The ID of the entity whose state this is.
        entity_id: EntityId,
        /// The new state of that entity.
        state: NetworkedActorState,
    },
    /// Tells the player what their server-assigned entity id is
    Welcome {
        entity_id: EntityId,
    },
}
#[derive(Debug, Serialize, Deserialize)]
pub struct NetworkedEntityData {
    pub transform: SerializableTransform,
}
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct NetworkedEntityDataSnapshot {
    pub entity_data_map: HashMap<EntityId, NetworkedEntityData>,
}
impl From<ClientChannel> for u8 {
    fn from(channel_id: ClientChannel) -> Self {
        match channel_id {
            ClientChannel::EntityTransforms => 0,
            ClientChannel::CriticalActions => 1,
            ClientChannel::State => 2,
        }
    }
}

impl ClientChannel {
    pub fn channels_config() -> Vec<ChannelConfig> {
        vec![
            ChannelConfig {
                channel_id: Self::EntityTransforms.into(),
                max_memory_usage_bytes: 5 * 1024 * 1024,
                // We don't care if positions jump a little for networked entities
                send_type: SendType::Unreliable,
            },
            ChannelConfig {
                channel_id: Self::CriticalActions.into(),
                max_memory_usage_bytes: 5 * 1024 * 1024,
                send_type: SendType::ReliableOrdered {
                    resend_time: Duration::from_millis(250),
                },
            },
            ChannelConfig {
                channel_id: Self::State.into(),
                max_memory_usage_bytes: 5 * 1024 * 1024,
                send_type: SendType::ReliableOrdered {
                    resend_time: Duration::from_millis(250),
                },
            },
        ]
    }
}

impl From<ServerChannel> for u8 {
    fn from(channel_id: ServerChannel) -> Self {
        match channel_id {
            ServerChannel::NetworkedEntities => 0,
            ServerChannel::ServerMessages => 1,
            ServerChannel::CriticalAcknowledgments => 2,
            ServerChannel::CriticalRelay => 3,
            ServerChannel::StateUpdates => 4,
        }
    }
}

impl ServerChannel {
    pub fn channels_config() -> Vec<ChannelConfig> {
        vec![
            ChannelConfig {
                channel_id: Self::NetworkedEntities.into(),
                max_memory_usage_bytes: 10 * 1024 * 1024,
                send_type: SendType::Unreliable,
            },
            ChannelConfig {
                channel_id: Self::ServerMessages.into(),
                max_memory_usage_bytes: 10 * 1024 * 1024,
                send_type: SendType::ReliableOrdered {
                    resend_time: Duration::from_millis(200),
                },
            },
            ChannelConfig {
                channel_id: Self::CriticalAcknowledgments.into(),
                max_memory_usage_bytes: 10 * 1024 * 1024,
                send_type: SendType::ReliableOrdered {
                    resend_time: Duration::from_millis(200),
                },
            },
            ChannelConfig {
                channel_id: Self::CriticalRelay.into(),
                max_memory_usage_bytes: 10 * 1024 * 1024,
                send_type: SendType::ReliableOrdered {
                    resend_time: Duration::from_millis(200),
                },
            },
            ChannelConfig {
                channel_id: Self::StateUpdates.into(),
                max_memory_usage_bytes: 10 * 1024 * 1024,
                send_type: SendType::ReliableOrdered {
                    resend_time: Duration::from_millis(200),
                },
            },
        ]
    }
}

pub fn connection_config() -> ConnectionConfig {
    ConnectionConfig {
        available_bytes_per_tick: 1024 * 1024,
        client_channels_config: ClientChannel::channels_config(),
        server_channels_config: ServerChannel::channels_config(),
    }
}

/// set up a simple 3D scene
pub fn setup_level(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // plane
    commands.spawn((
        Mesh3d(meshes.add(Mesh::from(Cuboid::new(40., 1., 40.)))),
        MeshMaterial3d(materials.add(Color::srgb(0.3, 0.5, 0.3))),
        Transform::from_xyz(0.0, -1.0, 0.0),
    ));
    // light
    commands.spawn((
        DirectionalLight {
            shadows_enabled: true,
            ..default()
        },
        Transform {
            translation: Vec3::new(0.0, 2.0, 0.0),
            rotation: Quat::from_rotation_x(-PI / 4.),
            ..default()
        },
    ));
}

#[derive(Debug, Component)]
pub struct Projectile {
    pub duration: Timer,
}

pub fn spawn_fireball(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    translation: Vec3,
    mut direction: Vec3,
) -> Entity {
    if !direction.is_normalized() {
        direction = Vec3::X;
    }
    commands
        .spawn((
            Mesh3d(meshes.add(Sphere { radius: 0.1 })),
            MeshMaterial3d(materials.add(Color::srgb(1.0, 0.0, 0.0))),
            Transform::from_translation(translation),
        ))
        .insert(Velocity(direction * 10.))
        .insert(Projectile {
            duration: Timer::from_seconds(1.5, TimerMode::Once),
        })
        .id()
}
