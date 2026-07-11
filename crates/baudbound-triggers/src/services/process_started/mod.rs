mod engine;
mod event;
mod service;
mod snapshot;
mod spec;
mod state;
mod worker;

#[cfg(windows)]
mod windows;

pub use service::ProcessStartedService;
#[cfg(test)]
pub(crate) use spec::{ProcessMatchMode, ProcessStartedSpec};

#[cfg(test)]
mod tests;
