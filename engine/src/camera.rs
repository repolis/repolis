use bevy::input::mouse::{MouseMotion, MouseScrollUnit, MouseWheel};
use bevy::prelude::*;

/// Vertical field of view of the city camera, mirrored in `setup_scene` so the
/// framing maths and the projection cannot drift apart.
pub const CAMERA_FOV: f32 = std::f32::consts::FRAC_PI_4;

/// Fraction of the viewport height a focused object should fill.
const FOCUS_FILL: f32 = 0.55;

#[derive(Component)]
pub struct CityCamera {
    pub focus: Vec3,
    pub radius: f32,
    pub alpha: f32,
    pub beta: f32,
    /// Targets for smooth interpolation when focusing on something.
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
            alpha: std::f32::consts::FRAC_PI_4,
            beta: 0.62,
            target_focus: Vec3::ZERO,
            target_radius: 200.0,
            home_focus: Vec3::ZERO,
            home_radius: 200.0,
        }
    }
}

impl CityCamera {
    /// Sets both the current and the "home" view, used by the reset button.
    pub fn frame(&mut self, center: Vec3, radius: f32) {
        self.focus = center;
        self.target_focus = center;
        self.radius = radius;
        self.target_radius = radius;
        self.home_focus = center;
        self.home_radius = radius;
        self.alpha = std::f32::consts::FRAC_PI_4;
        self.beta = 0.62;
    }

    /// Frames an object so it fills roughly FOCUS_FILL of the viewport height.
    /// `size` must be the larger of footprint and height: scaling by the
    /// footprint alone frames a tall tower as its own base and puts the camera
    /// inside it.
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
        let (sa, ca) = self.alpha.sin_cos();
        let (sb, cb) = self.beta.sin_cos();
        let pos = Vec3::new(
            self.focus.x + self.radius * sa * cb,
            self.focus.y + self.radius * sb,
            self.focus.z + self.radius * ca * cb,
        );
        *transform = Transform::from_translation(pos).looking_at(self.focus, Vec3::Y);
    }
}

pub fn camera_controls(
    mut query: Query<(&mut Transform, &mut CityCamera)>,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    mut motion: EventReader<MouseMotion>,
    mut wheel: EventReader<MouseWheel>,
    time: Res<Time>,
) {
    let dt = time.delta_seconds();

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
            cam.alpha -= drag.x * 0.005;
            cam.beta = (cam.beta + drag.y * 0.005).clamp(0.06, 1.52);
        }

        // WASD pans the orbit centre.
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
            let (sa, ca) = cam.alpha.sin_cos();
            let forward = Vec3::new(-sa, 0.0, -ca);
            let right = Vec3::new(ca, 0.0, -sa);
            let delta = (forward * pan.y + right * pan.x) * speed;
            cam.focus += delta;
            cam.target_focus += delta;
        }

        if scroll != 0.0 {
            // Purely proportional, so a notch always covers the same
            // fraction of the distance. A flat floor makes one notch
            // jump straight through whatever is being inspected.
            let factor = (1.0 - scroll * 0.12).clamp(0.45, 2.2);
            cam.target_radius = (cam.target_radius * factor).clamp(2.5, 20000.0);
        }

        // Damped, so focusing reads as a move rather than a teleport.
        let k = (1.0 - (-12.0 * dt).exp()).clamp(0.0, 1.0);
        let focus_delta = (cam.target_focus - cam.focus) * k;
        let radius_delta = (cam.target_radius - cam.radius) * k;
        cam.focus += focus_delta;
        cam.radius += radius_delta;

        cam.apply(&mut transform);
    }
}
