//! 几乎每个模块都会用到的少量通用项。只放错误处理与 JSON 基础类型；
//! crate 内部的函数和类型一律在使用处显式 `use`，以保持依赖方向可见。
pub(crate) use anyhow::{Context, Result, bail};
pub(crate) use serde_json::{Value, json};
