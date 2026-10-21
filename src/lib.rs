#[cfg(all(test, feature = "desktop"))]
mod acceptance;
#[cfg(feature = "bridge")]
pub mod backend;
pub mod cli;
pub mod common;
#[cfg(feature = "desktop")]
pub mod conversation;
#[cfg(all(feature = "bridge", target_os = "macos"))]
pub mod credentials;
pub mod databases;
#[cfg(feature = "desktop")]
pub mod desktop;
pub mod integration;
pub mod mcp;
pub mod models;
pub mod projects;
#[cfg(feature = "bridge")]
pub mod provision;
#[cfg(feature = "bridge")]
pub mod remote_fs;
pub mod repository;
pub mod runner;
#[cfg(feature = "bridge")]
pub mod ssh;
#[cfg(feature = "bridge")]
pub mod transfer;
#[cfg(feature = "bridge")]
pub mod update_gate;

pub mod workspace_ops;

#[cfg(feature = "bridge")]
pub mod git_auth;
