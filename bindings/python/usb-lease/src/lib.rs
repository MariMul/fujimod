//! ff80 sessions leased from Python USB sessions.
//!
//! The USB bindings install the borrower when they register, so binding crates
//! such as the remote bindings lease a session without depending on them.
use fujimod_usb::ff80::Session;
use pyo3::{exceptions::PyRuntimeError, prelude::*};
use std::{
    borrow::{Borrow, BorrowMut},
    sync::OnceLock,
};

/// An ff80 session leased from a Python USB session; dropping it ends the lease.
pub struct Lease(Box<dyn BorrowMut<Session>>);
impl Lease {
    pub fn new(lease: impl BorrowMut<Session> + 'static) -> Self {
        Self(Box::new(lease))
    }
}
impl Borrow<Session> for Lease {
    fn borrow(&self) -> &Session {
        (*self.0).borrow()
    }
}
impl BorrowMut<Session> for Lease {
    fn borrow_mut(&mut self) -> &mut Session {
        (*self.0).borrow_mut()
    }
}

/// Leases the open ff80 session of a Python USB session object.
pub type Borrower = fn(&Bound<'_, PyAny>) -> PyResult<Lease>;
static BORROWER: OnceLock<Borrower> = OnceLock::new();

/// Install the USB bindings' borrower; later installations are ignored.
pub fn install(borrower: Borrower) {
    let _ = BORROWER.set(borrower);
}

/// Lease the open ff80 session of a Python USB session object.
pub fn lease(session: &Bound<'_, PyAny>) -> PyResult<Lease> {
    let borrower = BORROWER
        .get()
        .ok_or_else(|| PyRuntimeError::new_err("USB bindings are not registered"))?;
    borrower(session)
}
