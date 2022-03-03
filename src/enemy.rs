use bevy::{
    app::CoreStage,
    asset::AssetStage,
    pbr::{NotShadowCaster, NotShadowReceiver},
    prelude::*,
};
use heron::prelude::*;

use crate::{AppState, Bullet, Layer, Quad};

pub struct EnemyPlugin;

impl Plugin for EnemyPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EnemyManager>()
            .add_system_set_to_stage(
                CoreStage::Update,
                SystemSet::on_enter(AppState::InGame).with_system(enemy_setup),
            )
            .add_system_set_to_stage(
                CoreStage::Update,
                SystemSet::on_update(AppState::InGame).with_system(enemy_update),
            );
    }
}

#[derive(Default)]
struct EnemyManager {
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
    bullet_mesh: Handle<Mesh>,
    bullet_material: Handle<StandardMaterial>,
}

impl EnemyManager {
    fn spawn(&self, mut commands: Commands, position: Vec3) {
        println!("SPAWN ENEMY @ {:?}", position);
        let mut enemy_controller = EnemyController::default();
        enemy_controller.fire_tag = Some(Box::new(FireTagSpiral {
            arms_count: 6,
            bullet_speed: 4.3,
            fire_delay: 0.04,
            rotate_speed: 35_f32.to_radians(),
            //
            cur_time: 0.,
            cur_angle: 0.,
        }));
        commands
            .spawn_bundle(PbrBundle {
                mesh: self.mesh.clone(),
                material: self.material.clone(),
                transform: Transform::from_translation(position),
                ..Default::default()
            })
            .insert(Name::new("Enemy"))
            .insert(enemy_controller)
            // Physics
            .insert(RigidBody::KinematicPositionBased)
            .insert(CollisionShape::Sphere { radius: 0.1 })
            //.insert(Velocity::from_linear(Vec3::X * 5.))
            //.insert(RotationConstraints::lock())
            .insert(
                CollisionLayers::none()
                    .with_group(Layer::Enemy)
                    .with_masks(&[Layer::World, Layer::Player, Layer::PlayerBullet]),
            );
    }
}

struct FireTagContext<'w, 's> {
    dt: f32,
    origin: Vec3,
    commands: Commands<'w, 's>,
    bullet_mesh: Handle<Mesh>,
    bullet_material: Handle<StandardMaterial>,
}

impl<'w, 's> FireTagContext<'w, 's> {
    fn fire(&mut self, angle: f32, speed: f32) {
        println!(
            "FIRE: origin={:?} angle={} speed={}",
            self.origin, angle, speed
        );
        let rot = Quat::from_rotation_z(angle);
        self.commands
            .spawn_bundle(PbrBundle {
                mesh: self.bullet_mesh.clone(),
                material: self.bullet_material.clone(),
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
    //
    cur_time: f32,
    cur_angle: f32,
}

impl FireTag for FireTagSpiral {
    fn execute(&mut self, mut context: &mut FireTagContext) {
        let dt = context.dt;
        self.cur_time += dt;
        if self.cur_time >= self.fire_delay {
            self.cur_time = 0.; // for safety, run at most once per frame
            let delta_angle = std::f32::consts::TAU / self.arms_count as f32;
            let mut angle = self.cur_angle;
            // repeat
            for idx in 0..self.arms_count {
                context.fire(angle, self.bullet_speed);
                // sequence
                angle += delta_angle;
            }
        }
        // sequence
        self.cur_angle += self.rotate_speed * dt;
    }
}

#[derive(Component)]
struct EnemyController {
    fire_tag: Option<Box<dyn FireTag + Send + Sync>>,
}

impl Default for EnemyController {
    fn default() -> Self {
        EnemyController { fire_tag: None }
    }
}

impl EnemyController {
    fn update<'w, 's>(
        &mut self,
        dt: f32,
        origin: Vec3,
        mut commands: Commands<'w, 's>,
        bullet_mesh: Handle<Mesh>,
        bullet_material: Handle<StandardMaterial>,
    ) -> Commands<'w, 's> {
        let mut context = FireTagContext {
            dt,
            origin,
            commands,
            bullet_mesh,
            bullet_material,
        };
        if let Some(fire_tag) = &mut self.fire_tag {
            fire_tag.execute(&mut context);
        }
        context.commands
    }
}

fn enemy_setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut manager: ResMut<EnemyManager>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let bullet_texture = asset_server.load("textures/bullet_dev_16.png");
    let bullet_mesh = meshes.add(Mesh::from(Quad { size: 0.1 }));
    let bullet_material = materials.add(StandardMaterial {
        base_color_texture: Some(bullet_texture),
        //emissive: Color::RED,
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        ..Default::default()
    });

    manager.mesh = meshes.add(Mesh::from(shape::Cube { size: 0.1 }));
    manager.material = materials.add(Color::rgb(0.8, 0.7, 0.6).into());
    manager.bullet_mesh = bullet_mesh;
    manager.bullet_material = bullet_material;

    // TEMP
    manager.spawn(commands, Vec3::new(2., 0., 0.));
}

fn enemy_update(
    mut commands: Commands,
    mut query: Query<(&mut EnemyController, &Transform)>,
    time: Res<Time>,
    manager: Res<EnemyManager>,
) {
    for (mut enemy, transform) in query.iter_mut() {
        commands = enemy.update(
            time.delta_seconds(),
            transform.translation,
            commands,
            manager.bullet_mesh.clone(),
            manager.bullet_material.clone(),
        );
    }
}
