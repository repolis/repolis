use bevy::prelude::*;
use bevy::input::mouse::{MouseMotion, MouseWheel};

#[derive(PartialEq)]
enum CameraMode { Orbit, Free }

#[derive(Component)]
struct CityCamera {
    mode: CameraMode,
    speed: f32,
    rot_speed: f32,
    focus: Vec3,
    radius: f32,
    alpha: f32,
    beta: f32,
}
impl Default for CityCamera {
    fn default() -> Self {
        Self {
            mode: CameraMode::Orbit,
            speed: 120.0,
            rot_speed: 1.5,
            focus: Vec3::ZERO,
            radius: 100.0,
            alpha: 0.0,
            beta: std::f32::consts::FRAC_PI_6,
        }
    }
}

#[derive(Component)]
struct CameraStateText;

fn setup(mut commands: Commands) {
    commands.spawn((
        TextBundle::from_section(
            "Camera: ORBIT (Press F to toggle)",
            TextStyle {
                font_size: 20.0,
                color: Color::WHITE,
                ..default()
            },
        )
        .with_style(Style {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            left: Val::Px(10.0),
            ..default()
        }),
        CameraStateText,
    ));
}

fn camera_controls(
    mut query: Query<(&mut Transform, &mut CityCamera)>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut mouse_motion: EventReader<MouseMotion>,
    mut mouse_wheel: EventReader<MouseWheel>,
    time: Res<Time>,
    mut text_query: Query<&mut Text, With<CameraStateText>>,
) {}
