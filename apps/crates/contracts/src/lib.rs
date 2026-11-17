//! Pixbot 跨 crate 的中立 DTO、端口 trait、事件协议、错误契约与纯函数。
//!
//! 本 crate 不包含运行时实现、网络 transport 或持久化适配器。

pub mod config;
pub mod error;
pub mod event;
pub mod message;
pub mod model;
pub mod ports;
pub mod request;
pub mod tool;
pub mod transcript;
pub mod usage;
