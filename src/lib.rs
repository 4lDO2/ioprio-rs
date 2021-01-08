//! This is a simple crate for managing Linux-specific I/O priorities, which are usable with
//! interfaces such as `io_uring`, Linux AIO, and can also be set globally for a single process or
//! group.
//!
//! Note that these priorities are Linux-specific, and the actual interpretation of what these
//! 16-bit masks is only defined in `linux/ioprio.h` and `linux/Documentation/block/ioprio.rst`,
//! which is only available in the Linux source, despite the syscalls having documentation as man
//! pages. This library is based on Linux 5.10 interface and documentation, although the interface
//! has not changed much whatsoever since it was introduced in Linux 2.6.13.
//!
//! Also, setting I/O priorities only has an effect when the Completely Fair I/O Scheduler is in
//! use, which is the default I/O scheduler.
//!
//! Refer to the _ioprio_set(2)_ syscall man page for more information about these API:s.
#![deny(missing_docs)]
use std::cmp::Ordering;
use std::convert::TryInto;

use nix::errno::Errno;
use nix::unistd::{Pid, Uid};

/// An I/O priority, either associated with a class and per-class data, or the standard priority.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Priority {
    inner: u16,
}
impl PartialOrd for Priority {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(Ord::cmp(&self.class()?, &other.class()?))
    }
}

/// A target, consisting of one or more processes matching the given query.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Target {
    /// A single process. Note that a PID value of zero refers to the calling process.
    /// (`IOPRIO_WHO_PROCESS`.)
    Process(Pid),
    /// A process group. As with single processes, setting this to zero refers to the process group
    /// that the current process belongs to. (`IOPRIO_WHO_PGRP`.)
    ProcessGroup(Pid),
    /// All processes owned by a user. (`IOPRIO_WHO_USER`.)
    User(Uid),
}

/// A priority class, being either real-time (`IOPRIO_CLASS_RT`), best-effort (`IOPRIO_CLASS_BE`),
/// or idle (`IOPRIO_CLASS_IDLE`).
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Class {
    /// The real-time class (`IOPRIO_CLASS_RT`), requiring elevated privileges to set to.
    Realtime(RtPriorityLevel),
    /// The best-effort class (`IOPRIO_CLASS_BE`), which is the default class.
    BestEffort(BePriorityLevel),
    /// The Idle class (`IOPRIO_CLASS_IDLE`).
    ///
    /// All I/O done with this priority, will _only_ be scheduled when the resource accessed, is
    /// completely idle. This is the lowest possible priority, and does not require any capability
    /// to set (with the exception of kernels before 2.6.25).
    Idle,
}

/// Real-time I/O priority levels, ranging from the numerical values 0-7, but reversed.
///
/// That is, zero is the highest priority level, while 7 is the lowest priority level. This
/// priority class associates these levels with how long timeslices the I/O scheduler will grant to
/// the I/O, rather than bandwidth (which applies to [`BestEffort`](Class::BestEffort)).
///
/// Note that this class can easily starve the entire system I/O, and thus requires `CAP_SYS_ADMIN`
/// to set.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RtPriorityLevel {
    inner: u8,
}
impl RtPriorityLevel {
    /// The highest real-time priority level, 0.
    pub const fn highest() -> Self {
        Self { inner: 0 }
    }
    /// The lowest real-time priority level, 7.
    pub const fn lowest() -> Self {
        Self { inner: 7 }
    }
    /// Wrap an underlying level.
    ///
    /// If the inner value is greater than 7, then [`None`] is returned.
    pub const fn from_level(level: u8) -> Option<Self> {
        if level < 8 {
            Some(Self { inner: level })
        } else {
            None
        }
    }
    /// Get the underlying level, ranging from 0 to 7.
    pub const fn level(self) -> u8 {
        self.inner
    }
    fn data(self) -> u16 {
        self.inner.into()
    }
}
impl Ord for RtPriorityLevel {
    fn cmp(&self, other: &Self) -> Ordering {
        Ord::cmp(&self.data(), &other.data()).reverse()
    }
}
impl PartialOrd for RtPriorityLevel {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(Ord::cmp(self, other))
    }
}
/// I/O priority levels of the best-effort scheduling class, which range from 0-7, reversed.
///
/// The highest level is 0, while the lowest level is 7.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct BePriorityLevel {
    inner: u8,
}
impl BePriorityLevel {
    /// The highest level, 0.
    pub const fn highest() -> Self {
        Self { inner: 0 }
    }
    /// The fallback level, 4.
    pub const fn fallback() -> Self {
        Self { inner: 4 }
    }
    /// The lowest level, 7.
    pub const fn lowest() -> Self {
        Self { inner: 7 }
    }
    /// Wrap an underlying level, returning [`None`] if it exceeds 7.
    pub const fn from_level(level: u8) -> Option<Self> {
        if level < 8 {
            Some(Self { inner: level })
        } else {
            None
        }
    }
    /// Get the underlying level, ranging from 0 to 7.
    pub const fn level(self) -> u8 {
        self.inner
    }
    fn data(self) -> u16 {
        self.inner.into()
    }
}
impl Ord for BePriorityLevel {
    fn cmp(&self, other: &Self) -> Ordering {
        Ord::cmp(&self.data(), &other.data()).reverse()
    }
}
impl PartialOrd for BePriorityLevel {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(Ord::cmp(self, other))
    }
}

