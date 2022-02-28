use bevy::{
    app::CoreStage,
    asset::AssetStage,
    gltf::{Gltf, GltfMesh},
    input::gamepad::GamepadButtonType,
    math::const_vec2,
    pbr::{NotShadowCaster, NotShadowReceiver},
    prelude::*,
};
use bevy_atmosphere::*;
use bevy_tweening::{lens::*, *};
use heron::prelude::*;
use leafwing_input_manager::prelude::*;
use std::time::Duration;

pub struct GamePlugin;

use crate::{AppState, Layer};

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<PlayerController>()
            .add_event::<DamageEvent>()
            .add_event::<InitLifebarsEvent>()
            .add_plugin(bevy_atmosphere::AtmospherePlugin { dynamic: true })
            .add_plugin(InputManagerPlugin::<PlayerAction>::default())
            .add_system_set_to_stage(
                CoreStage::Update,
                SystemSet::on_enter(AppState::InGame).with_system(game_setup),
            )
            .add_system_set_to_stage(
                CoreStage::Update,
                SystemSet::on_update(AppState::InGame)
                    .with_system(game_run)
                    .with_system(bullet_update)
                    .with_system(detect_collisions)
                    .with_system(update_hud),
            );
    }
}

#[derive(Actionlike, PartialEq, Eq, Clone, Copy, Hash, Debug)]
enum PlayerAction {
    MoveUp,
    MoveDown,
    MoveLeft,
    MoveRight,
    ShootPrimary,
    //
    DebugSpawnBoss,
}

#[derive(Component, Reflect)]
#[reflect(Component)]
struct PlayerController {
    input_dir: Vec2,
    primary_cooloff: f32,
    bullet_texture: Handle<Image>,
    bullet_mesh: Handle<Mesh>,
    bullet_material: Handle<StandardMaterial>,
    primary_fire_delay: f32,
    primary_fire_offset: Vec3,
}

impl Default for PlayerController {
    fn default() -> Self {
        PlayerController {
            input_dir: Vec2::ZERO,
            primary_cooloff: 0.,
            bullet_texture: Handle::default(),
            bullet_mesh: Handle::default(),
            bullet_material: Handle::default(),
            primary_fire_delay: 0.084,
            primary_fire_offset: Vec3::new(0.58, 0., -0.22),
        }
    }
}

#[derive(Component)]
struct Player;

#[derive(Component)]
pub struct Bullet(pub Vec3);

#[derive(Component, Default)]
struct ShipController {
    roll: f32,
}

#[derive(Component)]
struct MainCamera;

/// Event to damage a player or enemy.
#[derive(Debug)]
pub struct DamageEvent(pub f32);

struct Lifebar {
    color: Color,
}

#[derive(Component)]
struct LifebarUnder;

#[derive(Component)]
struct LifebarOver;

#[derive(Debug, Clone)]
struct InitLifebarsEvent {
    /// Entity holding the LifebarHud component of the lifebars to update.
    entity: Entity,
    /// Colors of all lifebars, from undermost (closer to zero life) to topmost (first one to take damages).
    colors: Vec<Color>,
    /// Total life per lifebar.
    life_per_bar: f32,
}

enum LifebarDirection {
    Horizontal,
    Vertical,
}

#[derive(Component)]
struct LifebarHud {
    direction: LifebarDirection,
    visible: bool,
    was_visible: bool,
    visible_pos: Vec2,
    hidden_pos: Vec2,
    /// Descriptions of all lifebars.
    lifebars: Vec<Lifebar>,
    /// Index of current lifebar.
    index: usize,
    /// Total life per lifebar.
    life: f32,
    /// Remaining life in current lifebar.
    remain_life: f32,
    /// Force an update of the lifebar state (including colors).
    force_update: bool,
    /// Material for the next lifebar under the current one, if any.
    under_mat: Handle<StandardMaterial>,
    /// Material for the current lifebar.
    over_mat: Handle<StandardMaterial>,
    underbar: Entity,
    overbar: Entity,
}

