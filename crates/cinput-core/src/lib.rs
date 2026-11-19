//! Lõi bộ gõ tiếng Việt C_Input — không phụ thuộc hệ điều hành.
//!
//! - [`engine::Engine`]: nhận từng phím, trả về phép sửa văn bản.
//! - [`method::InputMethod`]: Telex, VNI, Telex đơn giản.
//! - [`spell`]: cấu trúc âm tiết, kiểm tra chính tả, quy tắc đặt dấu.
//! - [`macros::MacroTable`]: gõ tắt.

pub mod engine;
pub mod english;
pub mod macros;
pub mod method;
pub mod spell;
pub mod types;

pub use engine::{Edit, Engine, KeyOutcome, Options, transform};
pub use english::EnglishWords;
pub use macros::MacroTable;
pub use method::InputMethod;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod tests_syllables;
