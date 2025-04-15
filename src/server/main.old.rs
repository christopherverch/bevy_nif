use bevy::{
    MinimalPlugins,
    app::{App, ScheduleRunnerPlugin, Update},
    log::{LogPlugin, info},
    prelude::*,
};
use std::io::Cursor; // Import Cursor for easy byte slice reading

use cdda_bevy::PacketType;
use cdda_bevy::PlayerInfo;
use cdda_bevy::networking::{
    NetworkEvent, NetworkResource, ServerPlugin, Transport, UdpSocketResource,
};
use std::net::SocketAddr;
use std::{net::UdpSocket, time::Duration};
use uuid::Uuid;
struct PlayerWithAddr {
    addr: SocketAddr,
    player_info: PlayerInfo,
}
#[derive(Resource, Default)]
struct PlayerList(Vec<PlayerWithAddr>);

const LISTEN_ADDRESS: &str = "127.0.0.1:4567";
fn main() {
    let socket = UdpSocket::bind(LISTEN_ADDRESS).expect("could not bind socket");
    socket
        .set_nonblocking(true)
        .expect("could not set socket to be nonblocking");
    socket
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("could not set read timeout");

    info!("Server now listening on {}", LISTEN_ADDRESS);

    App::new()
        .insert_resource(PlayerList::default())
        .insert_resource(UdpSocketResource::new(socket))
        .add_plugins((
            // run the server at a reduced tick rate (35 ticks per second)
            MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f32(
                1. / 35.,
            ))),
            LogPlugin::default(),
            ServerPlugin,
        ))
        .add_systems(Update, connection_handler)
        .run();
}

fn connection_handler(
    net: Res<NetworkResource>,
    mut events: EventReader<NetworkEvent>,
    mut transport: ResMut<Transport>,
    mut player_list: ResMut<PlayerList>,
    time: Res<Time>,
) {
    for event in events.read() {
        match event {
            NetworkEvent::Connected(handle) => {
                let uuid = Uuid::new_v4(); // Generate a unique ID
                for (addr, _) in net.connections.iter() {
                    let mut packet_type = PacketType::SpawnPlayer as u8;
                    if addr == handle {
                        packet_type = PacketType::FirstConnected as u8;
                    }
                    println!(
                        "Adding player with UUID and packet_type {} {}: {:?}",
                        addr, uuid, packet_type
                    );
                    let location = Vec3::new(0.0, 0.0, 0.0);
                    let player_info = PlayerInfo {
                        uuid,
                        name: "test".to_string(),
                        last_position: location.clone(),
                        current_position: location,
                        last_update_time: 0.01,
                        current_update_time: 0.0,
                    };
                    let mut serialized_player = Vec::new();
                    match player_info.encode(&mut serialized_player) {
                        Ok(()) => {
                            // Encode was successful, send the serialized data
                            let mut full_packet = Vec::with_capacity(1 + serialized_player.len()); // Pre-allocate
                            full_packet.push(packet_type); // Add the packet type as the first byte
                            full_packet.extend_from_slice(&serialized_player); // Append the serialized data
                            transport.send(*addr, &full_packet);
                            let player_info_addr = PlayerWithAddr {
                                addr: addr.clone(),
                                player_info,
                            };
                            if packet_type != PacketType::SpawnPlayer as u8 {
                                player_list.0.push(player_info_addr);
                            }
                        }
                        Err(e) => {
                            // Handle the error (e.g., log it, send an error message)
                            eprintln!("Error encoding player: {}", e);
                            // Optionally, send an error message to the client
                        }
                    }
                    /*  transport.send(*addr, &[069]);
                    transport.send(*addr, format!("{} has entered the chat", handle).as_bytes());*/
                }
            }
            NetworkEvent::Disconnected(handle) => {
                for (addr, _) in net.connections.iter() {
                    transport.send(*addr, format!("{} has left the chat", handle).as_bytes());
                }
            }
            NetworkEvent::Message(handle, msg) => {
                let packet_type = msg[0];

                match packet_type {
                    pt if pt == PacketType::UpdatePosition as u8 => {
                        for PlayerWithAddr { addr, .. } in player_list.0.iter() {
                            if addr == handle {
                                return;
                            }
                            /*transport.send(
                                *addr,
                                format!("{}: {}", handle, String::from_utf8_lossy(msg))
                                    .as_bytes(),
                            );*/
                            transport.send(*addr, msg);
                            println!("time: {}", time.elapsed_secs());
                        }
                    }
                    _ => {}
                }
            }
            NetworkEvent::SendError(err, msg) => {
                error!(
                    "NetworkEvent::SendError (payload [{:?}]): {:?}",
                    msg.payload, err
                );
            }
            NetworkEvent::RecvError(err) => {
                error!("NetworkEvent::RecvError: {:?}", err);
            }
        }
    }
}
