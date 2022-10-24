#![feature(result_flattening)]
#![feature(entry_insert)]
#![feature(assert_matches)]
#![feature(try_find)]
#![feature(box_syntax)]
#![feature(async_closure)]

pub use config::Config;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub mod commands;
pub mod config;
pub mod core;
pub mod data_model;
pub mod errors;
pub mod ops;
pub mod spec;
pub mod status_calculator;
pub mod stream;
pub mod stream_db;
pub mod ui_ops;
pub mod util;
pub mod yb_conf;
pub mod yb_env;
pub mod yb_options;
