//! Hover feedback in the scene: a faint glass shell that glides to the
//! building under the pointer and fades in and out. Purely visual; picking
//! and selection are untouched, this only reads what picking decided.

use bevy::pbr::{NotShadowCaster, NotShadowReceiver};
use bevy::prelude::*;

use crate::hover::{Pickable, SelectionState};

/// How far the shell stands off the building's faces, in world units.
const PAD: f32 = 0.35;
/// Peak strength of the shell's additive tint.
const TINT: [f32; 3] = [0.30, 0.40, 0.58];

#[derive(Component)]
pub struct HoverShell {
    /// 0..1, eased toward 1 while a building is hovered.
    strength: f32,
    /// Whether the shell has a position yet, so the first hover appears in
    /// place instead of flying in from the origin.
    placed: bool,
}

pub fn spawn_hover_shell(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        PbrBundle {
            mesh: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
            material: materials.add(StandardMaterial {
                base_color: Color::BLACK,
                unlit: true,
                alpha_mode: AlphaMode::Add,
                ..default()
            }),
            visibility: Visibility::Hidden,
            ..default()
        },
        HoverShell {
            strength: 0.0,
            placed: false,
        },
        // Light, not matter: it must not throw a box of shadow on the city.
        NotShadowCaster,
        NotShadowReceiver,
    ));
}

#[allow(clippy::type_complexity)]
pub fn update_hover_shell(
    time: Res<Time>,
    interaction: Res<SelectionState>,
    buildings: Query<&Transform, (With<Pickable>, Without<HoverShell>)>,
    mut shell_q: Query<(
        &mut Transform,
        &mut Visibility,
        &Handle<StandardMaterial>,
        &mut HoverShell,
    )>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Ok((mut tf, mut vis, mat, mut shell)) = shell_q.get_single_mut() else {
        return;
    };
    let dt = time.delta_seconds();
    let target = interaction.hovered.and_then(|e| buildings.get(e).ok());

    if let Some(b) = target {
        let goal_scale = b.scale + Vec3::new(PAD * 2.0, PAD, PAD * 2.0);
        let goal_pos = b.translation + Vec3::Y * (PAD * 0.5);
        if shell.placed && shell.strength > 0.02 {
            // Glide between neighbours rather than jumping.
            let k = 1.0 - (-dt * 18.0).exp();
            tf.translation = tf.translation.lerp(goal_pos, k);
            tf.scale = tf.scale.lerp(goal_scale, k);
            tf.rotation = tf.rotation.slerp(b.rotation, k);
        } else {
            tf.translation = goal_pos;
            tf.scale = goal_scale;
            tf.rotation = b.rotation;
            shell.placed = true;
        }
    }

    let goal = if target.is_some() { 1.0 } else { 0.0 };
    // Quick to light up, slower to let go.
    let rate = if goal > shell.strength { 16.0 } else { 7.0 };
    let before = shell.strength;
    shell.strength += (goal - shell.strength) * (1.0 - (-dt * rate).exp());
    if (shell.strength - before).abs() > 1e-4 {
        if let Some(m) = materials.get_mut(mat) {
            let s = shell.strength;
            m.base_color = Color::linear_rgb(TINT[0] * s, TINT[1] * s, TINT[2] * s);
        }
    }
    let visible = shell.strength > 0.01;
    let want = if visible {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    if *vis != want {
        *vis = want;
    }
}
