use core::ops::{Deref, DerefMut};

#[repr(transparent)]
pub struct SyncHack<T: ?Sized>(pub T);

unsafe impl<T: ?Sized> Sync for SyncHack<T> {}

impl<T: ?Sized> Deref for SyncHack<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: ?Sized> DerefMut for SyncHack<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
