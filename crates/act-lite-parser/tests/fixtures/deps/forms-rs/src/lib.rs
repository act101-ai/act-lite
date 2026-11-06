mod net;
#[path = "other/place.rs"]
mod moved;
cfg_feature! { pub mod gated; }
pub use net::{self, Conn as C};
extern crate alloc;
static S: &str = include_str!("data.txt");