impl Default for LifebarHud {
    fn default() -> Self {
        LifebarHud {
            direction: LifebarDirection::Horizontal,
            visible: true,
            was_visible: true,
            visible_pos: Vec2::ZERO,
            hidden_pos: Vec2::ZERO,
            lifebars: vec![],
            index: 0,
            life: 0.,
            remain_life: 0.,
            force_update: false,
            under_mat: Handle::default(),
            over_mat: Handle::default(),
            underbar: Entity::from_raw(0),
            overbar: Entity::from_raw(0),
        }
    }
}

impl LifebarHud {
    pub fn spawn<'w, 's>(
        mut this: LifebarHud,
        name: impl Into<std::borrow::Cow<'static, str>>,
        pos: Vec2,
        size_background: Vec2,
        mat_background: Handle<StandardMaterial>,
        size: Vec2,
        mut commands: Commands<'w, 's>,
        meshes: &mut Assets<Mesh>,
        materials: &mut Assets<StandardMaterial>,
    ) -> Commands<'w, 's> {
        // Bars mesh
        let mesh = meshes.add(Mesh::from(shape::Quad { size, flip: false }));

        // Underbar material
        this.under_mat = materials.add(StandardMaterial {
            base_color: this.lifebars[0].color,
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..Default::default()
        });

        // Overbar material
        this.over_mat = materials.add(StandardMaterial {
            base_color: this.lifebars[this.lifebars.len() - 1].color,
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..Default::default()
        });

        let player = commands
            .spawn_bundle(PbrBundle {
                mesh: meshes.add(Mesh::from(shape::Quad {
                    size: size_background,
                    flip: false,
                })),
                material: mat_background,
                transform: Transform::from_translation(Vec3::new(pos.x, pos.y, 1.)),
                ..Default::default()
            })
            .insert(Name::new(name))
            .insert(Animator::<Transform>::default().with_state(AnimatorState::Paused))
            .with_children(|parent| {
                this.underbar = parent
                    .spawn_bundle(PbrBundle {
                        mesh: mesh.clone(),
                        material: this.under_mat.clone(),
                        transform: Transform::from_xyz(0.0, 0.0, 0.001),
                        ..Default::default()
                    })
                    .insert(LifebarUnder)
                    .id();
                this.overbar = parent
                    .spawn_bundle(PbrBundle {
                        mesh,
                        material: this.over_mat.clone(),
                        transform: Transform::from_xyz(0.0, 0.0, 0.002),
                        ..Default::default()
                    })
                    .insert(LifebarOver)
                    .id();
            })
            .insert(this);

        commands
    }

    pub fn set_lifebars(&mut self, life: f32, colors: impl IntoIterator<Item = Color>) {
        self.lifebars = colors.into_iter().map(|color| Lifebar { color }).collect();
        self.index = self.lifebars.len() - 1;
        self.life = life;
        self.remain_life = life;
        self.force_update = true;
    }

    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }
}

#[derive(Component, Default)]
struct HudManager {}

const LIFEBAR_BOSS_VISIBLE_POS: Vec2 = const_vec2!([0., 1.8]);
const LIFEBAR_BOSS_HIDDEN_POS: Vec2 = const_vec2!([0., 2.2]);

// impl HudManager {
//     fn update(&mut self, under: &mut LifebarUnder, over: &mut LifebarOver) {}
// }

// FIXME
const SHIP1_SCALE: f32 = 0.3;