impl Class {
    fn rel_priority(self) -> u8 {
        match self {
            Self::Realtime(_) => 2,
            Self::BestEffort(_) => 1,
            Self::Idle => 0,
        }
    }
    fn kind(self) -> u16 {
        match self {
            Self::Realtime(_) => 1,
            Self::BestEffort(_) => 2,
            Self::Idle => 3,
        }
    }
    fn data(self) -> u16 {
        match self {
            Self::Realtime(rt) => rt.data(),
            Self::BestEffort(be) => be.data(),
            Self::Idle => 0,
        }
    }
}
impl Ord for Class {
    fn cmp(&self, other: &Self) -> Ordering {
        Ord::cmp(&self.rel_priority(), &other.rel_priority()).then_with(|| match (self, other) {
            (Self::Realtime(lhs), Self::Realtime(rhs)) => Ord::cmp(&lhs, &rhs),
            (Self::BestEffort(lhs), Self::BestEffort(rhs)) => Ord::cmp(&lhs, &rhs),
            (Self::Idle, Self::Idle) => Ordering::Equal,

            _ => unreachable!(),
        })
    }
}
impl PartialOrd for Class {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(Ord::cmp(self, other))
    }
}

impl Priority {
    /// Construct a new I/O priority value, from the priority class and per-class level.
    pub fn new(class: Class) -> Self {
        Self {
            inner: ((class.kind() as u16) << 13) | class.data(),
        }
    }
    /// Retrieve the class, if any such class was set.
    pub fn class(self) -> Option<Class> {
        let class_raw = self.inner >> 13;
        let data = self.inner & 0x1FFF;

        Some(match class_raw {
            1 => Class::Realtime(RtPriorityLevel::from_level(data.try_into().ok()?)?),
            2 => Class::BestEffort(BePriorityLevel::from_level(data.try_into().ok()?)?),
            3 => Class::Idle,

            _ => return None,
        })
    }
    /// Get the value for the default priority, with the inner value of zero.
    pub const fn standard() -> Self {
        Self { inner: 0 }
    }
    /// Get the inner I/O priority mask, which can be set in several interfaces, including
    /// `io_uring`, `AIO`, and the regular `ioprio_*` syscalls.
    pub const fn inner(self) -> u16 {
        self.inner
    }
    /// Construct an I/O priority from the inner value.
    ///
    /// Note that it is up to the caller to ensure the validity of the mask.
    pub const fn from_inner(inner: u16) -> Self {
        Self { inner }
    }
}
impl Default for Priority {
    fn default() -> Self {
        Self::standard()
    }
}

fn target_which_who(target: Target) -> [libc::c_int; 2] {
    match target {
        Target::Process(pid) => [1, pid.as_raw() as libc::c_int],
        Target::ProcessGroup(pgid) => [2, pgid.as_raw() as libc::c_int],
        Target::User(uid) => [3, uid.as_raw() as libc::c_int],
    }
}

/// Get the I/O priority of the processes of the given target.
///
/// If there are multiple processes, each with different priorities, then the highest priority of
/// them will be returned.
///
/// Refer to _ioprio_get(2)_ for further information.
pub fn get_priority(target: Target) -> nix::Result<Priority> {
    let [which, who] = target_which_who(target);

    let res = unsafe { libc::syscall(libc::SYS_ioprio_get, which, who) };

    Errno::result(res).map(|mask| Priority { inner: mask as u16 })
}
/// Set the I/O priority of the processes of the given target.
///
/// Note that increasing the priority class to real-time, will require elevated privileges (namely
/// `CAP_SYS_ADMIN`). Additionally, this process must also have the permissions to modify the
/// target process or group, or have `CAP_SYS_NICE`.
///
/// Refer to _ioprio_set(2)_ for further information.
pub fn set_priority(target: Target, priority: Priority) -> nix::Result<()> {
    let [which, who] = target_which_who(target);

    let res = unsafe {
        libc::syscall(
            libc::SYS_ioprio_set,
            which,
            who,
            priority.inner as libc::c_int,
        )
    };

    Errno::result(res).map(|_| ())
}

#[cfg(any(doc, feature = "iou"))]
mod sqe_ext {
    use super::*;

    mod private {
        pub trait Sealed {}
    }
    impl private::Sealed for iou_::SQE<'_> {}

    /// An extension trait for [`iou::SQE`](`iou_::SQE`), that allows retrieving and setting the
    /// I/O priority of each individual I/O event.
    pub trait SqeExt {
        /// Get the current priority stored in the SQE.
        fn priority(&self) -> Priority;
        /// Set the priority of the SQE, pertaining only to this particular I/O event.
        fn set_priority(&mut self, priority: Priority);
    }
    impl SqeExt for iou_::SQE<'_> {
        fn priority(&self) -> Priority {
            Priority {
                inner: self.raw().ioprio,
            }
        }
        fn set_priority(&mut self, priority: Priority) {
            unsafe {
                self.raw_mut().ioprio = priority.inner;
            }
        }
    }
}
#[cfg(any(doc, feature = "iou"))]
pub use sqe_ext::SqeExt;
