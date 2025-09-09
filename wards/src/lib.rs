//! Policy/tenancy engine (stub for Milestone 0)

pub mod approvals;
pub mod config;
pub mod policy;

#[derive(Default)]
pub struct Wards;
impl Wards {
    pub fn new() -> Self {
        Self
    }
}