fn game_run(
    mut commands: Commands,
    mut query: Query<(
        &mut PlayerController,
        &ActionState<PlayerAction>,
        &mut Transform,
    )>,
    mut q_ship: Query<(&mut Transform, &mut ShipController), Without<PlayerController>>,
    time: Res<Time>,
    // DEBUG
    mut init_events: EventWriter<InitLifebarsEvent>,
) {
    //println!("game_run");

    let (mut controller, action_state, mut transform) = query.single_mut();
    let dt = time.delta_seconds();

    controller.input_dir = Vec2::ZERO;
    if action_state.pressed(&PlayerAction::MoveUp) {
        controller.input_dir.y += 1.;
    }
    if action_state.pressed(&PlayerAction::MoveDown) {
        controller.input_dir.y -= 1.;
    }
    if action_state.pressed(&PlayerAction::MoveLeft) {
        controller.input_dir.x -= 1.;
    }
    if action_state.pressed(&PlayerAction::MoveRight) {
        controller.input_dir.x += 1.;
    }
    let dv = if let Some(input_dir) = controller.input_dir.try_normalize() {
        controller.input_dir = input_dir;
        const SPEED: f32 = 1.6;
        let dv = input_dir * SPEED * dt;
        transform.translation += Vec3::new(dv.x, dv.y, 0.);
        dv
    } else {
        Vec2::ZERO
    };

    let (mut ship_transform, mut ship_controller) = q_ship.single_mut();
    let target_roll = if dv.y > 0. {
        -40.
    } else {
        if dv.y < 0. {
            40.
        } else {
            0.
        }
    };
    let roll = ship_controller.roll.lerp(&target_roll, &(dt * 5.));
    ship_controller.roll = roll;
    ship_transform.rotation = Quat::from_rotation_x(roll.to_radians());

    let was_cooling = controller.primary_cooloff > 0.;
    controller.primary_cooloff -= dt;
    if action_state.pressed(&PlayerAction::ShootPrimary) && controller.primary_cooloff <= 0. {
        if !was_cooling {
            controller.primary_cooloff = 0.;
        }
        controller.primary_cooloff += controller.primary_fire_delay;
        let mut transform = transform.clone();
        transform.translation += controller.primary_fire_offset * SHIP1_SCALE;
        commands
            .spawn_bundle(PbrBundle {
                mesh: controller.bullet_mesh.clone(),
                material: controller.bullet_material.clone(),
                transform,
                ..Default::default()
            })
            .insert(Bullet(Vec3::X * 5.))
            // Rendering
            .insert(NotShadowCaster)
            .insert(NotShadowReceiver)
            // Physics
            .insert(RigidBody::Dynamic) // TODO - or Dynamic?
            .insert(CollisionShape::Sphere { radius: 0.1 })
            .insert(Velocity::from_linear(Vec3::X * 5.))
            .insert(RotationConstraints::lock())
            .insert(
                CollisionLayers::none()
                    .with_group(Layer::PlayerBullet)
                    .with_masks(&[Layer::World, Layer::Enemy]),
            );
    }

    // DEBUG

    // if action_state.just_pressed(&PlayerAction::DebugSpawnBoss) {
    //     init_events.send(InitLifebarsEvent {
    //         entity:
    //         colors: [Color::RED, Color::BLUE].into(),
    //         life_per_bar: 10.,
    //     })
    // }
}

fn bullet_update(
    mut commands: Commands,
    mut query: Query<(Entity, &mut Transform, &Bullet), Without<MainCamera>>,
    q_camera: Query<(&PerspectiveProjection, &Transform), With<MainCamera>>,
    time: Res<Time>,
) {
    // Calculate screen bounds based on camera
    let (camera_projection, camera_transform) = q_camera.single();
    // TODO - Dynamic margin in world units, to make it constant-size in screen space
    const MARGIN: f32 = 1.5; // in world units, so actually quite big if camera.x ~= 5 units
    let mut camera_half_height =
        (camera_projection.fov * camera_transform.translation.z * 0.5).abs();
    let camera_half_width = MARGIN + (camera_half_height * camera_projection.aspect_ratio).abs();
    camera_half_height += MARGIN;
    // println!(
    //     "Camera: w/2={} h/2={}",
    //     camera_half_width, camera_half_height
    // );

    let dt = time.delta_seconds();
    for (entity, mut transform, bullet) in query.iter_mut() {
        //transform.translation += bullet.0 * dt;

        // Kill bullets outside of screen bounds
        if transform.translation.x.abs() > camera_half_width
            || transform.translation.y.abs() > camera_half_height
        {
            commands.entity(entity).despawn();
        }
    }
}

