//! UI crate: semantic widgets + Geist skin + Observer-based interactions.

use bevy_embedded_assets::{EmbeddedAssetPlugin, PluginMode};

mod components;
mod events;
mod geist_ui;
mod theme;
pub mod ui;

pub use components::{
    UiButtonStyleOverride, UiButtonVariant, UiFocus, UiModalOverlay, UiProgress, UiResponsiveFlex,
    UiTextInput, UiTextInputValueText,
};
pub use events::{UiCancel, UiClick, UiSubmit};
pub use geist_ui::{CruftUiPlugin, GeistGridMaterial};
pub use theme::{UiFontResources, UiTheme};

pub struct CruftUiAssetsPlugin;

impl bevy::prelude::Plugin for CruftUiAssetsPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        app.add_plugins(EmbeddedAssetPlugin {
            mode: PluginMode::ReplaceDefault,
        });
    }
}

pub mod prelude {
    pub use crate::components::{
        UiButtonStyleOverride, UiButtonVariant, UiFocus, UiModalOverlay, UiProgress,
        UiResponsiveFlex, UiTextInput, UiTextInputValueText,
    };
    pub use crate::events::{UiCancel, UiClick, UiSubmit};
    pub use crate::geist_ui::{CruftUiPlugin, GeistGridMaterial};
    pub use crate::theme::{UiFontResources, UiTheme};
    pub use crate::ui;
    pub use crate::CruftUiAssetsPlugin;
}
