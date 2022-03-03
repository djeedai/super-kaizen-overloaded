use bevy::{
    app::CoreStage,
    asset::AssetStage,
    pbr::{NotShadowCaster, NotShadowReceiver},
    prelude::*,
    utils::HashMap,
};
use bevy_tweening::{lens::*, *};
use heron::prelude::*;
use serde::Deserialize;
use std::{
    f32::consts::{PI, TAU},
    time::Duration,
};

use crate::{
    game::{DamageEvent, LifebarHud, LifebarOrientation, PlayerController, UpdateLifebarsEvent},
    AppState, Bullet, Layer, Quad,
};

pub struct EnemyPlugin;

impl Plugin for EnemyPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EnemyManager>()
            .add_system_set_to_stage(
                CoreStage::Update,
                SystemSet::on_enter(AppState::InGame).with_system(setup_enemy),
            )
            .add_system_set_to_stage(
                CoreStage::Update,
                SystemSet::on_update(AppState::InGame).with_system(update_enemy),
            );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
enum BulletKind {
    #[serde(alias = "pink_donut")]
    PinkDonut,
    #[serde(alias = "white_ball")]
    WhiteBall,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
enum FireTagKind {
    #[serde(alias = "spiral")]
    Spiral,
    #[serde(alias = "aim_burst")]
    AimBurst,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
enum MotionPatternKind {
    #[serde(alias = "enter_stay")]
    EnterStay,
    #[serde(alias = "fly_by")]
    FlyBy,
}

#[derive(Debug, Clone, Deserialize)]
struct EnemyDescriptor {
    name: String,
    life: f32,
    #[serde(default)]
    is_boss: bool,
    fire_tag_kind: FireTagKind,
    motion_pattern_kind: MotionPatternKind,
    bullet_kind: BulletKind,
    #[serde(skip)]
    enemy_mesh: Handle<Mesh>,
    #[serde(skip)]
    enemy_material: Handle<StandardMaterial>,
    #[serde(skip)]
    bullet_mesh: Handle<Mesh>,
    #[serde(skip)]
    bullet_material: Handle<StandardMaterial>,
}

#[derive(Debug, Clone, Deserialize)]
struct TimelineEvent {
    time: f64,
    enemy: String,
    start_pos: Vec3,
}

#[derive(Default)]
struct Timeline {
    events: Vec<TimelineEvent>,
    index: usize,
    time: f64,
}

#[derive(Debug, Clone, Deserialize)]
struct EnemyDatabase {
    enemies: Vec<EnemyDescriptor>,
    timeline: Vec<TimelineEvent>,
}

struct BulletAssets {
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
}

struct EnemyManager {
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
    boss_lifebar_entity: Entity,
    descriptors: HashMap<String, EnemyDescriptor>,
    bullet_assets: HashMap<BulletKind, BulletAssets>,
    timeline: Timeline,
}

impl Default for EnemyManager {
    fn default() -> Self {
        EnemyManager {
            mesh: Handle::default(),
            material: Handle::default(),
            boss_lifebar_entity: Entity::from_raw(0),
            descriptors: HashMap::default(),
            bullet_assets: HashMap::default(),
            timeline: Timeline::default(),
        }
    }
}

impl EnemyManager {
    fn add_descriptor(&mut self, descriptor: EnemyDescriptor) {
        self.descriptors.insert(descriptor.name.clone(), descriptor);
    }

    fn execute_timeline(&mut self, dt: f32, commands: &mut Commands) {
        self.timeline.time += dt as f64;
        for index in self.timeline.index..self.timeline.events.len() {
            let ev = &self.timeline.events[index];
            if ev.time > self.timeline.time {
                self.timeline.index = index;
                return;
            }
            self.spawn(commands, &ev.enemy, ev.start_pos);
        }
        self.timeline.index = self.timeline.events.len(); // timeline done
    }