fn game_setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    println!("game_setup");

    let ship_mesh: Handle<Scene> = asset_server.load("ship1.glb#Scene0");

    let bullet_texture = asset_server.load("textures/bullet1.png");
    //let bullet_texture = asset_server.load("textures/dev_uv.png");
    let mut player_controller = PlayerController::default();
    player_controller.bullet_texture = bullet_texture.clone();
    player_controller.bullet_mesh = meshes.add(Mesh::from(Quad { size: 0.1 }));
    player_controller.bullet_material = materials.add(StandardMaterial {
        base_color_texture: Some(bullet_texture),
        //emissive: Color::RED,
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        ..Default::default()
    });

    // Main camera
    commands
        .spawn_bundle(PerspectiveCameraBundle {
            transform: Transform::from_xyz(0.0, 0.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
            ..Default::default()
        })
        .insert(MainCamera);

    // Debug camera for Heron/Rapier 2D collision shapes
    // FIXME - doesn't work
    // commands.spawn_bundle(OrthographicCameraBundle::new_2d());
    // commands.spawn_bundle(SpriteBundle{
    //     sprite: Sprite {
    //         color: Color::RED,
    //         custom_size: Some(Vec2::new(500., 10.)),
    //         ..Default::default()
    //     },
    //     //texture: asset_server.load("textures/bullet_dev_32.png"),
    //     transform: Transform::identity(),
    //     ..Default::default()
    // });

    // light
    commands
        .spawn_bundle(PointLightBundle {
            point_light: PointLight {
                intensity: 1500.0,
                shadows_enabled: true,
                ..Default::default()
            },
            transform: Transform::from_xyz(2.0, 4.0, 2.0),
            ..Default::default()
        })
        .insert(Name::new("PointLight"));

    //let font = asset_server.load("fonts/FiraMono-Regular.ttf");

    let mut input_map = InputMap::default();
    input_map.insert(PlayerAction::MoveUp, KeyCode::Up);
    input_map.insert(PlayerAction::MoveUp, KeyCode::W);
    input_map.insert(PlayerAction::MoveUp, GamepadButtonType::DPadUp);
    input_map.insert(PlayerAction::MoveDown, KeyCode::Down);
    input_map.insert(PlayerAction::MoveDown, KeyCode::S);
    input_map.insert(PlayerAction::MoveDown, GamepadButtonType::DPadDown);
    input_map.insert(PlayerAction::MoveLeft, KeyCode::Left);
    input_map.insert(PlayerAction::MoveLeft, KeyCode::A);
    input_map.insert(PlayerAction::MoveLeft, GamepadButtonType::DPadDown);
    input_map.insert(PlayerAction::MoveRight, KeyCode::Right);
    input_map.insert(PlayerAction::MoveRight, KeyCode::D);
    input_map.insert(PlayerAction::MoveRight, GamepadButtonType::DPadDown);
    input_map.insert(PlayerAction::ShootPrimary, KeyCode::Space);
    input_map.insert(PlayerAction::ShootPrimary, KeyCode::LControl);
    //input_map.insert(PlayerAction::ShootPrimary, MouseButton::Left);
    input_map.insert(PlayerAction::DebugSpawnBoss, KeyCode::F1);

    // Player entity
    commands
        // .spawn_bundle(PbrBundle {
        //     mesh: meshes.add(Mesh::from(shape::Cube { size: 0.1 })),
        //     material: materials.add(Color::rgb(0.8, 0.7, 0.6).into()),
        //     transform: Transform::from_xyz(0.0, 0.0, 0.0),
        //     ..Default::default()
        // })
        .spawn()
        .insert(Transform::from_translation(Vec3::X * -1.5)) // start on left side
        .insert(GlobalTransform::identity())
        .insert(Name::new("Player"))
        .insert(Player)
        .insert(player_controller)
        .insert_bundle(InputManagerBundle::<PlayerAction> {
            action_state: ActionState::default(),
            input_map,
        })
        // Physics
        .insert(RigidBody::KinematicPositionBased)
        .insert(CollisionShape::Sphere { radius: 0.1 })
        .insert(
            CollisionLayers::none()
                .with_group(Layer::Player)
                .with_masks(&[Layer::World, Layer::Enemy, Layer::EnemyBullet]),
        )
        // Rendering
        .with_children(|parent| {
            parent
                .spawn_bundle((
                    Transform::from_scale(Vec3::splat(SHIP1_SCALE)),
                    GlobalTransform::identity(),
                ))
                .insert(ShipController::default())
                .with_children(|parent| {
                    parent.spawn_scene(ship_mesh);
                });
        });

    let hud_mat_black = materials.add(StandardMaterial {
        base_color: Color::BLACK,
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        ..Default::default()
    });

    // Player lifebars
    let mut player_lifebars = LifebarHud::default();
    player_lifebars.direction = LifebarDirection::Vertical;
    player_lifebars.visible_pos = Vec2::new(-2.5, 0.);
    player_lifebars.hidden_pos = Vec2::new(-2.5, 0.);
    player_lifebars.set_lifebars(
        400.0,
        [
            Color::RED,
            Color::ORANGE,
            Color::YELLOW,
            Color::GREEN,
            Color::CYAN,
        ],
    );
    player_lifebars.set_visible(true);
    commands = LifebarHud::spawn(
        player_lifebars,
        "PlayerLifebar",
        Vec2::new(-2.5, 0.),
        Vec2::new(0.05, 3.01),
        hud_mat_black.clone(),
        Vec2::new(0.04, 3.),
        commands,
        &mut *meshes,
        &mut *materials,
    );

    // Boss lifebars
    let mut boss_lifebars = LifebarHud::default();
    boss_lifebars.direction = LifebarDirection::Horizontal;
    boss_lifebars.visible_pos = LIFEBAR_BOSS_VISIBLE_POS;
    boss_lifebars.hidden_pos = LIFEBAR_BOSS_HIDDEN_POS;
    boss_lifebars.set_lifebars(40.0, [Color::RED, Color::ORANGE, Color::YELLOW]);
    boss_lifebars.set_visible(true); // TEMP
    commands = LifebarHud::spawn(
        boss_lifebars,
        "BossLifebar",
        Vec2::new(0., 1.5),
        Vec2::new(4.01, 0.05),
        hud_mat_black.clone(),
        Vec2::new(4., 0.04),
        commands,
        &mut *meshes,
        &mut *materials,
    );

    // // HudManager
    // let mut hud = HudManager::default();
    // commands.spawn().insert(Name::new("HudManager")).insert(hud);
}

