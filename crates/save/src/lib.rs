//! 存档系统：索引扫描、管理操作（new/copy/rename/delete）与最小加载任务。

mod io;
pub mod api;
pub mod types;

pub use api::{
    CurrentSave, SaveIndex, SaveIndexReady, SaveLoadRequest, SaveLoadResult, SaveOpRequest,
    SaveOpResult, SavePlugin,
};
pub use types::{LoadedSave, SaveId, SaveMeta, SaveRootDir};
