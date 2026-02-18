//! New peeps instrumentation surface.
//!
//! Top-level split:
//! - `enabled`: real diagnostics runtime
//! - `disabled`: zero-cost pass-through API

#[cfg(not(target_arch = "wasm32"))]
pub mod fs;
#[cfg(target_arch = "wasm32")]
pub mod fs {}
pub mod net;

#[doc(hidden)]
pub use facet_value;
#[doc(hidden)]
pub use parking_lot;
#[doc(hidden)]
pub use tokio;

#[cfg(all(feature = "diagnostics", target_arch = "wasm32"))]
compile_error!(
    "`peeps` diagnostics is not supported on wasm32; build wasm targets without `feature=\"diagnostics\"`"
);

#[cfg(not(feature = "diagnostics"))]
mod disabled;
#[cfg(all(feature = "diagnostics", not(target_arch = "wasm32")))]
mod enabled;

#[cfg(not(feature = "diagnostics"))]
pub use disabled::*;
#[cfg(all(feature = "diagnostics", not(target_arch = "wasm32")))]
pub use enabled::*;

#[macro_export]
macro_rules! init {
    () => {{
        $crate::__init_from_macro(env!("CARGO_MANIFEST_DIR"));
    }};
}

#[macro_export]
macro_rules! facade {
    () => {
        pub mod peeps {
            pub const PEEPS_CX: $crate::PeepsContext =
                $crate::PeepsContext::new(env!("CARGO_MANIFEST_DIR"));

            pub trait MutexExt<T> {
                fn lock(&self) -> $crate::MutexGuard<'_, T>;
                fn try_lock(&self) -> Option<$crate::MutexGuard<'_, T>>;
            }

            impl<T> MutexExt<T> for $crate::Mutex<T> {
                #[track_caller]
                fn lock(&self) -> $crate::MutexGuard<'_, T> {
                    self.lock_with_source($crate::Source::caller(), PEEPS_CX)
                }

                #[track_caller]
                fn try_lock(&self) -> Option<$crate::MutexGuard<'_, T>> {
                    self.try_lock_with_source($crate::Source::caller(), PEEPS_CX)
                }
            }

            pub trait RwLockExt<T> {
                fn read(&self) -> $crate::parking_lot::RwLockReadGuard<'_, T>;
                fn write(&self) -> $crate::parking_lot::RwLockWriteGuard<'_, T>;
                fn try_read(&self) -> Option<$crate::parking_lot::RwLockReadGuard<'_, T>>;
                fn try_write(&self) -> Option<$crate::parking_lot::RwLockWriteGuard<'_, T>>;
            }

            impl<T> RwLockExt<T> for $crate::RwLock<T> {
                #[track_caller]
                fn read(&self) -> $crate::parking_lot::RwLockReadGuard<'_, T> {
                    self.read_with_source($crate::Source::caller(), PEEPS_CX)
                }

                #[track_caller]
                fn write(&self) -> $crate::parking_lot::RwLockWriteGuard<'_, T> {
                    self.write_with_source($crate::Source::caller(), PEEPS_CX)
                }

                #[track_caller]
                fn try_read(&self) -> Option<$crate::parking_lot::RwLockReadGuard<'_, T>> {
                    self.try_read_with_source($crate::Source::caller(), PEEPS_CX)
                }

                #[track_caller]
                fn try_write(&self) -> Option<$crate::parking_lot::RwLockWriteGuard<'_, T>> {
                    self.try_write_with_source($crate::Source::caller(), PEEPS_CX)
                }
            }

            pub trait SenderExt<T> {
                fn send(
                    &self,
                    value: T,
                ) -> impl core::future::Future<
                    Output = Result<(), $crate::tokio::sync::mpsc::error::SendError<T>>,
                > + '_;
            }

            impl<T> SenderExt<T> for $crate::Sender<T> {
                #[track_caller]
                fn send(
                    &self,
                    value: T,
                ) -> impl core::future::Future<
                    Output = Result<(), $crate::tokio::sync::mpsc::error::SendError<T>>,
                > + '_ {
                    self.send_with_source(value, $crate::Source::caller(), PEEPS_CX)
                }
            }

            pub trait ReceiverExt<T> {
                fn recv(&mut self) -> impl core::future::Future<Output = Option<T>> + '_;
            }

            impl<T> ReceiverExt<T> for $crate::Receiver<T> {
                #[track_caller]
                fn recv(&mut self) -> impl core::future::Future<Output = Option<T>> + '_ {
                    self.recv_with_source($crate::Source::caller(), PEEPS_CX)
                }
            }

            pub trait UnboundedSenderExt<T> {
                fn send(
                    &self,
                    value: T,
                ) -> Result<(), $crate::tokio::sync::mpsc::error::SendError<T>>;
            }

            impl<T> UnboundedSenderExt<T> for $crate::UnboundedSender<T> {
                #[track_caller]
                fn send(
                    &self,
                    value: T,
                ) -> Result<(), $crate::tokio::sync::mpsc::error::SendError<T>> {
                    self.send_with_source(value, $crate::Source::caller(), PEEPS_CX)
                }
            }

            pub trait UnboundedReceiverExt<T> {
                fn recv(&mut self) -> impl core::future::Future<Output = Option<T>> + '_;
            }

            impl<T> UnboundedReceiverExt<T> for $crate::UnboundedReceiver<T> {
                #[track_caller]
                fn recv(&mut self) -> impl core::future::Future<Output = Option<T>> + '_ {
                    self.recv_with_source($crate::Source::caller(), PEEPS_CX)
                }
            }

            pub trait OneshotSenderExt<T> {
                fn send(self, value: T) -> Result<(), T>;
            }

            impl<T> OneshotSenderExt<T> for $crate::OneshotSender<T> {
                #[track_caller]
                fn send(self, value: T) -> Result<(), T> {
                    self.send_with_source(value, $crate::Source::caller(), PEEPS_CX)
                }
            }

            pub trait OneshotReceiverExt<T> {
                fn recv(
                    self,
                ) -> impl core::future::Future<
                    Output = Result<T, $crate::tokio::sync::oneshot::error::RecvError>,
                >;
            }

            impl<T> OneshotReceiverExt<T> for $crate::OneshotReceiver<T> {
                #[track_caller]
                fn recv(
                    self,
                ) -> impl core::future::Future<
                    Output = Result<T, $crate::tokio::sync::oneshot::error::RecvError>,
                > {
                    self.recv_with_source($crate::Source::caller(), PEEPS_CX)
                }
            }

            pub trait BroadcastSenderExt<T: Clone> {
                fn send(
                    &self,
                    value: T,
                ) -> Result<usize, $crate::tokio::sync::broadcast::error::SendError<T>>;
            }

            impl<T: Clone> BroadcastSenderExt<T> for $crate::BroadcastSender<T> {
                #[track_caller]
                fn send(
                    &self,
                    value: T,
                ) -> Result<usize, $crate::tokio::sync::broadcast::error::SendError<T>> {
                    self.send_with_source(value, $crate::Source::caller(), PEEPS_CX)
                }
            }

            pub trait BroadcastReceiverExt<T: Clone> {
                fn recv(
                    &mut self,
                ) -> impl core::future::Future<
                    Output = Result<T, $crate::tokio::sync::broadcast::error::RecvError>,
                > + '_;
            }

            impl<T: Clone> BroadcastReceiverExt<T> for $crate::BroadcastReceiver<T> {
                #[track_caller]
                fn recv(
                    &mut self,
                ) -> impl core::future::Future<
                    Output = Result<T, $crate::tokio::sync::broadcast::error::RecvError>,
                > + '_ {
                    self.recv_with_source($crate::Source::caller(), PEEPS_CX)
                }
            }

            pub trait WatchSenderExt<T: Clone> {
                fn send(
                    &self,
                    value: T,
                ) -> Result<(), $crate::tokio::sync::watch::error::SendError<T>>;
                fn send_replace(&self, value: T) -> T;
            }

            impl<T: Clone> WatchSenderExt<T> for $crate::WatchSender<T> {
                #[track_caller]
                fn send(
                    &self,
                    value: T,
                ) -> Result<(), $crate::tokio::sync::watch::error::SendError<T>> {
                    self.send_with_source(value, $crate::Source::caller(), PEEPS_CX)
                }

                #[track_caller]
                fn send_replace(&self, value: T) -> T {
                    self.send_replace_with_source(value, $crate::Source::caller(), PEEPS_CX)
                }
            }

            pub trait WatchReceiverExt<T: Clone> {
                fn changed(
                    &mut self,
                ) -> impl core::future::Future<
                    Output = Result<(), $crate::tokio::sync::watch::error::RecvError>,
                > + '_;
                fn borrow(&self) -> $crate::tokio::sync::watch::Ref<'_, T>;
                fn borrow_and_update(&mut self) -> $crate::tokio::sync::watch::Ref<'_, T>;
            }

            impl<T: Clone> WatchReceiverExt<T> for $crate::WatchReceiver<T> {
                #[track_caller]
                fn changed(
                    &mut self,
                ) -> impl core::future::Future<
                    Output = Result<(), $crate::tokio::sync::watch::error::RecvError>,
                > + '_ {
                    self.changed_with_source($crate::Source::caller(), PEEPS_CX)
                }

                #[track_caller]
                fn borrow(&self) -> $crate::tokio::sync::watch::Ref<'_, T> {
                    self.borrow()
                }

                #[track_caller]
                fn borrow_and_update(&mut self) -> $crate::tokio::sync::watch::Ref<'_, T> {
                    self.borrow_and_update()
                }
            }

            pub trait NotifyExt {
                fn notified(&self) -> impl core::future::Future<Output = ()> + '_;
                fn notify_one(&self);
                fn notify_waiters(&self);
            }

            impl NotifyExt for $crate::Notify {
                #[track_caller]
                fn notified(&self) -> impl core::future::Future<Output = ()> + '_ {
                    self.notified_with_source($crate::Source::caller(), PEEPS_CX)
                }

                #[track_caller]
                fn notify_one(&self) {
                    self.notify_one()
                }

                #[track_caller]
                fn notify_waiters(&self) {
                    self.notify_waiters()
                }
            }

            pub trait SemaphoreExt {
                fn acquire(
                    &self,
                ) -> impl core::future::Future<
                    Output = Result<$crate::SemaphorePermit<'_>, $crate::tokio::sync::AcquireError>,
                > + '_;
                fn acquire_many(
                    &self,
                    n: u32,
                ) -> impl core::future::Future<
                    Output = Result<$crate::SemaphorePermit<'_>, $crate::tokio::sync::AcquireError>,
                > + '_;
                fn acquire_owned(
                    &self,
                ) -> impl core::future::Future<
                    Output = Result<$crate::OwnedSemaphorePermit, $crate::tokio::sync::AcquireError>,
                > + '_;
                fn acquire_many_owned(
                    &self,
                    n: u32,
                ) -> impl core::future::Future<
                    Output = Result<$crate::OwnedSemaphorePermit, $crate::tokio::sync::AcquireError>,
                > + '_;
            }

            impl SemaphoreExt for $crate::Semaphore {
                #[track_caller]
                fn acquire(
                    &self,
                ) -> impl core::future::Future<
                    Output = Result<$crate::SemaphorePermit<'_>, $crate::tokio::sync::AcquireError>,
                > + '_ {
                    self.acquire_with_source($crate::Source::caller(), PEEPS_CX)
                }

                #[track_caller]
                fn acquire_many(
                    &self,
                    n: u32,
                ) -> impl core::future::Future<
                    Output = Result<$crate::SemaphorePermit<'_>, $crate::tokio::sync::AcquireError>,
                > + '_ {
                    self.acquire_many_with_source(n, $crate::Source::caller(), PEEPS_CX)
                }

                #[track_caller]
                fn acquire_owned(
                    &self,
                ) -> impl core::future::Future<
                    Output = Result<$crate::OwnedSemaphorePermit, $crate::tokio::sync::AcquireError>,
                > + '_ {
                    self.acquire_owned_with_source($crate::Source::caller(), PEEPS_CX)
                }

                #[track_caller]
                fn acquire_many_owned(
                    &self,
                    n: u32,
                ) -> impl core::future::Future<
                    Output = Result<$crate::OwnedSemaphorePermit, $crate::tokio::sync::AcquireError>,
                > + '_ {
                    self.acquire_many_owned_with_source(n, $crate::Source::caller(), PEEPS_CX)
                }
            }

            pub trait JoinSetExt<T>
            where
                T: Send + 'static,
            {
                fn spawn<F>(&mut self, label: &'static str, future: F)
                where
                    F: core::future::Future<Output = T> + Send + 'static;

                fn join_next(
                    &mut self,
                ) -> impl core::future::Future<
                    Output = Option<Result<T, $crate::tokio::task::JoinError>>,
                > + '_;
            }

            impl<T> JoinSetExt<T> for $crate::JoinSet<T>
            where
                T: Send + 'static,
            {
                #[track_caller]
                fn spawn<F>(&mut self, label: &'static str, future: F)
                where
                    F: core::future::Future<Output = T> + Send + 'static,
                {
                    self.spawn_with_source(label, future, $crate::Source::caller(), PEEPS_CX)
                }

                #[track_caller]
                fn join_next(
                    &mut self,
                ) -> impl core::future::Future<
                    Output = Option<Result<T, $crate::tokio::task::JoinError>>,
                > + '_ {
                    self.join_next_with_source($crate::Source::caller(), PEEPS_CX)
                }
            }

            pub mod prelude {
                pub use super::BroadcastReceiverExt;
                pub use super::BroadcastSenderExt;
                pub use super::JoinSetExt;
                pub use super::MutexExt;
                pub use super::NotifyExt;
                pub use super::OneshotReceiverExt;
                pub use super::OneshotSenderExt;
                pub use super::ReceiverExt;
                pub use super::RwLockExt;
                pub use super::SemaphoreExt;
                pub use super::SenderExt;
                pub use super::UnboundedReceiverExt;
                pub use super::UnboundedSenderExt;
                pub use super::WatchReceiverExt;
                pub use super::WatchSenderExt;
            }
        }
    };
}
