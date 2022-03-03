use bevy::{app::CoreStage, asset::AssetStage, prelude::*};

use crate::AppState;

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
}

impl EnemyManager {
    fn spawn(&self, mut commands: Commands, position: Vec3) {
        commands
            .spawn_bundle(PbrBundle {
                mesh: self.mesh.clone(),
                material: self.material.clone(),
                transform: Transform::from_translation(position),
                ..Default::default()
            })
            .insert(Name::new("Enemy"))
            .insert(Enemy);
    }
}

#[derive(Component)]
struct Enemy;

impl Enemy {
    fn update(&mut self, dt: f32) {}
}

fn enemy_setup(
    //mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut manager: ResMut<EnemyManager>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    manager.mesh = meshes.add(Mesh::from(shape::Cube { size: 0.1 }));
    manager.material = materials.add(Color::rgb(0.8, 0.7, 0.6).into());
}

fn enemy_update(mut query: Query<&mut Enemy>, time: Res<Time>) {
    for mut enemy in query.iter_mut() {
        enemy.update(time.delta_seconds());
    }
}
