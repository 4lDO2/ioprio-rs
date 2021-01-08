use std::cmp::Ordering;
use std::convert::TryInto;

use nix::errno::Errno;
use nix::unistd::{Pid, Uid};

pub struct Priority {
    inner: u16,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Target {
    Process(Pid),
    ProcessGroup(Pid),
    User(Uid),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Class {
    Realtime(RtPriorityLevel),
    BestEffort(BePriorityLevel),
    Idle,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RtPriorityLevel {
    inner: u8,
}
impl RtPriorityLevel {
    pub const fn highest() -> Self {
        Self {
            inner: 0,
        }
    }
    pub const fn lowest() -> Self {
        Self {
            inner: 7,
        }
    }
    pub const fn from_level(level: u8) -> Option<Self> {
        if level < 8 {
            Some(Self { inner: level })
        } else {
            None
        }
    }
    pub const fn level(self) -> u8 {
        self.inner
    }
    fn data(self) -> u16 {
        self.inner.into()
    }
}
impl Ord for RtPriorityLevel {
    fn cmp(&self, other: &Self) -> Ordering {
        Ord::cmp(&self.data(), &other.data())
            .reverse()
    }
}
impl PartialOrd for RtPriorityLevel {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(Ord::cmp(self, other))
    }
}
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct BePriorityLevel {
    inner: u8,
}
impl BePriorityLevel {
    pub const fn highest() -> Self {
        Self {
            inner: 0,
        }
    }
    pub const fn fallback() -> Self {
        Self {
            inner: 4,
        }
    }
    pub const fn lowest() -> Self {
        Self {
            inner: 7,
        }
    }
    pub const fn from_level(level: u8) -> Option<Self> {
        if level < 8 {
            Some(Self { inner: level })
        } else {
            None
        }
    }
    pub const fn level(self) -> u8 {
        self.inner
    }
    fn data(self) -> u16 {
        self.inner.into()
    }
}
impl Ord for BePriorityLevel {
    fn cmp(&self, other: &Self) -> Ordering {
        Ord::cmp(&self.data(), &other.data())
            .reverse()
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
        Ord::cmp(&self.rel_priority(), &other.rel_priority())
            .then_with(|| match (self, other) {
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
    pub fn new(class: Class) -> Self {
        Self {
            inner: ((class.kind() as u16) << 13) | class.data(),
        }
    }
    pub fn class(self) -> Option<Class> {
        let class_raw = self.inner >> 13;
        let data = self.inner & 0x1FFF;

        Some(match class_raw {
            1 => Class::Realtime(RtPriorityLevel { inner: data.try_into().ok()? }),
            2 => Class::BestEffort(BePriorityLevel { inner: data.try_into().ok()? }),
            3 => Class::Idle,

            _ => return None,
        })
    }
    pub const fn standard() -> Self {
        Self { inner: 0 }
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

pub fn get_priority(target: Target) -> nix::Result<Priority> {
    let [which, who] = target_which_who(target);

    let res = unsafe {
        libc::syscall(libc::SYS_ioprio_get, which, who)
    };

    Errno::result(res).map(|mask| Priority { inner: mask as u16 })
}
pub fn set_priority(target: Target, priority: Priority) -> nix::Result<()> {
    let [which, who] = target_which_who(target);

    let res = unsafe {
        libc::syscall(libc::SYS_ioprio_set, which, who, priority.inner as libc::c_int)
    };

    Errno::result(res).map(|_| ())
}
