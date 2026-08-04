//! Panic hook plugin: log the panic, then hard-abort the process.
//!
//! We intentionally do **not** show a native modal dialog here. On macOS, AppKit alerts
//! must run on the main thread; showing one from a panicking compute-pool thread (common
//! for Bevy system-param failures) can wedge the process so hard Force Quit fails.
//! Waiting on that dialog before abort made the window sit in "Not Responding".

#![warn(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::sync::Arc;

use bevy::prelude::*;

pub trait PanicHandleFn<Res>:
    Fn(&std::panic::PanicHookInfo) -> Res + Send + Sync + 'static
{
}
impl<Res, T: Fn(&std::panic::PanicHookInfo) -> Res + Send + Sync + 'static> PanicHandleFn<Res>
    for T
{
}

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
                    Arc::new(|_: &std::panic::PanicHookInfo| "Fatal Error".to_owned())
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
    /// After logging, the previously existing panic hook will be called (then we abort).
    pub fn take_call_from_existing(mut self) -> Self {
        self.custom_hook = Some(Arc::new(std::panic::take_hook()));
        self
    }

    #[must_use]
    /// After logging, this function will be called (then we abort).
    pub fn set_call_func(mut self, call_func: impl PanicHandleFn<()>) -> Self {
        self.custom_hook = Some(Arc::new(call_func));
        self
    }

    #[must_use]
    /// The log title will be set to the result of this function
    pub fn set_title_func(mut self, title_func: impl PanicHandleFn<String>) -> Self {
        self.custom_name = Some(Arc::new(title_func));
        self
    }

    #[must_use]
    /// The log body will be set to the result of this function
    pub fn set_body_func(mut self, body_func: impl PanicHandleFn<String>) -> Self {
        self.custom_body = Some(Arc::new(body_func));
        self
    }
}

/// Bevy plugin that logs panics and aborts the process.
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
        // Do not chain the previous hook — some default hooks try to unwind or touch the
        // runtime after a pooled-thread panic and can deadlock with winit/Metal.
        let _previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let title_string = (handler.custom_title)(info);
            let info_string = (handler.custom_body)(info);

            bevy::log::error!("{title_string}\n{info_string}");
            eprintln!("{title_string}\n{info_string}");
            eprintln!("(aborting process — no modal dialog; see log above)");

            (handler.custom_hook)(info);

            // Never return into a half-dead Bevy/winit frame.
            std::process::abort();
        }));
    }
}
