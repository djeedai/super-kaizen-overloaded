use bevy::{app::CoreStage, asset::AssetStage, prelude::*};

pub struct DebugPlugin;

impl Plugin for DebugPlugin {
    fn build(&self, app: &mut App) {
        app.add_startup_system(fps_counter_setup)
            .add_system(fps_counter)
            // Helper to exit with ESC key
            .add_system(bevy::input::system::exit_on_esc_system);
    }
}

#[derive(Component)]
struct FpsCounter(pub f64);

fn fps_counter_setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn_bundle(UiCameraBundle::default());

    commands
        .spawn_bundle(NodeBundle {
            // root
            style: Style {
                size: Size::new(Val::Percent(100.0), Val::Percent(100.0)),
                justify_content: JustifyContent::Center,
                ..Default::default()
            },
            color: UiColor(Color::NONE),
            ..Default::default()
        })
        .insert(Name::new("FpsCounter"))
        .with_children(|parent| {
            parent
                .spawn_bundle(TextBundle {
                    style: Style {
                        align_self: AlignSelf::FlexEnd,
                        position_type: PositionType::Absolute,
                        position: Rect {
                            top: Val::Px(5.0),
                            right: Val::Px(5.0),
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                    text: Text::with_section(
                        "",
                        TextStyle {
                            font: asset_server.load("fonts/FiraMono-Regular.ttf"),
                            font_size: 14.0,
                            color: Color::rgb_u8(32, 32, 32),
                        },
                        TextAlignment {
                            horizontal: HorizontalAlign::Left,
                            ..Default::default()
                        },
                    ),
                    ..Default::default()
                })
                .insert(FpsCounter(0.));
        });
}

fn fps_counter(mut query: Query<(&mut Text, &mut FpsCounter)>, time: Res<Time>) {
    let (mut text, mut counter) = query.single_mut();
    let now = time.seconds_since_startup();
    if counter.0 + 1. <= now {
        text.sections[0].value = format!("{:.1}ms", time.delta_seconds() * 1000.).into();
        counter.0 = now;
    }
}
