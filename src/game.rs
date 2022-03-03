use bevy::{
    app::CoreStage,
    asset::{AssetStage, LoadState},
    gltf::{Gltf, GltfMesh},
    input::gamepad::GamepadButtonType,
    math::const_vec2,
    pbr::{NotShadowCaster, NotShadowReceiver},
    prelude::*,
    window::WindowId,
};
use bevy_atmosphere::*;
use bevy_kira_audio::{
    Audio as KiraAudio, AudioChannel as KiraAudioChannel, AudioPlugin as KiraAudioPlugin,
    AudioSource as KiraAudioSource, PlaybackState,
};
use bevy_tweening::{lens::*, *};
use heron::prelude::*;
use leafwing_input_manager::prelude::*;
use rand::prelude::*;
use std::{f32::consts::PI, time::Duration};

pub struct GamePlugin;

use crate::{menu::AudioManager, AppState, Layer};

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<PlayerController>()
            .add_event::<DamageEvent>()
            .add_event::<InitLifebarsEvent>()
            .add_event::<ShowLifebarsEvent>()
            .add_event::<UpdateLifebarsEvent>()
            .init_resource::<AudioRes>()
            .add_plugin(bevy_atmosphere::AtmospherePlugin { dynamic: true })
            .add_plugin(InputManagerPlugin::<PlayerAction>::default())
            .add_system_set_to_stage(
                CoreStage::PreUpdate,
                SystemSet::on_update(AppState::InGame).with_system(update_screen_bounds),
            )
            .add_system_set_to_stage(
                CoreStage::Update,
                SystemSet::on_enter(AppState::InGame).with_system(game_setup),
            )
            .add_system_set_to_stage(
                CoreStage::Update,
                SystemSet::on_update(AppState::InGame)
                    .with_system(update_player)
                    .with_system(despawn_bullets_outside_screen)
                    .with_system(detect_collisions)
                    .with_system(update_sky_from_sun)
                    .with_system(update_hud),
            );
    }
}

#[derive(Component)]
struct Sun;

fn update_sky_from_sun(
    mut sky_mat: ResMut<AtmosphereMat>,
    mut query: Query<(&mut Transform, &mut DirectionalLight), With<Sun>>,
    time: Res<Time>,
) {
    if let Some((mut light_trans, mut directional)) = query.single_mut().into() {
        // TODO - Control that better to not fall into night
        light_trans.rotation =
            Quat::from_rotation_x(-PI + PI * time.seconds_since_startup() as f32 / 60.);

        // Update sky from sun direction
        let pos = light_trans.rotation.mul_vec3(Vec3::Z);
        sky_mat.sun_position = pos;
        //directional.illuminance = t.sin().max(0.0).powf(2.0) * 100000.0;
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
pub struct PlayerController {
    input_dir: Vec2,
    primary_cooloff: f32,
    bullet_texture: Handle<Image>,
    bullet_mesh: Handle<Mesh>,
    bullet_material: Handle<StandardMaterial>,
    primary_fire_delay: f32,
    primary_fire_offset: Vec3,
    life: f32,
    remain_life: f32,
    lifebar_entity: Entity,
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
            life: 100.,
            remain_life: 100.,
            lifebar_entity: Entity::from_raw(0),
        }
    }
}

