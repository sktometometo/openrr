#![doc = include_str!("../README.md")]
#![warn(missing_debug_implementations, missing_docs, rust_2018_idioms)]
// This lint is unable to correctly determine if an atomic is sufficient to replace the mutex use.
// https://github.com/rust-lang/rust-clippy/issues/4295
#![allow(clippy::mutex_atomic)]

mod proxy;

#[path = "gen/api.rs"]
mod api;

use std::{fmt, path::Path, sync::Arc, time::Duration};

use abi_stable::{erased_types::TD_Opaque, library::lib_header_from_path, StableAbi};
use arci::{async_trait, WaitFuture};

pub use crate::api::*;
// This is not a public API. Use export_plugin! macro for plugin exporting.
#[doc(hidden)]
pub use crate::proxy::PluginMod_Ref;
use crate::proxy::{GamepadTraitObject, JointTrajectoryClientTraitObject};

/// Exports the plugin that will instantiated with the specified expression.
///
/// # Examples
///
/// ```
/// use openrr_plugin::Plugin;
///
/// openrr_plugin::export_plugin!(MyPlugin);
///
/// pub struct MyPlugin;
///
/// impl Plugin for MyPlugin {
///     fn name(&self) -> String {
///         "MyPlugin".into()
///     }
/// }
/// ```
#[macro_export]
macro_rules! export_plugin {
    ($plugin_constructor:expr $(,)?) => {
        /// Exports the root module of this library.
        ///
        /// This code isn't run until the layout of the type it returns is checked.
        #[::abi_stable::export_root_module]
        pub fn instantiate_root_module() -> $crate::PluginMod_Ref {
            $crate::PluginMod_Ref::new(plugin_constructor)
        }

        /// Instantiates the plugin.
        #[::abi_stable::sabi_extern_fn]
        pub fn plugin_constructor() -> $crate::PluginProxy {
            $crate::PluginProxy::new($plugin_constructor)
        }
    };
}

impl PluginProxy {
    /// Loads a plugin from the specified path.
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, arci::Error> {
        let path = path.as_ref();

        let header = lib_header_from_path(path).map_err(anyhow::Error::from)?;
        let root_module = header
            .init_root_module::<PluginMod_Ref>()
            .map_err(anyhow::Error::from)?;

        let plugin_constructor = root_module.plugin_constructor();
        let plugin = plugin_constructor();

        Ok(plugin)
    }

    /// Returns the name of this plugin.
    ///
    /// NOTE: This is *not* a unique identifier.
    pub fn name(&self) -> String {
        self.0.name().into()
    }
}

impl fmt::Debug for PluginProxy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PluginProxy")
            .field("name", &self.name())
            .finish()
    }
}

// =============================================================================
// JointTrajectoryClient

/// FFI-safe equivalent of [`Box<dyn arci::JointTrajectoryClient>`](arci::JointTrajectoryClient).
#[repr(C)]
#[derive(StableAbi)]
pub struct JointTrajectoryClientProxy(JointTrajectoryClientTraitObject);

impl JointTrajectoryClientProxy {
    /// Creates a new `JointTrajectoryClientProxy`.
    pub fn new<T>(client: T) -> Self
    where
        T: arci::JointTrajectoryClient + 'static,
    {
        Self(JointTrajectoryClientTraitObject::from_value(
            client, TD_Opaque,
        ))
    }
}

impl arci::JointTrajectoryClient for JointTrajectoryClientProxy {
    fn joint_names(&self) -> Vec<String> {
        self.0.joint_names().into_iter().map(|s| s.into()).collect()
    }

    fn current_joint_positions(&self) -> Result<Vec<f64>, arci::Error> {
        Ok(self
            .0
            .current_joint_positions()
            .into_result()?
            .into_iter()
            .map(f64::from)
            .collect())
    }

    fn send_joint_positions(
        &self,
        positions: Vec<f64>,
        duration: Duration,
    ) -> Result<WaitFuture, arci::Error> {
        Ok(self
            .0
            .send_joint_positions(
                positions.into_iter().map(Into::into).collect(),
                duration.into(),
            )
            .into_result()?
            .into())
    }

    fn send_joint_trajectory(
        &self,
        trajectory: Vec<arci::TrajectoryPoint>,
    ) -> Result<WaitFuture, arci::Error> {
        Ok(self
            .0
            .send_joint_trajectory(trajectory.into_iter().map(Into::into).collect())
            .into_result()?
            .into())
    }
}

impl fmt::Debug for JointTrajectoryClientProxy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("JointTrajectoryClientProxy").finish()
    }
}

// =============================================================================
// arci::Gamepad

/// FFI-safe equivalent of [`Box<dyn arci::Gamepad>`](arci::Gamepad).
// Don't implement Clone -- use of Arc is implementation detail.
#[repr(C)]
#[derive(StableAbi)]
pub struct GamepadProxy(GamepadTraitObject);

impl GamepadProxy {
    /// Creates a new `GamepadProxy`.
    pub fn new<T>(gamepad: T) -> Self
    where
        T: arci::Gamepad + 'static,
    {
        Self(GamepadTraitObject::from_value(Arc::new(gamepad), TD_Opaque))
    }
}

#[async_trait]
impl arci::Gamepad for GamepadProxy {
    async fn next_event(&self) -> arci::gamepad::GamepadEvent {
        let this = Self(self.0.clone());
        tokio::task::spawn_blocking(move || this.0.next_event().into())
            .await
            .unwrap_or(arci::gamepad::GamepadEvent::Unknown)
    }

    fn stop(&self) {
        self.0.stop();
    }
}

impl fmt::Debug for GamepadProxy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GamepadProxy").finish()
    }
}
