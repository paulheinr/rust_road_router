use super::*;

mod piecewise_linear_function;
use self::piecewise_linear_function::*;

mod geometry;
use self::geometry::*;
pub use self::geometry::TTFPoint;

mod graph;
pub use self::graph::Graph as TDGraph;

mod shortcut;
pub use self::shortcut::*;

mod shortcut_source;
use self::shortcut_source::*;

pub mod shortcut_graph;
pub use self::shortcut_graph::ShortcutGraph;
pub use self::shortcut_graph::PartialShortcutGraph;
pub use self::shortcut_graph::CustomizedGraph;
pub use self::shortcut_graph::SingleDirBoundsGraph;

#[allow(clippy::float_cmp)]
mod time {
    use std::{
        f64::NAN,
        ops::{Add, Sub, Mul, Div},
        cmp::Ordering,
        borrow::Borrow,
    };

    // TODO switch to something ULP based?
    // implications for division with EPSILON like divisors?
    pub const EPSILON: f64 = 0.000_001;

    pub fn fuzzy_eq(x: f64, y: f64) -> bool {
        (x - y).abs() <= EPSILON
    }
    pub fn fuzzy_neq(x: f64, y: f64) -> bool {
        !fuzzy_eq(x, y)
    }
    pub fn fuzzy_lt(x: f64, y: f64) -> bool {
        (x - y) < -EPSILON
    }

    #[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
    pub struct FlWeight(f64);

    #[cfg(not(override_tdcch_approx))]
    pub const APPROX: FlWeight = FlWeight(1.0);
    #[cfg(override_tdcch_approx)]
    pub const APPROX: FlWeight = FlWeight(include!(concat!(env!("OUT_DIR"), "/TDCCH_APPROX")));

    impl FlWeight {
        pub const INFINITY: Self = FlWeight(2_147_483_647.0);

        pub fn new(t: f64) -> Self {
            debug_assert_ne!(t, NAN);
            FlWeight(t)
        }

        pub const fn zero() -> Self {
            FlWeight(0.0)
        }

        pub fn fuzzy_eq(self, other: Self) -> bool {
            fuzzy_eq(self.0, other.0)
        }
        pub fn fuzzy_lt(self, other: Self) -> bool {
            fuzzy_lt(self.0, other.0)
        }

        pub fn abs(self) -> FlWeight {
            FlWeight::new(self.0.abs())
        }
    }

    impl Eq for FlWeight {} // TODO ensure that the val will never be NAN

    impl Ord for FlWeight {
        fn cmp(&self, other: &Self) -> Ordering {
            self.partial_cmp(other).unwrap()
        }
    }

    impl<W: Borrow<FlWeight>> Add<W> for FlWeight {
        type Output = FlWeight;

        fn add(self, other: W) -> Self::Output {
            FlWeight::new(self.0 + other.borrow().0)
        }
    }

    impl<'a, W: Borrow<FlWeight>> Add<W> for &'a FlWeight {
        type Output = FlWeight;

        fn add(self, other: W) -> Self::Output {
            *self + other
        }
    }

    impl Add<Timestamp> for FlWeight {
        type Output = Timestamp;

        fn add(self, other: Timestamp) -> Self::Output {
            Timestamp::new(self.0 + other.0)
        }
    }

    impl Sub<FlWeight> for FlWeight {
        type Output = FlWeight;

        fn sub(self, other: FlWeight) -> Self::Output {
            FlWeight::new(self.0 - other.0)
        }
    }

    impl Mul<FlWeight> for FlWeight {
        type Output = FlWeight;

        fn mul(self, other: FlWeight) -> Self::Output {
            FlWeight::new(self.0 * other.0)
        }
    }

    impl Mul<FlWeight> for f64 {
        type Output = FlWeight;

        fn mul(self, other: FlWeight) -> Self::Output {
            FlWeight::new(self * other.0)
        }
    }

    impl Div<FlWeight> for FlWeight {
        type Output = FlWeight;

        fn div(self, other: FlWeight) -> Self::Output {
            debug_assert!(fuzzy_neq(other.0, 0.0));
            FlWeight::new(self.0 / other.0)
        }
    }

    impl<W: Borrow<Timestamp>> From<W> for FlWeight {
        fn from(w: W) -> Self {
            FlWeight::new(w.borrow().0)
        }
    }

    impl From<FlWeight> for f64 {
        fn from(w: FlWeight) -> Self {
            w.0
        }
    }

