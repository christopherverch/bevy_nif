use bevy::prelude::*;
use std::io::{Read, Write};
use uuid::Uuid;

pub mod networking;
#[repr(u8)] // Specify the underlying type here
#[derive(Debug, PartialEq)]
pub enum PacketType {
    FirstConnected = 10,
    SpawnPlayer = 15,
    UpdatePosition = 16,    // Add this
    PlayerDisconnected = 3, // Add this
    Other = 4,
}
pub fn packet_type_from_u8(value: u8) -> PacketType {
    match value {
        pt if pt == PacketType::FirstConnected as u8 => PacketType::FirstConnected,
        pt if pt == PacketType::SpawnPlayer as u8 => PacketType::SpawnPlayer,
        pt if pt == PacketType::UpdatePosition as u8 => PacketType::UpdatePosition,
        _ => PacketType::Other,
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EncodedUuid(pub Uuid);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EncodedVec3(pub Vec3);
#[derive(Component, Debug, Clone, PartialEq)]
pub struct PlayerInfo {
    pub id: u64,
    pub name: String,
    pub last_position: Vec3,
    pub current_position: Vec3,
    pub last_update_time: f32,
    pub current_update_time: f32,
}
