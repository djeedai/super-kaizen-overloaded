use bevy::{
    app::{AppExit, CoreStage},
    asset::AssetStage,
    input::gamepad::GamepadButtonType,
    prelude::*,
};
use bevy_kira_audio::{
    Audio as KiraAudio, AudioChannel as KiraAudioChannel, AudioPlugin as KiraAudioPlugin,
    AudioSource as KiraAudioSource, InstanceHandle,
};
use bevy_tweening::{lens::*, *};
use leafwing_input_manager::prelude::*;
use std::time::Duration;

pub struct MenuPlugin;

use crate::AppState;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(InputManagerPlugin::<MenuAction>::default())
            .add_plugin(KiraAudioPlugin)
            .init_resource::<AudioManager>()
            .add_system_set(
                SystemSet::on_enter(AppState::Menu)
                    .with_system(menu_setup)
                    .with_system(start_background_audio),
            )
            .add_system_set(SystemSet::on_update(AppState::Menu).with_system(menu_run))
            .add_system_set(SystemSet::on_exit(AppState::Menu).with_system(menu_cleanup));
    }
}

#[derive(Actionlike, PartialEq, Eq, Clone, Copy, Hash, Debug)]
enum MenuAction {
    SelectNext,
    SelectPrev,
    ClickButton,
}

#[derive(Component, Default)]
struct Menu {
    selected_index: i32,
    sound_channel_sfx: KiraAudioChannel,
    sound_click: Handle<KiraAudioSource>,
}

#[derive(Component, Default)]
struct Button(pub i32);

pub struct AudioManager {
    pub menu_bgm: Handle<KiraAudioSource>,
    pub menu_instance: Option<InstanceHandle>,
    pub game_bgm: Handle<KiraAudioSource>,
    pub game_instance: Option<InstanceHandle>,
}

impl Default for AudioManager {
    fn default() -> Self {
        AudioManager {
            menu_bgm: Handle::default(),
            menu_instance: None,
            game_bgm: Handle::default(),
            game_instance: None,
        }
    }
}

fn menu_run(
    mut q_menu: Query<(&mut Menu, &ActionState<MenuAction>)>,
    mut q_animators: Query<(&Button, &mut Animator<Transform>)>,
    q_buttons: Query<(&Button, &Node, &GlobalTransform)>,
    mut exit: EventWriter<AppExit>,
    audio: Res<KiraAudio>,
    mut app_state: ResMut<State<AppState>>,
    mut cursor_moved_events: EventReader<CursorMoved>,
    mouse_button_input: Res<Input<MouseButton>>,
) {
    let (mut menu, action_state) = q_menu.single_mut();
    let prev_sel = menu.selected_index;
    if action_state.just_pressed(&MenuAction::SelectNext) {
        menu.selected_index = (menu.selected_index + 1).min(1);
    }
    if action_state.just_pressed(&MenuAction::SelectPrev) {
        menu.selected_index = (menu.selected_index - 1).max(0);
    }
    for ev in cursor_moved_events.iter() {
        for (button, node, transform) in q_buttons.iter() {
            let origin = transform.translation.truncate();
            let half_size = node.size / 2.;
            if (origin.x - ev.position.x).abs() < half_size.x
                && (origin.y - ev.position.y).abs() < half_size.y
            {
                menu.selected_index = button.0;
            }
        }
    }

    if prev_sel != menu.selected_index {
        audio.play_in_channel(menu.sound_click.clone(), &menu.sound_channel_sfx);
        for (button, mut animator) in q_animators.iter_mut() {
            if button.0 == prev_sel {
                let tween_out = Tween::new(
                    EaseFunction::QuadraticInOut,
                    TweeningType::Once,
                    Duration::from_secs_f32(0.4),
                    TransformScaleLens {
                        start: Vec3::new(1.1, 1.1, 1.1),
                        end: Vec3::ONE,
                    },
                );
                animator.set_tweenable(tween_out);
                animator.state = AnimatorState::Playing;
            } else if button.0 == menu.selected_index {
                let tween_in = Tween::new(
                    EaseFunction::QuadraticInOut,
                    TweeningType::Once,
                    Duration::from_secs_f32(0.4),
                    TransformScaleLens {
                        start: Vec3::ONE,
                        end: Vec3::new(1.1, 1.1, 1.1),
                    },
                );
                animator.set_tweenable(tween_in);
                animator.state = AnimatorState::Playing;
            }
        }
    }

    if action_state.just_pressed(&MenuAction::ClickButton) {
        match menu.selected_index {
            0 => app_state.set(AppState::InGame).unwrap(),
            1 => exit.send(AppExit),
            _ => unreachable!(),
        }
    }
}