    fn spawn(&self, commands: &mut Commands, desc: &str, position: Vec3) {
        if let Some(desc) = self.descriptors.get(&desc.to_owned()) {
            let motion_pattern: Box<dyn MotionPattern + Send + Sync> =
                match &desc.motion_pattern_kind {
                    MotionPatternKind::EnterStay => {
                        let mut motion = EnterStayMotion::default();
                        motion.enter_height = position.y;
                        Box::new(motion)
                    }
                    MotionPatternKind::FlyBy => {
                        let mut motion = FlyByMotion::default();
                        motion.start = position;
                        motion.direction = if position.y > 0. {
                            Vec3::new(-1., 0.25, 0.)
                        } else {
                            Vec3::new(-1., -0.25, 0.)
                        };
                        Box::new(motion)
                    }
                };
            let bullet_assets = self.bullet_assets.get(&desc.bullet_kind).unwrap();
            let fire_tag: Box<dyn FireTag + Send + Sync> = match &desc.fire_tag_kind {
                FireTagKind::Spiral => {
                    let mut fire_tag = FireTagSpiral::default();
                    fire_tag.bullet_mesh = bullet_assets.mesh.clone();
                    fire_tag.bullet_material = bullet_assets.material.clone();
                    Box::new(fire_tag)
                }
                FireTagKind::AimBurst => {
                    let mut fire_tag = FireTagAimBurst::default();
                    fire_tag.bullet_mesh = bullet_assets.mesh.clone();
                    fire_tag.bullet_material = bullet_assets.material.clone();
                    Box::new(fire_tag)
                }
            };

            let mut enemy_controller = EnemyController::default();
            enemy_controller.motion_pattern = Some(motion_pattern);
            enemy_controller.fire_tag = Some(fire_tag);
            enemy_controller.life = desc.life;
            enemy_controller.remain_life = desc.life;

            let entity = commands
                .spawn_bundle(PbrBundle {
                    mesh: self.mesh.clone(),
                    material: self.material.clone(),
                    transform: Transform::from_translation(position),
                    ..Default::default()
                })
                .insert(Name::new(desc.name.clone()))
                .insert(enemy_controller)
                .insert(Animator::<Transform>::default().with_state(AnimatorState::Paused))
                // Physics
                .insert(RigidBody::KinematicPositionBased)
                .insert(CollisionShape::Sphere { radius: 0.1 })
                //.insert(Velocity::from_linear(Vec3::X * 5.))
                //.insert(RotationConstraints::lock())
                .insert(
                    CollisionLayers::none()
                        .with_group(Layer::Enemy)
                        .with_masks(&[Layer::World, Layer::Player, Layer::PlayerBullet]),
                )
                .id();
            println!("SPAWNED ENEMY {:?} @ {:?}", entity, position);
        } else {
            println!("Failed to spawn unknown enemy type '{}'", desc);
        }
    }
}

struct FireTagContext<'w, 's, 'ctx> {
    dt: f32,
    origin: Vec3,
    player_position: Vec3,
    commands: &'ctx mut Commands<'w, 's>,
}

impl<'w, 's, 'ctx> FireTagContext<'w, 's, 'ctx> {
    fn new(
        dt: f32,
        origin: Vec3,
        player_position: Vec3,
        commands: &'ctx mut Commands<'w, 's>,
    ) -> Self {
        FireTagContext {
            dt,
            origin,
            player_position,
            commands,
        }
    }

