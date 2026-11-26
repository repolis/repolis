use bevy::input::mouse::{MouseMotion, MouseScrollUnit, MouseWheel};
use bevy::prelude::*;

/// Mirrored in `setup_scene` so framing maths and projection cannot drift apart.
pub const CAMERA_FOV: f32 = std::f32::consts::FRAC_PI_4;

/// Fraction of the viewport height a focused object should fill.
const FOCUS_FILL: f32 = 0.55;

const DEFAULT_YAW: f32 = std::f32::consts::FRAC_PI_4;
const DEFAULT_PITCH: f32 = 0.62;
const MIN_PITCH: f32 = 0.06;
const MAX_PITCH: f32 = 1.52;
const MIN_RADIUS: f32 = 2.5;
const MAX_RADIUS: f32 = 20000.0;

#[derive(Component)]
pub struct CityCamera {
    pub focus: Vec3,
    pub radius: f32,
    pub yaw: f32,
    pub pitch: f32,
    pub target_focus: Vec3,
    pub target_radius: f32,
    pub home_focus: Vec3,
    pub home_radius: f32,
}

impl Default for CityCamera {
    fn default() -> Self {
        Self {
            focus: Vec3::ZERO,
            radius: 200.0,
            yaw: DEFAULT_YAW,
            pitch: DEFAULT_PITCH,
            target_focus: Vec3::ZERO,
            target_radius: 200.0,
            home_focus: Vec3::ZERO,
            home_radius: 200.0,
        }
    }
}

impl CityCamera {
    /// Also sets the "home" view used by reset.
    pub fn frame(&mut self, center: Vec3, radius: f32) {
        self.focus = center;
        self.target_focus = center;
        self.radius = radius;
        self.target_radius = radius;
        self.home_focus = center;
        self.home_radius = radius;
        self.yaw = DEFAULT_YAW;
        self.pitch = DEFAULT_PITCH;
    }

    /// `size` must be max(footprint, height), or a tall tower puts the camera inside it.
    pub fn frame_object(&mut self, center: Vec3, size: f32) {
        // Half the object subtends half of FOCUS_FILL of the vertical FOV.
        let half_angle = (CAMERA_FOV * 0.5 * FOCUS_FILL).max(0.01);
        let radius = (size * 0.5) / half_angle.tan();
        self.target_focus = center;
        self.target_radius = radius.clamp(8.0, self.home_radius.max(8.0));
    }

    pub fn go_home(&mut self) {
        self.target_focus = self.home_focus;
        self.target_radius = self.home_radius;
    }

    pub fn apply(&self, transform: &mut Transform) {
        let (sin_yaw, cos_yaw) = self.yaw.sin_cos();
        let (sin_pitch, cos_pitch) = self.pitch.sin_cos();
        let pos = Vec3::new(
            self.focus.x + self.radius * sin_yaw * cos_pitch,
            self.focus.y + self.radius * sin_pitch,
            self.focus.z + self.radius * cos_yaw * cos_pitch,
        );
        *transform = Transform::from_translation(pos).looking_at(self.focus, Vec3::Y);
    }

    pub fn to_state(&self) -> String {
        format!(
            "{:.1},{:.1},{:.1},{:.3},{:.3}",
            self.focus.x, self.focus.z, self.radius, self.yaw, self.pitch
        )
    }

    pub fn apply_state(&mut self, state: &str) {
        let values: Vec<f32> = state.split(',').filter_map(|v| v.parse().ok()).collect();
        let &[x, z, radius, yaw, pitch] = values.as_slice() else {
            return;
        };
        self.focus = Vec3::new(x, 0.0, z);
        self.target_focus = self.focus;
        self.radius = radius.clamp(MIN_RADIUS, MAX_RADIUS);
        self.target_radius = self.radius;
        self.yaw = yaw;
        self.pitch = pitch.clamp(MIN_PITCH, MAX_PITCH);
    }
}

pub fn camera_controls(
    mut query: Query<(&mut Transform, &mut CityCamera)>,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    mut motion: MessageReader<MouseMotion>,
    mut wheel: MessageReader<MouseWheel>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();

    let mut drag = Vec2::ZERO;
    let dragging = buttons.pressed(MouseButton::Left) || buttons.pressed(MouseButton::Right);
    let panning = buttons.pressed(MouseButton::Middle) || buttons.pressed(MouseButton::Right);
    for ev in motion.read() {
        if dragging {
            drag += ev.delta;
        }
    }
    // Browsers report wheel deltas in pixels, ~100 per notch; a desktop mouse
    // reports one line per notch. Summing `ev.y` blind is a 100x difference.
    let mut scroll = 0.0;
    for ev in wheel.read() {
        scroll += match ev.unit {
            MouseScrollUnit::Line => ev.y,
            MouseScrollUnit::Pixel => ev.y / 90.0,
        };
    }
    scroll = scroll.clamp(-4.0, 4.0);

    for (mut transform, mut cam) in query.iter_mut() {
        if keys.just_pressed(KeyCode::KeyR) {
            cam.go_home();
        }

        if drag != Vec2::ZERO && !panning {
            cam.yaw -= drag.x * 0.005;
            cam.pitch = (cam.pitch + drag.y * 0.005).clamp(MIN_PITCH, MAX_PITCH);
        }

        let mut pan = Vec2::ZERO;
        if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) {
            pan.y += 1.0;
        }
        if keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown) {
            pan.y -= 1.0;
        }
        if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) {
            pan.x -= 1.0;
        }
        if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) {
            pan.x += 1.0;
        }
        if panning && drag != Vec2::ZERO {
            pan.x -= drag.x * 0.12;
            pan.y += drag.y * 0.12;
        }
        if pan != Vec2::ZERO {
            let speed = cam.radius * 0.6 * dt;
            let (sin_yaw, cos_yaw) = cam.yaw.sin_cos();
            let forward = Vec3::new(-sin_yaw, 0.0, -cos_yaw);
            let right = Vec3::new(cos_yaw, 0.0, -sin_yaw);
            let delta = (forward * pan.y + right * pan.x) * speed;
            cam.focus += delta;
            cam.target_focus += delta;
        }

        if scroll != 0.0 {
            // Purely proportional: a flat floor makes one notch jump through the target.
            let factor = (1.0 - scroll * 0.12).clamp(0.45, 2.2);
            cam.target_radius = (cam.target_radius * factor).clamp(MIN_RADIUS, MAX_RADIUS);
        }

        let ease = (1.0 - (-12.0 * dt).exp()).clamp(0.0, 1.0);
        let focus_delta = (cam.target_focus - cam.focus) * ease;
        let radius_delta = (cam.target_radius - cam.radius) * ease;
        cam.focus += focus_delta;
        cam.radius += radius_delta;

        cam.apply(&mut transform);
    }
}
