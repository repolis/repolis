use bevy::prelude::*;
use bevy::pbr::ScreenSpaceAmbientOcclusionSettings;
fn check(mut commands: Commands) {
    commands.spawn(ScreenSpaceAmbientOcclusionSettings::default());
}