impl PlayerController {
    fn spawn_bullet(&self, commands: &mut Commands, transform: &Transform) {
        commands
            .spawn_bundle(PbrBundle {
                mesh: self.bullet_mesh.clone(),
                material: self.bullet_material.clone(),
                transform: *transform,
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
}

#[derive(Component)]
struct Player;

#[derive(Component)]
pub struct Bullet(pub Vec3);

#[derive(Component, Default)]
struct ShipController {
    roll: f32,
}

#[derive(Component, Default)]
struct MainCamera {
    screen_bounds: Rect<f32>,
}

impl MainCamera {
    pub fn update_screen_bounds(
        &mut self,
        projection: &PerspectiveProjection,
        transform: &Transform,
    ) {
        let camera_half_height = (projection.fov * transform.translation.z * 0.5).abs();
        let camera_half_width = (camera_half_height * projection.aspect_ratio).abs();
        self.screen_bounds.left = -camera_half_width;
        self.screen_bounds.right = camera_half_width;
        self.screen_bounds.bottom = -camera_half_height;
        self.screen_bounds.top = camera_half_height;
        println!(
            "Screen bounds changed: cw/2={} ch/2={} bounds={:?}",
            camera_half_width, camera_half_height, self.screen_bounds
        );
    }
}

/// Event to damage a player or enemy.
#[derive(Debug)]
pub struct DamageEvent {
    pub entity: Entity,
    pub damage: f32,
}

pub struct Lifebar {
    pub color: Color,
}

#[derive(Component)]
struct LifebarUnder;

#[derive(Component)]
struct LifebarOver;

#[derive(Debug, Clone)]
pub struct InitLifebarsEvent {
    /// Entity holding the LifebarHud component of the lifebars to update.
    pub entity: Entity,
    /// Colors of all lifebars, from undermost (closer to zero life) to topmost (first one to take damages).
    pub colors: Vec<Color>,
    /// Total life per lifebar.
    pub life_per_bar: f32,
}

#[derive(Debug, Clone)]
pub struct ShowLifebarsEvent {
    /// Entity holding the LifebarHud component of the lifebars to update.
    pub entity: Entity,
}

#[derive(Debug, Clone)]
pub struct UpdateLifebarsEvent {
    /// Entity holding the LifebarHud component to update.
    pub entity: Entity,
    /// New value for the remaining life to apply to the lifebar.
    pub remain_life: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifebarOrientation {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifebarFillSeqPhase {
    /// Off-screen, waiting.
    Idle,
    /// Slide inside screen from hidden to visible position.
    SlideIn,
    /// Fill up bars until full. Contains index of currently filling bar.
    FillUp(usize),
    /// Ready for use.
    Ready,
    /// Slide outside screen from visible to hidden position.
    SlideOut,
}

#[derive(Component)]
pub struct LifebarHud {
    ///
    pub orientation: LifebarOrientation,
    pub visible_pos: Vec2,
    pub hidden_pos: Vec2,
    /// Descriptions of all lifebars.
    pub lifebars: Vec<Lifebar>,
    /// Index of current lifebar.
    pub index: usize,
    /// Total life per lifebar.
    pub life: f32,
    /// Remaining life in current lifebar.
    pub remain_life: f32,
    /// Force an update of the lifebar state (including colors).
    pub force_update: bool,
    /// Material for the next lifebar under the current one, if any.
    pub under_mat: Handle<StandardMaterial>,
    /// Material for the current lifebar.
    pub over_mat: Handle<StandardMaterial>,
    pub underbar_entity: Entity,
    pub overbar_entity: Entity,
    pub fill_seq: LifebarFillSeqPhase,
}

impl Default for LifebarHud {
    fn default() -> Self {
        LifebarHud {
            orientation: LifebarOrientation::Horizontal,
            visible_pos: Vec2::ZERO,
            hidden_pos: Vec2::ZERO,
            lifebars: vec![],
            index: 0,
            life: 0.,
            remain_life: 0.,
            force_update: false,
            under_mat: Handle::default(),
            over_mat: Handle::default(),
            underbar_entity: Entity::from_raw(0),
            overbar_entity: Entity::from_raw(0),
            fill_seq: LifebarFillSeqPhase::Idle,
        }
    }
}

impl LifebarHud {
    pub fn spawn<'w, 's>(
        mut this: LifebarHud,
        name: impl Into<std::borrow::Cow<'static, str>>,
        size_background: Vec2,
        mat_background: Handle<StandardMaterial>,
        size: Vec2,
        commands: &mut Commands<'w, 's>,
        meshes: &mut Assets<Mesh>,
        materials: &mut Assets<StandardMaterial>,
    ) -> Entity {
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

        commands
            .spawn_bundle(PbrBundle {
                mesh: meshes.add(Mesh::from(shape::Quad {
                    size: size_background,
                    flip: false,
                })),
                material: mat_background,
                transform: Transform::from_translation(Vec3::new(
                    this.hidden_pos.x,
                    this.hidden_pos.y,
                    1.,
                )),
                ..Default::default()
            })
            .insert(Name::new(name))
            .insert(Animator::<Transform>::default().with_state(AnimatorState::Paused))
            .with_children(|parent| {
                this.underbar_entity = parent
                    .spawn_bundle(PbrBundle {
                        mesh: mesh.clone(),
                        material: this.under_mat.clone(),
                        transform: Transform::from_xyz(0.0, 0.0, 0.001),
                        ..Default::default()
                    })
                    .insert(LifebarUnder)
                    .id();
                this.overbar_entity = parent
                    .spawn_bundle(PbrBundle {
                        mesh,
                        material: this.over_mat.clone(),
                        transform: Transform::from_xyz(0.0, 0.0, 0.002),
                        ..Default::default()
                    })
                    .insert(LifebarOver)
                    .insert(Animator::<Transform>::default().with_state(AnimatorState::Paused))
                    .id();
            })
            .insert(this)
            .id()
    }

    pub fn set_lifebars(&mut self, life: f32, colors: impl IntoIterator<Item = Color>) {
        self.lifebars = colors.into_iter().map(|color| Lifebar { color }).collect();
        self.index = self.lifebars.len() - 1;
        self.life = life;
        self.remain_life = life;
        self.force_update = true;
    }

    pub fn set_remain_life(&mut self, remain_life: f32) {
        self.remain_life = remain_life;
        self.force_update = true;
    }
}

#[derive(Component, Default)]
struct HudManager {}

// FIXME
const SHIP1_SCALE: f32 = 0.3;

fn update_player(
    mut commands: Commands,
    mut query: Query<(
        Entity,
        &mut PlayerController,
        &ActionState<PlayerAction>,
        &mut Transform,
    )>,
    mut q_ship: Query<(&mut Transform, &mut ShipController), Without<PlayerController>>,
    time: Res<Time>,
    mut damage_events: EventReader<DamageEvent>,
    mut lifebar_events: EventWriter<UpdateLifebarsEvent>,
    // DEBUG
    //mut init_events: EventWriter<InitLifebarsEvent>,
    //mut show_events: EventWriter<ShowLifebarsEvent>,
) {
    //println!("update_player");

    if query.is_empty() {
        // Player died
        return;
    }

    let (player_entity, mut controller, action_state, mut transform) = query.single_mut();
    let dt = time.delta_seconds();

    // Apply damage to player
    let player_damage: f32 = damage_events
        .iter()
        .filter_map(|ev| {
            if ev.entity == player_entity {
                Some(ev.damage)
            } else {
                None
            }
        })
        .sum();
    if player_damage > 0. {
        controller.remain_life -= player_damage;
        lifebar_events.send(UpdateLifebarsEvent {
            entity: controller.lifebar_entity,
            remain_life: controller.remain_life,
        });
    }
    if controller.remain_life <= 0. {
        commands.entity(player_entity).despawn_recursive();
        // GAME ENDS
        println!("PLAYER KILLED");
        return;
    }

    // Move player
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
        transform.translation += controller.primary_fire_offset * SHIP1_SCALE / 2.; // FIXME - fire origin
        controller.spawn_bullet(&mut commands, &transform);
        transform.translation.y += 0.1;
        controller.spawn_bullet(&mut commands, &transform);
        transform.translation.y -= 0.2;
        controller.spawn_bullet(&mut commands, &transform);
    }

    // DEBUG

    // if action_state.just_pressed(&PlayerAction::DebugSpawnBoss) {
    //     init_events.send(InitLifebarsEvent {
    //         entity: player_lifebars_entity,
    //         colors: [Color::RED, Color::BLUE].into(),
    //         life_per_bar: 10.,
    //     });
    //     show_events.send(ShowLifebarsEvent {
    //         entity: player_lifebars_entity,
    //     });
    // }
}

/// Calculate screen bounds based on camera projection.
fn update_screen_bounds(
    mut query: Query<(
        &mut MainCamera,
        ChangeTrackers<PerspectiveProjection>,
        &PerspectiveProjection,
        ChangeTrackers<Transform>,
        &Transform,
    )>,
) {
    let (
        mut main_camera,
        camera_projection_tracker,
        camera_projection,
        camera_transform_tracker,
        camera_transform,
    ) = query.single_mut();
    if camera_projection_tracker.is_changed() || camera_transform_tracker.is_changed() {
        main_camera.update_screen_bounds(camera_projection, camera_transform);
    }
}

fn despawn_bullets_outside_screen(
    mut commands: Commands,
    mut query: Query<(Entity, &mut Transform, &Bullet), Without<MainCamera>>,
    q_camera: Query<(&PerspectiveProjection, &Transform), With<MainCamera>>,
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

    for (entity, mut transform, bullet) in query.iter_mut() {
        if transform.translation.x.abs() > camera_half_width
            || transform.translation.y.abs() > camera_half_height
        {
            commands.entity(entity).despawn();
        }
    }
}

#[derive(Default)]
struct AudioRes {
    sfx_channel: KiraAudioChannel,
    sound_hit: Handle<KiraAudioSource>,
}

fn game_setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    audio: Res<KiraAudio>,
    windows: Res<Windows>,
    mut audio_res: ResMut<AudioRes>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut init_events: EventWriter<InitLifebarsEvent>,
    mut show_events: EventWriter<ShowLifebarsEvent>,
) {
    println!("game_setup");

    let ship_mesh: Handle<Scene> = asset_server.load("ship1.glb#Scene0");

    audio_res.sfx_channel = KiraAudioChannel::new("sfx".to_string());
    audio.set_volume_in_channel(0.5, &audio_res.sfx_channel);
    audio_res.sound_hit = asset_server.load("sounds/hit.ogg");

    // Main camera
    let camera_depth = 5.0;
    let mut camera_bundle = PerspectiveCameraBundle {
        transform: Transform::from_xyz(0.0, 0.0, camera_depth).looking_at(Vec3::ZERO, Vec3::Y),
        ..Default::default()
    };
    // FIXME - aspect ratio will be fixed-up later based on window size, but we need it now
    let window = windows.get(WindowId::primary()).unwrap();
    let aspect_ratio = window.width() / window.height();
    camera_bundle.perspective_projection.aspect_ratio = aspect_ratio;
    let mut main_camera = MainCamera::default();
    main_camera.update_screen_bounds(
        &camera_bundle.perspective_projection,
        &camera_bundle.transform,
    );
    let screen_bounds = main_camera.screen_bounds;
    println!("Initial screen bounds: {:?}", screen_bounds);
    commands.spawn_bundle(camera_bundle).insert(main_camera);

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
        .spawn_bundle(DirectionalLightBundle {
            directional_light: DirectionalLight {
                color: Color::WHITE,
                illuminance: 8000.0,
                shadows_enabled: true,
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(Name::new("Sun"))
        .insert(Sun);

    //let font = asset_server.load("fonts/FiraMono-Regular.ttf");

    // FIXME - Copied in enemy.rs :(
    let hud_mat_black = materials.add(StandardMaterial {
        base_color: Color::BLACK,
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        ..Default::default()
    });

    // let z_hud = 1.;
    // let perspective_correction = camera_depth / (camera_depth - z_hud);
    // let screen_to_world = (screen_bounds.bottom - screen_bounds.top).abs()
    //     / window.physical_height().max(1) as f32
    //     * perspective_correction;
    // let lifebar_margin = 64. * screen_to_world;
    // println!(
    //     "w={} h={} screen_to_world={} lifebar_margin={}",
    //     window.physical_width(),
    //     window.physical_height(),
    //     screen_to_world,
    //     lifebar_margin
    // );
    let lifebar_margin_v = 0.4;
    let lifebar_margin_h = lifebar_margin_v * aspect_ratio;

    // Player lifebars
    let mut player_lifebars = LifebarHud::default();
    player_lifebars.orientation = LifebarOrientation::Vertical;
    player_lifebars.visible_pos = Vec2::new(screen_bounds.left + lifebar_margin_h, 0.);
    player_lifebars.hidden_pos = Vec2::new(screen_bounds.left - lifebar_margin_h, 0.);
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
    let player_lifebars_entity = LifebarHud::spawn(
        player_lifebars,
        "PlayerLifebar",
        Vec2::new(0.05, 3.01),
        hud_mat_black.clone(),
        Vec2::new(0.04, 3.),
        &mut commands,
        &mut *meshes,
        &mut *materials,
    );

    // Show player lifebars
    let player_lifebar_colors = [
        Color::RED,
        Color::ORANGE,
        Color::YELLOW,
        Color::GREEN,
        Color::CYAN,
    ];
    let player_lifebars_count = player_lifebar_colors.len();
    let player_life_per_lifebar = 100.;
    init_events.send(InitLifebarsEvent {
        entity: player_lifebars_entity,
        colors: player_lifebar_colors.into(),
        life_per_bar: player_life_per_lifebar,
    });
    show_events.send(ShowLifebarsEvent {
        entity: player_lifebars_entity,
    });

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
    player_controller.life = player_lifebars_count as f32 * player_life_per_lifebar;
    player_controller.remain_life = player_controller.life;
    player_controller.lifebar_entity = player_lifebars_entity;

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

    // // HudManager
    // let mut hud = HudManager::default();
    // commands.spawn().insert(Name::new("HudManager")).insert(hud);

    let clouds_texture = asset_server.load("textures/clouds2.png");
    let mut rng = rand::thread_rng();
    for _ in 0..10 {
        let h = rng.gen::<f32>() * 3. - 1.5;
        let delay = rng.gen::<f32>() * 2.457;
        let duration = 0.7 + rng.gen::<f32>() * 1.3;
        let x = 0.8 + rng.gen::<f32>() * 0.4;
        let y = 0.8 + rng.gen::<f32>() * 0.4;
        let s = 0.3 + rng.gen::<f32>() * 1.4;
        let clouds_tween = Delay::new(Duration::from_secs_f32(delay)).then(Tween::new(
            EaseMethod::Linear,
            TweeningType::Loop,
            Duration::from_secs_f32(duration),
            TransformPositionLens {
                end: Vec3::new(-5., h, 0.),
                start: Vec3::new(5., h, 0.),
            },
        ));
        commands
            .spawn_bundle(PbrBundle {
                mesh: meshes.add(Mesh::from(shape::Quad {
                    size: Vec2::new(2., 0.3),
                    flip: false,
                })),
                material: materials.add(StandardMaterial {
                    base_color_texture: Some(clouds_texture.clone()),
                    unlit: true,
                    alpha_mode: AlphaMode::Blend,
                    ..Default::default()
                }),
                transform: Transform::from_translation(Vec3::X * 10.) // out of screen
                    .with_scale(Vec3::new(x * s, y * s, 1.)),
                ..Default::default()
            })
            .insert(Name::new("clouds"))
            .insert(Animator::new(clouds_tween));
    }
}

fn detect_collisions(
    mut commands: Commands,
    mut collision_events: EventReader<CollisionEvent>,
    mut damage_events: EventWriter<DamageEvent>,
    query_player: Query<&mut PlayerController>,
    audio: Res<KiraAudio>,
    audio_res: Res<AudioRes>,
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
                    damage_events.send(DamageEvent {
                        entity: data1.rigid_body_entity(),
                        damage: 1.,
                    });
                }
                if data2.collision_layers().contains_group(Layer::Player) {
                    damage_events.send(DamageEvent {
                        entity: data2.rigid_body_entity(),
                        damage: 1.,
                    });
                }

                // Damage enemy
                if data1.collision_layers().contains_group(Layer::Enemy) {
                    damage_events.send(DamageEvent {
                        entity: data1.rigid_body_entity(),
                        damage: 1.,
                    });
                    audio.play_in_channel(audio_res.sound_hit.clone(), &audio_res.sfx_channel);
                }
                if data2.collision_layers().contains_group(Layer::Enemy) {
                    damage_events.send(DamageEvent {
                        entity: data2.rigid_body_entity(),
                        damage: 1.,
                    });
                    audio.play_in_channel(audio_res.sound_hit.clone(), &audio_res.sfx_channel);
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
    mut hud_query: Query<
        (
            Entity,
            &mut LifebarHud,
            &mut Transform,
            &mut Animator<Transform>,
        ),
        Without<LifebarOver>,
    >,
    mut over_query: Query<
        (&mut LifebarOver, &mut Transform, &mut Animator<Transform>),
        Without<LifebarHud>,
    >,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut init_events: EventReader<InitLifebarsEvent>,
    mut show_events: EventReader<ShowLifebarsEvent>,
    mut update_events: EventReader<UpdateLifebarsEvent>,
    //
    //asset_server: Res<AssetServer>,
    //audio: Res<KiraAudio>,
) {
    // Initialize any lifebar HUD if needed
    for ev in init_events.iter() {
        if let Ok((_, mut hud, _, _)) = hud_query.get_mut(ev.entity) {
            let mut colors = ev.colors.clone();
            println!(
                "INIT LIFEBAR: entity={:?} life_per_bar={} colors_count={}",
                ev.entity,
                ev.life_per_bar,
                colors.len()
            );
            hud.set_lifebars(ev.life_per_bar, colors.into_iter());
        }
    }

    // Show any lifebar HUD if needed
    for ev in show_events.iter() {
        if let Ok((_, mut hud, mut transform, mut animator)) = hud_query.get_mut(ev.entity) {
            println!(
                "SHOW LIFEBAR: entity={:?} prev_state={:?}",
                ev.entity, hud.fill_seq
            );
            if hud.fill_seq == LifebarFillSeqPhase::Idle {
                animator.set_tweenable(Tween::new(
                    EaseMethod::Linear,
                    TweeningType::Once,
                    Duration::from_secs_f32(2.5),
                    TransformPositionLens {
                        start: Vec3::new(
                            hud.hidden_pos.x,
                            hud.hidden_pos.y,
                            transform.translation.z,
                        ),
                        end: Vec3::new(
                            hud.visible_pos.x,
                            hud.visible_pos.y,
                            transform.translation.z,
                        ),
                    },
                ));
                animator.rewind();
                animator.state = AnimatorState::Playing;
                hud.fill_seq = LifebarFillSeqPhase::SlideIn;
                hud.index = 0; // start from bottom-most bar
            }
        }
    }

    // Update all HUDs
    for (hud_entity, mut hud, mut transform, mut animator) in hud_query.iter_mut() {
        let mut need_color_update = hud.force_update;
        hud.force_update = false;

        if let Ok((mut overbar, mut over_transform, mut over_animator)) =
            over_query.get_mut(hud.overbar_entity)
        {
            // Transition fill sequence if needed
            if animator.progress() >= 1. || over_animator.progress() >= 1. {
                // TODO - auto-stop on completed
                animator.stop();
                over_animator.stop();

                match hud.fill_seq {
                    LifebarFillSeqPhase::SlideIn => {
                        hud.fill_seq = LifebarFillSeqPhase::FillUp(0);
                        need_color_update = true;
                        let start = match hud.orientation {
                            LifebarOrientation::Horizontal => Vec3::new(0., 1., 1.),
                            LifebarOrientation::Vertical => Vec3::new(1., 0., 1.),
                        };
                        over_animator.set_tweenable(Tween::new(
                            EaseMethod::Linear,
                            TweeningType::Once,
                            Duration::from_secs_f32(1.5),
                            TransformScaleLens {
                                start,
                                end: Vec3::ONE,
                            },
                        ));
                        over_animator.state = AnimatorState::Playing;
                    }
                    LifebarFillSeqPhase::FillUp(mut bar_index) => {
                        bar_index += 1;
                        if bar_index < hud.lifebars.len() {
                            hud.index = bar_index;
                            hud.fill_seq = LifebarFillSeqPhase::FillUp(bar_index);
                            over_animator.state = AnimatorState::Playing;
                            need_color_update = true;
                        } else {
                            hud.fill_seq = LifebarFillSeqPhase::Ready;
                        }
                    }
                    LifebarFillSeqPhase::SlideOut => {
                        hud.fill_seq = LifebarFillSeqPhase::Idle;
                    }
                    _ => (),
                }
            }

            // Update lifetime bars from damage events
            if hud.fill_seq == LifebarFillSeqPhase::Ready {
                if let Some(ev) = update_events
                    .iter()
                    .filter(|ev| ev.entity == hud_entity)
                    .last()
                {
                    let total_life = (hud.life * hud.lifebars.len() as f32).max(1.);
                    let new_index = ev.remain_life / hud.life;
                    let over_progress = new_index.fract();
                    let new_index = new_index.floor() as usize;
                    hud.remain_life = over_progress * hud.life;
                    // println!(
                    //     "hud: life_per_bar={} lifebar_count={} total_life={} remain_life={} bar_index={} bar_remain_life={}",
                    //     hud.life, hud.lifebars.len(), total_life, ev.remain_life, new_index, hud.remain_life
                    // );
                    if hud.index != new_index {
                        // Change bars
                        hud.index = new_index;
                        need_color_update = true;
                        // if hud.index == 0 && hud.remain_life <= 0. {
                        //     // killed
                        //     println!("ENTITY KILLED");
                        //     // {
                        //     //     let sound_channel_sfx = KiraAudioChannel::new("sfx".to_string());
                        //     //     audio.set_volume_in_channel(0.7, &sound_channel_sfx);
                        //     //     let sound_click = asset_server.load("sounds/explosion.ogg");
                        //     //     audio.play_in_channel(sound_click.clone(), &sound_channel_sfx);
                        //     // }
                        //     hud.fill_seq = LifebarFillSeqPhase::SlideOut;
                        //     animator.set_tweenable(Tween::new(
                        //         EaseMethod::Linear,
                        //         TweeningType::Once,
                        //         Duration::from_secs_f32(2.5),
                        //         TransformPositionLens {
                        //             start: transform.translation,
                        //             end: Vec3::new(
                        //                 hud.hidden_pos.x,
                        //                 hud.hidden_pos.y,
                        //                 transform.translation.z,
                        //             ),
                        //         },
                        //     ));
                        //     animator.rewind();
                        //     animator.state = AnimatorState::Playing;
                        // }
                    }

                    // Scale overbar by progress
                    match hud.orientation {
                        LifebarOrientation::Horizontal => {
                            over_transform.scale = Vec3::new(over_progress, 1., 1.)
                        }
                        LifebarOrientation::Vertical => {
                            over_transform.scale = Vec3::new(1., over_progress, 1.)
                        }
                    }
                }
            }
        }

        // Update bars color
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