fn detect_collisions(
    mut commands: Commands,
    mut collision_events: EventReader<CollisionEvent>,
    mut damage_events: EventWriter<DamageEvent>,
) {
    for event in collision_events.iter() {
        match event {
            CollisionEvent::Started(data1, data2) => {
                // println!(
                //     "Entity {:?} and {:?} started to collide",
                //     data1.rigid_body_entity(),
                //     data2.rigid_body_entity()
                // );

                // Damage player
                if data1.collision_layers().contains_group(Layer::Player) {
                    damage_events.send(DamageEvent(1.));
                }
                if data2.collision_layers().contains_group(Layer::Player) {
                    damage_events.send(DamageEvent(1.));
                }

                // Damage enemy
                if data1.collision_layers().contains_group(Layer::Enemy) {
                    damage_events.send(DamageEvent(1.));
                }
                if data2.collision_layers().contains_group(Layer::Enemy) {
                    damage_events.send(DamageEvent(1.));
                }

                // Despawn bullet
                if data1.collision_layers().contains_group(Layer::PlayerBullet) {
                    commands.entity(data1.rigid_body_entity()).despawn();
                }
                if data2.collision_layers().contains_group(Layer::PlayerBullet) {
                    commands.entity(data2.rigid_body_entity()).despawn();
                }
                if data1.collision_layers().contains_group(Layer::EnemyBullet) {
                    commands.entity(data1.rigid_body_entity()).despawn();
                }
                if data2.collision_layers().contains_group(Layer::EnemyBullet) {
                    commands.entity(data2.rigid_body_entity()).despawn();
                }
            }
            CollisionEvent::Stopped(data1, data2) => {
                // println!(
                //     "Entity {:?} and {:?} stopped to collide",
                //     data1.rigid_body_entity(),
                //     data2.rigid_body_entity()
                // )
            }
        }
    }
}

