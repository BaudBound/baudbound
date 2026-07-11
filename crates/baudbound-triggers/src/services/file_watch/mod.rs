mod event;
mod service;
mod spec;

#[cfg(test)]
pub(crate) use event::file_watch_event;
pub use service::FileWatchService;
#[cfg(test)]
pub(crate) use spec::FileWatchSpec;

#[cfg(test)]
mod tests;
