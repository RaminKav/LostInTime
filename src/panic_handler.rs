//! A wrapper for panics using Bevy's plugin system.
//!
//! On supported platforms (windows, macos, linux) will produce a popup using the `msgbox` crate in addition to writing via `log::error!`, or if `bevy::log::LogPlugin` is not enabled, `stderr`.

#![warn(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::sync::Arc;

use bevy::prelude::*;

pub trait PanicHandleFn<Res>: Fn(&std::panic::PanicInfo) -> Res + Send + Sync + 'static {}
impl<Res, T: Fn(&std::panic::PanicInfo) -> Res + Send + Sync + 'static> PanicHandleFn<Res> for T {}

#[derive(Default)]
pub struct PanicHandlerBuilder {
    custom_name: Option<Arc<dyn PanicHandleFn<String>>>,
    custom_body: Option<Arc<dyn PanicHandleFn<String>>>,
    custom_hook: Option<Arc<dyn PanicHandleFn<()>>>,
}
impl PanicHandlerBuilder {
    #[must_use]
    /// Builds the `PanicHandler`
    pub fn build(self) -> PanicHandler {
        PanicHandler {
            custom_title: {
                self.custom_name.unwrap_or_else(|| {
                    Arc::new(|_: &std::panic::PanicInfo| "Fatal Error".to_owned())
                })
            },
            custom_body: {
                self.custom_body.unwrap_or_else(|| {
                    Arc::new(|info| {
                        format!(
                            "Unhandled panic! @ {}:\n{}",
                            info.location()
                                .map_or("Unknown Location".to_owned(), ToString::to_string),
                            info.payload().downcast_ref::<String>().unwrap_or(
                                &((*info.payload().downcast_ref::<&str>().unwrap_or(&"No Info"))
                                    .to_string())
                            )
                        )
                    })
                })
            },
            custom_hook: { self.custom_hook.unwrap_or_else(|| Arc::new(|_| {})) },
        }
    }

    #[must_use]
    /// After the popup is closed, the previously existing panic hook will be called
    pub fn take_call_from_existing(mut self) -> Self {
        self.custom_hook = Some(Arc::new(std::panic::take_hook()));
        self
    }

    #[must_use]
    /// After the popup is closed, this function will be called
    pub fn set_call_func(mut self, call_func: impl PanicHandleFn<()>) -> Self {
        self.custom_hook = Some(Arc::new(call_func));
        self
    }

    #[must_use]
    /// The popup title will be set to the result of this function
    pub fn set_title_func(mut self, title_func: impl PanicHandleFn<String>) -> Self {
        self.custom_name = Some(Arc::new(title_func));
        self
    }

    #[must_use]
    /// The popup body will be set to the result of this function
    pub fn set_body_func(mut self, body_func: impl PanicHandleFn<String>) -> Self {
        self.custom_body = Some(Arc::new(body_func));
        self
    }
}

/// Bevy plugin that opens a popup window on panic & logs an error
#[derive(Clone)]
pub struct PanicHandler {
    pub custom_title: Arc<dyn PanicHandleFn<String>>,
    pub custom_body: Arc<dyn PanicHandleFn<String>>,
    pub custom_hook: Arc<dyn PanicHandleFn<()>>,
}
impl PanicHandler {
    #[must_use]
    #[allow(clippy::new_ret_no_self)]
    /// Create a new builder. The custom hook does nothing.
    pub fn new() -> PanicHandlerBuilder {
        PanicHandlerBuilder::default()
    }
}

impl Plugin for PanicHandler {
    fn build(&self, _: &mut App) {
        let handler = self.clone();
        std::panic::set_hook(Box::new(move |info| {
            let title_string = (handler.custom_title)(info);
            let info_string = (handler.custom_body)(info);

            // Known limitations: Logging in tests prints to stdout immediately.
            // This will print duplicate messages to stdout if the default panic hook is being used & env_logger is initialized.
            bevy::log::error!("{title_string}\n{info_string}");

            // Don't interrupt test execution with a popup, and dont try on unsupported platforms.
            #[cfg(all(
                not(test),
                any(target_os = "windows", target_os = "macos", target_os = "linux")
            ))]
            {
                if let Err(e) = native_dialog::MessageDialog::new()
                    .set_title(&title_string)
                    .set_text(&info_string)
                    .set_type(native_dialog::MessageType::Error)
                    .show_alert()
                {
                    bevy::log::error!("{e}");
                }
            }

            (handler.custom_hook)(info);
        }));
    }
}