    impl Default for FlWeight {
        fn default() -> Self {
            Self::zero()
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
    pub struct Timestamp(f64);

    impl Timestamp {
        pub const NEVER: Self = Timestamp(2_147_483_647.0);

        pub fn new(t: f64) -> Self {
            debug_assert_ne!(t, NAN);
            Timestamp(t)
        }

        pub const fn zero() -> Self {
            Timestamp(0.0)
        }

        pub fn fuzzy_eq(self, other: Self) -> bool {
            fuzzy_eq(self.0, other.0)
        }
        pub fn fuzzy_lt(self, other: Self) -> bool {
            fuzzy_lt(self.0, other.0)
        }

        pub fn split_of_period(self) -> (FlWeight, Timestamp) {
            (FlWeight::new(self.0.div_euclid(super::period().0)), Timestamp::new(self.0.rem_euclid(super::period().0)))
        }
    }

    impl Eq for Timestamp {} // TODO ensure that the val will never be NAN

    impl Ord for Timestamp {
        fn cmp(&self, other: &Self) -> Ordering {
            self.partial_cmp(other).unwrap()
        }
    }

    impl<W: Borrow<FlWeight>> From<W> for Timestamp {
        fn from(w: W) -> Self {
            // TODO modulo period?
            Timestamp::new(w.borrow().0)
        }
    }

    impl<W: Borrow<FlWeight>> Add<W> for Timestamp {
        type Output = Timestamp;

        fn add(self, other: W) -> Self::Output {
            let result = self.0 + other.borrow().0;
            Timestamp::new(result)
        }
    }

    impl<W: Borrow<FlWeight>> Sub<W> for Timestamp {
        type Output = Timestamp;

        fn sub(self, other: W) -> Self::Output {
            let result = self.0 - other.borrow().0;
            Timestamp::new(result)
        }
    }

    impl Sub<Timestamp> for Timestamp {
        type Output = FlWeight;

        fn sub(self, other: Timestamp) -> Self::Output {
            FlWeight::new(self.0 - other.0)
        }
    }

    impl From<Timestamp> for f64 {
        fn from(t: Timestamp) -> Self {
            t.0
        }
    }

    impl Default for Timestamp {
        fn default() -> Self {
            Self::zero()
        }
    }
}
pub use self::time::*;

#[cfg(test)]
thread_local! {
    static TEST_PERIOD_MOCK: std::cell::Cell<Option<Timestamp>> = std::cell::Cell::new(None);
}

#[cfg(test)]
unsafe fn set_period(period: Timestamp) {
    TEST_PERIOD_MOCK.with(|period_cell| period_cell.set(Some(period)))
}

#[cfg(test)]
unsafe fn reset_period() {
    TEST_PERIOD_MOCK.with(|period_cell| period_cell.set(None))
}

#[cfg(test)] use std::panic;
#[cfg(test)]
fn run_test_with_periodicity<T>(period: Timestamp, test: T) -> ()
    where T: FnOnce() -> () + panic::UnwindSafe
{
    unsafe { set_period(period) };

    let result = panic::catch_unwind(|| {
        test()
    });

    unsafe { reset_period() };

    assert!(result.is_ok())
}

#[cfg(test)]
pub fn period() -> Timestamp {
    return TEST_PERIOD_MOCK.with(|period_cell| period_cell.get().expect("period() used but not set"));
}

#[cfg(not(test))]
#[inline]
pub fn period() -> Timestamp {
    Timestamp::new(86_400.0)
}

use std::sync::atomic::{AtomicUsize, AtomicIsize};

pub static NODES_CUSTOMIZED: AtomicUsize = AtomicUsize::new(0);
pub static IPP_COUNT: AtomicUsize = AtomicUsize::new(0);
pub static PATH_SOURCES_COUNT: AtomicUsize = AtomicUsize::new(0);
pub static ACTUALLY_MERGED: AtomicUsize = AtomicUsize::new(0);
pub static ACTUALLY_LINKED: AtomicUsize = AtomicUsize::new(0);
pub static ACTIVE_SHORTCUTS: AtomicUsize = AtomicUsize::new(0);
pub static UNNECESSARY_LINKED: AtomicUsize = AtomicUsize::new(0);
pub static CONSIDERED_FOR_APPROX: AtomicUsize = AtomicUsize::new(0);
pub static SAVED_BY_APPROX: AtomicIsize = AtomicIsize::new(0);

#[derive(Debug)]
pub struct ReusablePLFStorage {
    data: Vec<TTFPoint>,
    first_points: Vec<u32>
}

impl ReusablePLFStorage {
    fn new() -> Self {
        ReusablePLFStorage { data: Vec::new(), first_points: Vec::new() }
    }

    fn push_plf(&mut self) -> MutTopPLF {
        self.first_points.push(self.data.len() as u32);
        MutTopPLF { storage: self }
    }

    fn top_plfs(&self) -> (&[TTFPoint], &[TTFPoint]) {
        let num_plfs = self.first_points.len();
        (&self.data[self.first_points[num_plfs - 2] as usize .. self.first_points[num_plfs - 1] as usize], &self.data[self.first_points[num_plfs - 1] as usize ..])
    }

    fn top_plf(&self) -> &[TTFPoint] {
        &self.data[self.first_points[self.first_points.len() - 1] as usize ..]
    }
}

#[derive(Debug)]
struct MutTopPLF<'a> {
    storage: &'a mut ReusablePLFStorage
}

impl<'a> MutTopPLF<'a> {
    fn storage(&self) -> &ReusablePLFStorage {
        &self.storage
    }

    fn storage_mut(&mut self) -> &mut ReusablePLFStorage {
        &mut self.storage
    }
}

impl<'a> PLFTarget for MutTopPLF<'a> {
    fn push(&mut self, val: TTFPoint) {
        self.storage.data.push(val);
    }

    fn pop(&mut self) -> Option<TTFPoint> {
        if self.is_empty() {
            None
        } else {
            self.storage.data.pop()
        }
    }
}

impl<'a> Extend<TTFPoint> for MutTopPLF<'a> {
    fn extend<T: IntoIterator<Item = TTFPoint>>(&mut self, iter: T) {
        self.storage.data.extend(iter);
    }
}

impl<'a> std::ops::Deref for MutTopPLF<'a> {
    type Target = [TTFPoint];

    fn deref(&self) -> &Self::Target {
        self.storage.top_plf()
    }
}

impl<'a> Drop for MutTopPLF<'a> {
    fn drop(&mut self) {
        self.storage.data.truncate(*self.storage.first_points.last().unwrap() as usize);
        self.storage.first_points.pop();
    }
}

trait PLFTarget: Extend<TTFPoint> + std::ops::Deref<Target = [TTFPoint]> {
    fn push(&mut self, val: TTFPoint);
    fn pop(&mut self) -> Option<TTFPoint>;
}

impl PLFTarget for Vec<TTFPoint> {
    fn push(&mut self, val: TTFPoint) {
        self.push(val);
    }

    fn pop(&mut self) -> Option<TTFPoint> {
        self.pop()
    }
}