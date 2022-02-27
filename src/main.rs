#![allow(dead_code, unused_imports, unused_variables, unused_mut)]

use bevy::{
    diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin},
    prelude::*,
};
use bevy_inspector_egui::WorldInspectorPlugin;
use bevy_kira_audio::{Audio, AudioPlugin};
use bevy_tweening::*;
use heron::prelude::*;

mod debug;
mod enemy;
mod game;
mod menu;

use debug::DebugPlugin;
use enemy::EnemyPlugin;
use game::GamePlugin;
use menu::MenuPlugin;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AppState {
    Boot,
    Menu,
    InGame,
}

#[derive(PhysicsLayer)]
pub enum Layer {
    World,
    Player,
    PlayerBullet,
    Enemy,
    EnemyBullet,
}

fn main() {
    let mut app = App::new();
    app.insert_resource(WindowDescriptor {
        title: "unfair".to_string(),
        // width: 1200.,
        // height: 600.,
        vsync: true,
        ..Default::default()
    })
    .insert_resource(ClearColor(Color::rgba(0., 0., 0., 0.)))
    .insert_resource(bevy_atmosphere::AtmosphereMat::default())
    .add_plugins(DefaultPlugins)
    .add_plugin(FrameTimeDiagnosticsPlugin::default())
    //.add_plugin(LogDiagnosticsPlugin::default())
    .add_plugin(DebugPlugin)
    .add_plugin(WorldInspectorPlugin::new())
    .add_plugin(TweeningPlugin)
    .add_plugin(AudioPlugin)
    .add_plugin(PhysicsPlugin::default());

    let initial_state = AppState::Boot;
    app.add_state(initial_state)
        .add_state_to_stage(CoreStage::First, initial_state) // BUG #1671
        .add_state_to_stage(CoreStage::PreUpdate, initial_state) // BUG #1671
        .add_state_to_stage(CoreStage::PostUpdate, initial_state) // BUG #1671
        .add_state_to_stage(CoreStage::Last, initial_state); // BUG #1671

    app.add_plugin(MenuPlugin)
        .add_plugin(GamePlugin)
        .add_plugin(EnemyPlugin);

    // Only enable MSAA on non-web platforms
    #[cfg(not(target_arch = "wasm32"))]
    app.insert_resource(Msaa { samples: 4 });

    app.add_system_set(SystemSet::on_update(AppState::Boot).with_system(boot));

    app.run();
}

fn boot(mut state: ResMut<State<AppState>>) {
    // workaround for on_enter() not working on initial state; use a dummy initial state instead
    state.set(AppState::Menu).unwrap();
}
