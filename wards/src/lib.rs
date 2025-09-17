//! Policy/tenancy engine (stub for Milestone 0)

pub mod approvals;
pub mod config;
pub mod policy;
pub mod schedule;

#[derive(Default)]
pub struct Wards;
impl Wards {
    pub fn new() -> Self {
        Self
    }
}
