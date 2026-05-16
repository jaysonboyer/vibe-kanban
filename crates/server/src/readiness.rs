//! Process-global readiness state, observable via the `/api/health` route.
//!
//! `AtomicU8`-backed for lock-free reads from the health handler. The startup
//! orchestrator calls `Phase::*::set()` as it progresses; the handler reads
//! via `Phase::current()`. Maps to D-05 / VKSTART-11 and CONCERNS.md
//! "No Health Check Endpoint".

use std::sync::atomic::{AtomicU8, Ordering};

use serde::Serialize;
use ts_rs::TS;

pub static READINESS: AtomicU8 = AtomicU8::new(Phase::Starting as u8);

#[repr(u8)]
#[derive(Debug, Clone, Copy, Serialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum Phase {
    Starting = 0,
    MigratingDb = 1,
    InitializingServices = 2,
    Ready = 10,
}

impl Phase {
    pub fn current() -> Self {
        match READINESS.load(Ordering::Acquire) {
            10 => Phase::Ready,
            2 => Phase::InitializingServices,
            1 => Phase::MigratingDb,
            _ => Phase::Starting,
        }
    }

    pub fn set(self) {
        READINESS.store(self as u8, Ordering::Release);
    }
}
