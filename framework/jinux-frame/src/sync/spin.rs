use core::cell::UnsafeCell;
use core::fmt;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicBool, Ordering};

use crate::trap::disable_local;
use crate::trap::DisabledLocalIrqGuard;

/// A spin lock.
pub struct SpinLock<T> {
    val: UnsafeCell<T>,
    lock: AtomicBool,
}

impl<T> SpinLock<T> {
    /// Creates a new spin lock.
    pub const fn new(val: T) -> Self {
        Self {
            val: UnsafeCell::new(val),
            lock: AtomicBool::new(false),
        }
    }

    /// Acquire the spin lock.
    ///
    /// This method runs in a busy loop until the lock can be acquired.
    /// After acquiring the spin lock, all interrupts are disabled.
    pub fn lock(&self) -> SpinLockGuard<T> {
        // FIXME: add disable_preemption
        let guard = disable_local();
        self.acquire_lock();
        SpinLockGuard {
            lock: &self,
            irq_guard: guard,
        }
    }

    /// Try Acquire the spin lock immedidately.
    pub fn try_lock(&self) -> Option<SpinLockGuard<T>> {
        // FIXME: add disable_preemption
        let irq_guard = disable_local();
        if self.try_acquire_lock() {
            let lock_guard = SpinLockGuard {
                lock: &self,
                irq_guard,
            };
            return Some(lock_guard);
        }
        return None;
    }

    /// Access the spin lock, otherwise busy waiting
    fn acquire_lock(&self) {
        while !self.try_acquire_lock() {
            core::hint::spin_loop();
        }
    }

    fn try_acquire_lock(&self) -> bool {
        self.lock
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    fn release_lock(&self) {
        self.lock.store(false, Ordering::SeqCst);
    }
}

impl<T: fmt::Debug> fmt::Debug for SpinLock<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self.val, f)
    }
}

// Safety. Only a single lock holder is permitted to access the inner data of Spinlock.
unsafe impl<T: Send> Send for SpinLock<T> {}
unsafe impl<T: Send> Sync for SpinLock<T> {}

/// The guard of a spin lock.
pub struct SpinLockGuard<'a, T> {
    lock: &'a SpinLock<T>,
    irq_guard: DisabledLocalIrqGuard,
}

impl<'a, T> Deref for SpinLockGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &mut *self.lock.val.get() }
    }
}

impl<'a, T> DerefMut for SpinLockGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.lock.val.get() }
    }
}

impl<'a, T> Drop for SpinLockGuard<'a, T> {
    fn drop(&mut self) {
        self.lock.release_lock();
    }
}

impl<'a, T: fmt::Debug> fmt::Debug for SpinLockGuard<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<'a, T> !Send for SpinLockGuard<'a, T> {}

// Safety. SpinLockGuard can be shared between tasks/threads in same CPU.
// As SpinLock disables interrupts to prevent race conditions caused by interrupts.
unsafe impl<T: Sync> Sync for SpinLockGuard<'_, T> {}