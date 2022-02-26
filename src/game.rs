use bevy::{app::CoreStage, asset::AssetStage, input::gamepad::GamepadButtonType, prelude::*};
use bevy_tweening::{lens::*, *};
use leafwing_input_manager::prelude::*;
use std::time::Duration;

pub struct GamePlugin;

use crate::AppState;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(InputManagerPlugin::<PlayerAction>::default())
            .add_system_set_to_stage(
                CoreStage::Update,
                SystemSet::on_enter(AppState::InGame).with_system(game_setup),
            )
            .add_system_set_to_stage(
                CoreStage::Update,
                SystemSet::on_update(AppState::InGame).with_system(game_run),
            );
    }
}

#[derive(Actionlike, PartialEq, Eq, Clone, Copy, Hash, Debug)]
enum PlayerAction {
    MoveUp,
    MoveDown,
    MoveLeft,
    MoveRight,
}

#[derive(Component, Default)]
struct PlayerController {
    input_dir: Vec2,
}

fn game_run(mut query: Query<(&mut PlayerController, &ActionState<PlayerAction>)>) {
    //println!("game_run");

    let (mut controller, action_state) = query.single_mut();
    if action_state.pressed(&PlayerAction::MoveUp) {
        controller.input_dir.y -= 1.;
        println!("UP");
    }
    if action_state.pressed(&PlayerAction::MoveDown) {
        controller.input_dir.y += 1.;
        println!("DOWN");
    }
    if action_state.pressed(&PlayerAction::MoveLeft) {
        controller.input_dir.x -= 1.;
        println!("LEFT");
    }
    if action_state.pressed(&PlayerAction::MoveRight) {
        controller.input_dir.x += 1.;
        println!("RIGHT");
    }
    controller.input_dir = controller.input_dir.normalize();


}

fn game_setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    println!("game_setup");
    commands.spawn_bundle(OrthographicCameraBundle::new_2d());

    //let font = asset_server.load("fonts/FiraMono-Regular.ttf");

    let mut input_map = InputMap::default();
    input_map.insert(PlayerAction::MoveUp, KeyCode::Up);
    input_map.insert(PlayerAction::MoveUp, GamepadButtonType::DPadUp);
    input_map.insert(PlayerAction::MoveDown, KeyCode::Down);
    input_map.insert(PlayerAction::MoveDown, GamepadButtonType::DPadDown);
}
