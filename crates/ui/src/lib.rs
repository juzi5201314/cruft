//! UI crate: semantic widgets + Geist skin + Observer-based interactions.

mod components;
mod events;
mod geist_ui;
mod theme;
pub mod ui;

pub use components::{UiButtonStyleOverride, UiButtonVariant, UiProgress, UiResponsiveFlex};
pub use events::UiClick;
pub use geist_ui::{CruftUiPlugin, GeistGridMaterial};
pub use theme::UiTheme;

pub mod prelude {
    pub use crate::components::{
        UiButtonStyleOverride, UiButtonVariant, UiProgress, UiResponsiveFlex,
    };
    pub use crate::events::UiClick;
    pub use crate::geist_ui::{CruftUiPlugin, GeistGridMaterial};
    pub use crate::theme::UiTheme;
    pub use crate::ui;
}