fn menu_setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    println!("menu_setup");
    commands.spawn_bundle(UiCameraBundle::default());

    let font = asset_server.load("fonts/FiraMono-Regular.ttf");

    let title_image = asset_server.load("title.png");

    let mut menu = Menu::default();
    menu.sound_channel_sfx = KiraAudioChannel::new("sfx".to_string());
    menu.sound_click = asset_server.load("sounds/click4.ogg");

    let mut input_map = InputMap::default();
    input_map.insert(MenuAction::SelectNext, KeyCode::Down);
    input_map.insert(MenuAction::SelectNext, KeyCode::S);
    input_map.insert(MenuAction::SelectNext, GamepadButtonType::DPadDown);
    input_map.insert(MenuAction::SelectPrev, KeyCode::Up);
    input_map.insert(MenuAction::SelectPrev, KeyCode::W);
    input_map.insert(MenuAction::SelectPrev, GamepadButtonType::DPadUp);
    input_map.insert(MenuAction::ClickButton, KeyCode::Return);
    input_map.insert(MenuAction::ClickButton, KeyCode::Space);
    input_map.insert(MenuAction::ClickButton, GamepadButtonType::South);
    #[cfg(not(debug_assertions))] // only in release, otherwise annoying with egui inspector
    input_map.insert(MenuAction::ClickButton, MouseButton::Left);

    let container = commands
        .spawn_bundle(NodeBundle {
            style: Style {
                position_type: PositionType::Absolute,
                position: Rect::all(Val::Px(0.)),
                margin: Rect::all(Val::Px(16.)),
                padding: Rect::all(Val::Px(16.)),
                flex_direction: FlexDirection::ColumnReverse,
                align_content: AlignContent::Center,
                align_items: AlignItems::Center,
                align_self: AlignSelf::Center,
                justify_content: JustifyContent::Center,
                ..Default::default()
            },
            color: UiColor(Color::NONE),
            ..Default::default()
        })
        .insert(Name::new("menu"))
        .insert(menu)
        .insert_bundle(InputManagerBundle::<MenuAction> {
            action_state: ActionState::default(),
            input_map,
        })
        .id();

    // Title
    commands
        .spawn_bundle(NodeBundle {
            node: Node {
                size: Vec2::new(800., 380.),
            },
            style: Style {
                size: Size::new(Val::Px(800.), Val::Px(380.)),
                min_size: Size::new(Val::Px(800.), Val::Px(380.)),
                margin: Rect::all(Val::Px(0.)),
                padding: Rect::all(Val::Px(0.)),
                align_content: AlignContent::Center,
                align_items: AlignItems::Center,
                align_self: AlignSelf::Center,
                justify_content: JustifyContent::Center,
                ..Default::default()
            },
            image: UiImage(title_image),
            ..Default::default()
        })
        .insert(Name::new("title"))
        .insert(Parent(container));

    const DURATION_SEC: f32 = 1.2;
    const DELAY_MS: u64 = 200;

    let mut start_time_ms = 0;
    for (index, text) in ["New Game", "Quit"].iter().enumerate() {
        let delay = Delay::new(Duration::from_millis(start_time_ms));
        start_time_ms += DELAY_MS;
        let tween_scale = Tween::new(
            EaseFunction::BounceOut,
            TweeningType::Once,
            Duration::from_secs_f32(DURATION_SEC),
            TransformScaleLens {
                start: Vec3::ZERO,
                end: if index == 0 {
                    Vec3::new(1.1, 1.1, 1.1)
                } else {
                    Vec3::ONE
                },
            },
        );
        let seq = delay.then(tween_scale.with_completed_event(true, 0));
        commands
            .spawn_bundle(NodeBundle {
                node: Node {
                    size: Vec2::new(300., 80.),
                },
                style: Style {
                    min_size: Size::new(Val::Px(300.), Val::Px(80.)),
                    margin: Rect::all(Val::Px(8.)),
                    padding: Rect::all(Val::Px(8.)),
                    align_content: AlignContent::Center,
                    align_items: AlignItems::Center,
                    align_self: AlignSelf::Center,
                    justify_content: JustifyContent::Center,
                    ..Default::default()
                },
                color: UiColor(Color::rgb_u8(57, 194, 190)),
                transform: Transform::from_scale(Vec3::splat(0.01)),
                ..Default::default()
            })
            .insert(Name::new(format!("button:{}", text)))
            .insert(Button(index as i32))
            .insert(Parent(container))
            .insert(Animator::new(seq))
            .with_children(|parent| {
                parent.spawn_bundle(TextBundle {
                    text: Text::with_section(
                        text.to_string(),
                        TextStyle {
                            font: font.clone(),
                            font_size: 48.0,
                            color: Color::rgb_u8(32, 32, 32),
                        },
                        TextAlignment {
                            vertical: VerticalAlign::Center,
                            horizontal: HorizontalAlign::Center,
                        },
                    ),
                    ..Default::default()
                });
            });
    }
}

fn menu_cleanup(mut commands: Commands, query: Query<Entity, With<Menu>>) {
    commands.entity(query.single()).despawn_recursive();
}

fn start_background_audio(
    asset_server: Res<AssetServer>,
    audio: Res<KiraAudio>,
    mut audio_manager: ResMut<AudioManager>,
) {
    //if config.sound.enabled {
    audio_manager.menu_bgm = asset_server.load("bgm/436507__doctor-dreamchip__2018-08-02.ogg");
    audio.set_volume(1.); //config.sound.volume);
    audio_manager.menu_instance = Some(audio.play_looped(audio_manager.menu_bgm.clone()));
    //}
}
