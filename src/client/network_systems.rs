use bevy::prelude::*;
use bevy_renet::renet::RenetClient;
use cdda_bevy::{
    ClientMessage, NetworkedActorState,
    client::{actor_defs::ActorState, defs::LocalPlayer},
    networking::ClientChannel,
};

/// This system runs on the client and sends its state to the server whenever it changes.
pub fn send_local_player_state_system(
    mut client: ResMut<RenetClient>,
    // Query for the entity that has BOTH LocalPlayer and a changed ActorState
    query: Query<&ActorState, (With<LocalPlayer>, Changed<ActorState>)>,
) {
    // Check if the query found our local player and if their state has actually changed.
    // The `Changed` filter is a Bevy optimization to avoid running this logic every frame.
    if let Ok(actor_state) = query.single() {
        // Convert our full ActorState into the lean, network-ready version.
        let networked_state = NetworkedActorState::from(actor_state);

        // Wrap our state update in the ClientMessage enum.
        let message = ClientMessage::StateUpdate(networked_state);

        // Serialize the message into binary data.
        // bincode is a good choice for performance.
        let serialized_message = bincode::serialize(&message).unwrap();

        // Send the message to the server on a "reliable" channel.
        // Reliable means the network library guarantees it will arrive and in the correct order.
        client.send_message(ClientChannel::State, serialized_message);
    }
}