fn update_hud(
    mut query: Query<
        (&mut LifebarHud, &mut Transform, &mut Animator<Transform>),
        Without<LifebarOver>,
    >,
    mut over_query: Query<(&mut LifebarOver, &mut Transform), Without<LifebarHud>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut damage_events: EventReader<DamageEvent>,
    mut init_events: EventReader<InitLifebarsEvent>,
) {
    // Initialize any lifebar HUD if needed
    for ev in init_events.iter() {
        if let Ok((mut hud, _, _)) = query.get_mut(ev.entity) {
            let mut colors = ev.colors.clone();
            hud.set_lifebars(ev.life_per_bar, colors.into_iter());
            hud.set_visible(true);
        }
    }
    let mut damage = 0.;
    for ev in damage_events.iter() {
        // FIXME - Only target entity
        damage += ev.0;
    }

    // Update all HUDs
    for (mut hud, mut transform, mut animator) in query.iter_mut() {
        // Update lifetime bars from damage events
        let mut need_color_update = hud.force_update;
        if hud.force_update {
            hud.index = hud.lifebars.len().max(1) - 1;
            hud.remain_life = hud.life;
        }
        hud.force_update = false;
        if damage > 0. {
            hud.remain_life -= damage;
            println!(
                "damage: {}, lifebar_remain_life: {}",
                damage, hud.remain_life
            );
            let mut over_progress;
            if hud.remain_life <= 0. {
                // Change bars
                if hud.index >= 1 {
                    hud.remain_life = hud.life;
                    hud.index -= 1;
                    over_progress = 1.;
                    need_color_update = true;
                } else {
                    // killed
                    println!("ENTITY KILLED");
                    over_progress = 0.;
                    hud.visible = false;
                }
            } else {
                over_progress = (hud.remain_life / hud.life).clamp(0., 1.);
                println!(
                    "{} / {} = over_progress: {}",
                    hud.remain_life, hud.life, over_progress
                );
            }

            // Scale overbar by progress
            if let Ok((mut over, mut transform)) = over_query.get_mut(hud.overbar) {
                match hud.direction {
                    LifebarDirection::Horizontal => {
                        transform.scale = Vec3::new(over_progress, 1., 1.)
                    }
                    LifebarDirection::Vertical => {
                        transform.scale = Vec3::new(1., over_progress, 1.)
                    }
                }
            }
        }
        if need_color_update {
            let over_color = hud.lifebars[hud.index].color;
            let under_color = if hud.index > 0 {
                hud.lifebars[hud.index - 1].color
            } else {
                Color::NONE
            };
            if let Some(under_mat) = materials.get_mut(hud.under_mat.clone()) {
                under_mat.base_color = under_color;
            }
            if let Some(over_mat) = materials.get_mut(hud.over_mat.clone()) {
                over_mat.base_color = over_color;
            }
        }

        // Update visible/hidden animation
        if hud.visible != hud.was_visible {
            hud.was_visible = hud.visible;
            let pos2d = if hud.visible {
                hud.visible_pos
            } else {
                hud.hidden_pos
            };
            animator.set_tweenable(Tween::new(
                EaseMethod::Linear,
                TweeningType::Once,
                Duration::from_secs_f32(2.5),
                TransformPositionLens {
                    start: transform.translation,
                    end: Vec3::new(pos2d.x, pos2d.y, transform.translation.z),
                },
            ));
            animator.state = AnimatorState::Playing;
        }
    }
}

/// A square on the XY plane centered at the origin.
#[derive(Debug, Copy, Clone)]
pub struct Quad {
    /// The total side length of the square.
    pub size: f32,
}

impl Default for Quad {
    fn default() -> Self {
        Quad { size: 1.0 }
    }
}

impl From<Quad> for Mesh {
    fn from(quad: Quad) -> Self {
        let extent = quad.size / 2.0;

        let vertices = [
            ([-extent, -extent, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0]),
            ([-extent, extent, 0.0], [0.0, 0.0, 1.0], [0.0, 0.0]),
            ([extent, -extent, 0.0], [0.0, 0.0, 1.0], [1.0, 1.0]),
            ([extent, extent, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0]),
        ];

        let indices = bevy::render::mesh::Indices::U16(vec![0, 2, 1, 1, 2, 3]);

        let mut positions = Vec::with_capacity(4);
        let mut normals = Vec::with_capacity(4);
        let mut uvs = Vec::with_capacity(4);
        for (position, normal, uv) in &vertices {
            positions.push(*position);
            normals.push(*normal);
            uvs.push(*uv);
        }

        let mut mesh = Mesh::new(bevy::render::render_resource::PrimitiveTopology::TriangleList);
        mesh.set_indices(Some(indices));
        mesh.set_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        mesh.set_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
        mesh.set_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
        mesh
    }
}
