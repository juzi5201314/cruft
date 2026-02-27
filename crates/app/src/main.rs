use bevy::prelude::*;

mod plugins;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(ImagePlugin::default_nearest()))
        .add_plugins(plugins::ProceduralTexturePlugin)
        .run();
}
