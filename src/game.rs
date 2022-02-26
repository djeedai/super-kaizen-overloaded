use bevy::{app::CoreStage, asset::AssetStage, input::gamepad::GamepadButtonType, prelude::*};
use bevy_atmosphere::*;
use bevy_tweening::{lens::*, *};
use leafwing_input_manager::prelude::*;
use std::time::Duration;

pub struct GamePlugin;

use crate::AppState;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(bevy_atmosphere::AtmospherePlugin { dynamic: true })
            .add_plugin(InputManagerPlugin::<PlayerAction>::default())
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

#[derive(Component)]
struct Player;

fn game_run(
    mut query: Query<(
        &mut PlayerController,
        &ActionState<PlayerAction>,
        &mut Transform,
    )>,
    time: Res<Time>,
) {
    //println!("game_run");

    let (mut controller, action_state, mut transform) = query.single_mut();
    controller.input_dir = Vec2::ZERO;
    if action_state.pressed(&PlayerAction::MoveUp) {
        controller.input_dir.y += 1.;
        println!("UP");
    }
    if action_state.pressed(&PlayerAction::MoveDown) {
        controller.input_dir.y -= 1.;
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
    if let Some(input_dir) = controller.input_dir.try_normalize() {
        controller.input_dir = input_dir;

        const SPEED: f32 = 1.;
        let dv = input_dir * SPEED * time.delta_seconds();

        transform.translation += Vec3::new(dv.x, dv.y, 0.);
    }
}

fn game_setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    println!("game_setup");

    commands.spawn_bundle(PerspectiveCameraBundle {
        transform: Transform::from_xyz(0.0, 0.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
        ..Default::default()
    });

    // light
    commands.spawn_bundle(PointLightBundle {
        point_light: PointLight {
            intensity: 1500.0,
            shadows_enabled: true,
            ..Default::default()
        },
        transform: Transform::from_xyz(2.0, 4.0, 2.0),
        ..Default::default()
    });

    //let font = asset_server.load("fonts/FiraMono-Regular.ttf");

    let mut input_map = InputMap::default();
    input_map.insert(PlayerAction::MoveUp, KeyCode::Up);
    input_map.insert(PlayerAction::MoveUp, GamepadButtonType::DPadUp);
    input_map.insert(PlayerAction::MoveDown, KeyCode::Down);
    input_map.insert(PlayerAction::MoveDown, GamepadButtonType::DPadDown);
    input_map.insert(PlayerAction::MoveLeft, KeyCode::Left);
    input_map.insert(PlayerAction::MoveLeft, GamepadButtonType::DPadDown);
    input_map.insert(PlayerAction::MoveRight, KeyCode::Right);
    input_map.insert(PlayerAction::MoveRight, GamepadButtonType::DPadDown);

    commands
        .spawn_bundle(PbrBundle {
            mesh: meshes.add(Mesh::from(shape::Cube { size: 0.1 })),
            material: materials.add(Color::rgb(0.8, 0.7, 0.6).into()),
            transform: Transform::from_xyz(0.0, 0.0, 0.0),
            ..Default::default()
        })
        .insert(Name::new("Player"))
        .insert(Player)
        .insert(PlayerController::default())
        .insert_bundle(InputManagerBundle::<PlayerAction> {
            action_state: ActionState::default(),
            input_map,
        });
}