    fn fire(
        &mut self,
        rot: Quat,
        speed: f32,
        mesh: Handle<Mesh>,
        material: Handle<StandardMaterial>,
    ) {
        // println!(
        //     "FIRE: origin={:?} angle={} speed={}",
        //     self.origin, angle, speed
        // );
        self.commands
            .spawn_bundle(PbrBundle {
                mesh,
                material,
                transform: Transform::from_rotation(rot).with_translation(self.origin),
                ..Default::default()
            })
            .insert(Bullet(Vec3::X * speed))
            // Rendering
            .insert(NotShadowCaster)
            .insert(NotShadowReceiver)
            // Physics
            .insert(RigidBody::Dynamic) // TODO - or Dynamic?
            .insert(CollisionShape::Sphere { radius: 0.1 })
            .insert(Velocity::from_linear(rot.mul_vec3(Vec3::X * speed)))
            .insert(RotationConstraints::lock())
            .insert(
                CollisionLayers::none()
                    .with_group(Layer::EnemyBullet)
                    .with_masks(&[Layer::World, Layer::Player]),
            );
    }
}

trait FireTag {
    fn execute(&mut self, context: &mut FireTagContext);
}

struct FireTagSpiral {
    arms_count: i32,
    bullet_speed: f32,
    fire_delay: f32,
    rotate_speed: f32,
    bullet_mesh: Handle<Mesh>,
    bullet_material: Handle<StandardMaterial>,
    //
    cur_time: f32,
    cur_angle: f32,
    cur_iter: i32,
}

impl Default for FireTagSpiral {
    fn default() -> Self {
        FireTagSpiral {
            arms_count: 6,
            bullet_speed: 4.3,
            fire_delay: 0.04,
            rotate_speed: 35_f32.to_radians(),
            bullet_mesh: Handle::default(),
            bullet_material: Handle::default(),
            //
            cur_time: 0.,
            cur_angle: 0.,
            cur_iter: 0,
        }
    }
}

impl FireTag for FireTagSpiral {
    fn execute(&mut self, mut context: &mut FireTagContext) {
        let dt = context.dt;
        // println!(
        //     "EXEC: dt={} cur_angle={} cur_iter={}",
        //     dt, self.cur_angle, self.cur_iter
        // );
        self.cur_time += dt;
        let cone_angle = 30_f32.to_radians(); // need to be >= 60 deg for 6 arms, othewise there's a time gap!
        if self.cur_time >= self.fire_delay {
            self.cur_time = 0.; // for safety, run at most once per frame
            let delta_angle = TAU / self.arms_count as f32;
            let mut angle = self.cur_angle % TAU;
            // find the arm with a direction aiming closest to the player
            // we need to stop firing for a bit always on the same arm, otherwise
            // it's useless if this is distributed across 2 arms (not enough space
            // on either of them to safely pass through).
            let player_angle = PI; // TODO
            let aim_arm_idx = (0..self.arms_count)
                .map(|idx| (idx, (angle + delta_angle * idx as f32) % TAU))
                .min_by(|(idx0, angle0), (id1, angle1)| {
                    // equality cannot happen since arms are evenly spaced out
                    if (angle0 - player_angle).abs() <= (angle1 - player_angle).abs() {
                        std::cmp::Ordering::Less
                    } else {
                        std::cmp::Ordering::Greater
                    }
                })
                .map(|(idx, _)| idx)
                .unwrap_or(0);
            //println!("AIM ARM = #{}", aim_arm_idx);
            self.cur_iter += 1;
            // repeat
            for idx in 0..self.arms_count {
                // println!(
                //     "ARM #{}: angle={} min={} max={}",
                //     idx,
                //     angle,
                //     PI - cone_angle,
                //     PI + cone_angle
                // );
                if self.cur_iter % 25 >= 5 || idx != aim_arm_idx {
                    let rot = Quat::from_rotation_z(angle);
                    context.fire(
                        rot,
                        self.bullet_speed,
                        self.bullet_mesh.clone(),
                        self.bullet_material.clone(),
                    );
                }
                // sequence
                angle = (angle + delta_angle) % TAU;
            }
        }
        // sequence
        self.cur_angle = (self.cur_angle + self.rotate_speed * dt) % TAU;
    }
}

struct FireTagAimBurst {
    bullet_count: i32,
    bullet_speed: f32,
    fire_delay: f32,
    bullet_mesh: Handle<Mesh>,
    bullet_material: Handle<StandardMaterial>,
    //
    cur_time: f32,
    cur_iter: i32,
}

impl Default for FireTagAimBurst {
    fn default() -> Self {
        FireTagAimBurst {
            bullet_count: 6,
            bullet_speed: 2.1,
            fire_delay: 0.04,
            bullet_mesh: Handle::default(),
            bullet_material: Handle::default(),
            //
            cur_time: 0.,
            cur_iter: 0,
        }
    }
}

impl FireTag for FireTagAimBurst {
    fn execute(&mut self, mut context: &mut FireTagContext) {
        if self.cur_iter < self.bullet_count {
            let dt = context.dt;
            // println!(
            //     "EXEC: dt={} cur_angle={} cur_iter={}",
            //     dt, self.cur_angle, self.cur_iter
            // );
            self.cur_time += dt;
            if self.cur_time >= self.fire_delay {
                self.cur_time = 0.; // for safety, run at most once per frame
                let dir = (context.player_position - context.origin)
                    .try_normalize()
                    .unwrap_or(Vec3::X);
                let rot = Quat::from_rotation_arc(Vec3::X, dir);
                context.fire(
                    rot,
                    self.bullet_speed,
                    self.bullet_mesh.clone(),
                    self.bullet_material.clone(),
                );
                self.cur_iter += 1;
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MotionResult {
    DoNothing,
    StartFireTag,
}

trait MotionPattern {
    fn do_motion(
        &mut self,
        dt: f32,
        transform: &mut Transform,
        animator: &mut Animator<Transform>,
    ) -> MotionResult;
}

enum EnterStayPhase {
    Idle,
    Enter,
    Stay,
}

struct EnterStayMotion {
    phase: EnterStayPhase,
    enter_height: f32,
}

impl Default for EnterStayMotion {
    fn default() -> Self {
        EnterStayMotion {
            phase: EnterStayPhase::Idle,
            enter_height: 0.,
        }
    }
}

impl MotionPattern for EnterStayMotion {
    fn do_motion(
        &mut self,
        dt: f32,
        transform: &mut Transform,
        animator: &mut Animator<Transform>,
    ) -> MotionResult {
        match self.phase {
            EnterStayPhase::Idle => {
                self.phase = EnterStayPhase::Enter;
                transform.translation = Vec3::new(5., self.enter_height, 0.);
                let tween = Tween::new(
                    EaseFunction::QuadraticOut,
                    TweeningType::Once,
                    Duration::from_secs_f32(5.),
                    TransformPositionLens {
                        start: transform.translation,
                        end: Vec3::new(2., self.enter_height, 0.),
                    },
                );
                animator.set_tweenable(tween);
                animator.state = AnimatorState::Playing;
                MotionResult::DoNothing
            }
            EnterStayPhase::Enter => {
                if animator.progress() >= 1. {
                    self.phase = EnterStayPhase::Stay;
                    let tween = Tween::new(
                        EaseFunction::QuadraticInOut,
                        TweeningType::PingPong,
                        Duration::from_secs_f32(3.),
                        TransformPositionLens {
                            start: transform.translation,
                            end: transform.translation + Vec3::Y * 0.6,
                        },
                    );
                    animator.set_tweenable(tween);
                    animator.state = AnimatorState::Playing;
                    MotionResult::StartFireTag
                } else {
                    MotionResult::DoNothing
                }
            }
            EnterStayPhase::Stay => MotionResult::DoNothing,
        }
    }
}

struct FlyByMotion {
    start: Vec3,
    direction: Vec3,
    has_fired: bool,
}

impl Default for FlyByMotion {
    fn default() -> Self {
        FlyByMotion {
            start: Vec3::ZERO,
            direction: Vec3::ZERO,
            has_fired: false,
        }
    }
}

impl MotionPattern for FlyByMotion {
    fn do_motion(
        &mut self,
        dt: f32,
        transform: &mut Transform,
        animator: &mut Animator<Transform>,
    ) -> MotionResult {
        match &animator.state {
            AnimatorState::Paused => {
                let tween = Tween::new(
                    EaseFunction::QuadraticOut,
                    TweeningType::Once,
                    Duration::from_secs_f32(5.),
                    TransformPositionLens {
                        start: self.start,
                        end: self.start + self.direction * 6.,
                    },
                );
                animator.set_tweenable(tween);
                animator.state = AnimatorState::Playing;
                MotionResult::DoNothing
            }
            AnimatorState::Playing => {
                if !self.has_fired && animator.progress() >= 0.3 {
                    self.has_fired = true;
                    MotionResult::StartFireTag
                } else {
                    MotionResult::DoNothing
                }
            }
        }
    }
}

#[derive(Component)]
struct EnemyController {
    motion_pattern: Option<Box<dyn MotionPattern + Send + Sync>>,
    fire_tag: Option<Box<dyn FireTag + Send + Sync>>,
    fire_tag_started: bool,
    life: f32,
    remain_life: f32,
}

impl Default for EnemyController {
    fn default() -> Self {
        EnemyController {
            motion_pattern: None,
            fire_tag: None,
            fire_tag_started: false,
            life: 0.,
            remain_life: 0.,
        }
    }
}

impl EnemyController {
    fn update(
        &mut self,
        dt: f32,
        origin: Vec3,
        player_position: Vec3,
        commands: &mut Commands,
        transform: &mut Transform,
        animator: &mut Animator<Transform>,
    ) {
        // Move
        if let Some(motion_pattern) = &mut self.motion_pattern {
            if motion_pattern.do_motion(dt, transform, animator) == MotionResult::StartFireTag {
                self.fire_tag_started = true;
            }
        }

        // Fire
        if self.fire_tag_started {
            //println!("ENEMY_UPDATE: dt={} origin={:?}", dt, origin);
            let mut context = FireTagContext::new(dt, origin, player_position, commands);
            if let Some(fire_tag) = &mut self.fire_tag {
                fire_tag.execute(&mut context);
            }
        }
    }
}

fn setup_enemy(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut manager: ResMut<EnemyManager>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    manager.bullet_assets.insert(
        BulletKind::PinkDonut,
        BulletAssets {
            mesh: meshes.add(Mesh::from(Quad { size: 0.1 })),
            material: materials.add(StandardMaterial {
                base_color_texture: Some(asset_server.load("textures/bullet2.png")),
                //emissive: Color::RED,
                unlit: true,
                alpha_mode: AlphaMode::Blend,
                ..Default::default()
            }),
        },
    );
    manager.bullet_assets.insert(
        BulletKind::WhiteBall,
        BulletAssets {
            mesh: meshes.add(Mesh::from(Quad { size: 0.08 })),
            material: materials.add(StandardMaterial {
                base_color_texture: Some(asset_server.load("textures/bullet3.png")),
                //emissive: Color::WHITE,
                unlit: true,
                alpha_mode: AlphaMode::Blend,
                ..Default::default()
            }),
        },
    );

    // FIXME - Copied from game.rs :(
    let hud_mat_black = materials.add(StandardMaterial {
        base_color: Color::BLACK,
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        ..Default::default()
    });

    // Boss lifebars
    let mut boss_lifebars = LifebarHud::default();
    boss_lifebars.orientation = LifebarOrientation::Horizontal;
    //boss_lifebars.visible_pos = Vec2::new(0., screen_bounds.top + lifebar_margin_v);
    //boss_lifebars.hidden_pos = Vec2::new(0., screen_bounds.top - lifebar_margin_v);
    boss_lifebars.visible_pos = Vec2::new(0., 1.5); // TODO
    boss_lifebars.hidden_pos = Vec2::new(0., 2.0); // TODO
    boss_lifebars.set_lifebars(40.0, [Color::RED, Color::ORANGE, Color::YELLOW]);
    let boss_lifebar_entity = LifebarHud::spawn(
        boss_lifebars,
        "BossLifebar",
        Vec2::new(4.01, 0.05),
        hud_mat_black.clone(),
        Vec2::new(4., 0.04),
        &mut commands,
        &mut *meshes,
        &mut *materials,
    );

    manager.mesh = meshes.add(Mesh::from(shape::Cube { size: 0.1 }));
    manager.material = materials.add(Color::rgb(0.8, 0.7, 0.6).into());
    manager.boss_lifebar_entity = boss_lifebar_entity;

    let mut database: EnemyDatabase =
        serde_json::from_str(&include_str!("../assets/enemy_db.json")).unwrap();
    for descriptor in database.enemies.drain(..) {
        manager.add_descriptor(descriptor);
    }

    manager.timeline.events = database.timeline;

    // TEMP
    // manager.spawn(&mut commands, "fly_by", Vec3::new(5., 0.8, 0.));
    // manager.spawn(&mut commands, "fly_by", Vec3::new(5., -0.8, 0.));
    // manager.spawn(&mut commands, "6_arm_spiral", Vec3::new(3.5, 0., 0.));
}

fn update_enemy(
    mut commands: Commands,
    mut query: Query<
        (
            Entity,
            &mut EnemyController,
            &mut Transform,
            &mut Animator<Transform>,
        ),
        Without<PlayerController>,
    >,
    q_player: Query<&Transform, With<PlayerController>>,
    time: Res<Time>,
    mut manager: ResMut<EnemyManager>,
    mut damage_events: EventReader<DamageEvent>,
    mut lifebar_events: EventWriter<UpdateLifebarsEvent>,
) {
    //println!("update_enemy() t={}", time.seconds_since_startup());

    let dt = time.delta_seconds();

    // Execute timeline
    manager.execute_timeline(dt, &mut commands);

    // need to loop once per enemy, so collect all now
    let damage_events = damage_events.iter().collect::<Vec<_>>();

    for (entity, mut controller, mut transform, mut animator) in query.iter_mut() {
        // Apply damage to enemy
        let damage: f32 = damage_events
            .iter()
            .filter_map(|ev| {
                if ev.entity == entity {
                    Some(ev.damage)
                } else {
                    None
                }
            })
            .sum();
        if damage > 0. {
            controller.remain_life -= damage;
            lifebar_events.send(UpdateLifebarsEvent {
                entity: manager.boss_lifebar_entity,
                remain_life: controller.remain_life,
            });
        }
        if controller.remain_life <= 0. {
            commands.entity(entity).despawn_recursive();
            println!("ENEMY {:?} KILLED", entity);
            return;
        }

        //println!("enemy xform={:?}", transform);
        controller.update(
            dt,
            transform.translation,
            q_player.single().translation,
            &mut commands,
            &mut *transform,
            &mut *animator,
        );
    }
}
