//! 上游 spike/local.rs 的宿主模块：提供它期望的 super::{Result,fail}。
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
pub fn fail(text: &str) -> Box<dyn std::error::Error> { text.to_owned().into() }
#[path="../../spike/local.rs"]
pub mod local;
