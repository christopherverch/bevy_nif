use bevy_state::state::States;
#[derive(Clone, Copy, Debug, Eq, PartialEq, States, Default, Hash)]
pub enum GameState {
    #[default]
    MainMenu,
    Connecting,
    Offline,
    Connected,
}
#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum CharCreationState {
    #[default]
    None,
    Naming,
    ChoosingRace,
    ChoosingClass,
    BirthSign,
}
