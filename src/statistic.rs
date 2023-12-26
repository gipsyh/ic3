use crate::Ic3;
use std::{
    fmt::{self, Debug},
    ops::AddAssign,
    time::{Duration, Instant},
};

#[allow(unused)]
#[derive(Debug, Default)]
pub struct Statistic {
    case: Case,
    time: RunningTime,
    pub num_get_bad_state: usize,

    pub num_sat_inductive: usize,
    pub sat_inductive_time: Duration,
    pub num_solver_restart: usize,

    pub num_mic: usize,
    pub average_mic_cube_len: StatisticAverage,
    pub average_po_cube_len: StatisticAverage,
    pub mic_drop: SuccessRate,
    pub num_down: usize,

    pub minimal_predecessor_time: Duration,

    pub overall_block_time: Duration,
    pub overall_propagate_time: Duration,

    pub test_success: SuccessRate,
}

impl Statistic {
    pub fn new(mut case: &str) -> Self {
        if let Some((_, c)) = case.rsplit_once('/') {
            case = c;
        }
        Self {
            case: Case(case.to_string()),
            ..Default::default()
        }
    }
}

#[derive(Default)]
pub struct StatisticAverage {
    sum: f64,
    num: usize,
}

impl Debug for StatisticAverage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.2}", self.sum / self.num as f64)
    }
}

impl AddAssign<usize> for StatisticAverage {
    fn add_assign(&mut self, rhs: usize) {
        self.sum += rhs as f64;
        self.num += 1;
    }
}

impl AddAssign<f64> for StatisticAverage {
    fn add_assign(&mut self, rhs: f64) {
        self.sum += rhs;
        self.num += 1;
    }
}

#[derive(Default)]
pub struct SuccessRate {
    succ: usize,
    fail: usize,
}

impl SuccessRate {
    pub fn success(&mut self) {
        self.succ += 1;
    }

    pub fn fail(&mut self) {
        self.fail += 1;
    }

    pub fn statistic(&mut self, success: bool) {
        if success {
            self.success()
        } else {
            self.fail()
        }
    }
}

impl Debug for SuccessRate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "success: {}, fail: {}, success rate: {:.2}%",
            self.succ,
            self.fail,
            (self.succ as f64 / (self.succ + self.fail) as f64) * 100_f64
        )
    }
}

#[derive(Default)]
pub struct Case(String);

impl Debug for Case {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

struct RunningTime {
    start: Instant,
}

impl Default for RunningTime {
    fn default() -> Self {
        Self {
            start: Instant::now(),
        }
    }
}

impl Debug for RunningTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.2}s", self.start.elapsed().as_secs_f64())
    }
}

impl Ic3 {
    pub fn statistic(&self) {
        self.obligations.statistic();
        self.frames.statistic();
        println!("{:#?}", self.statistic);
    }
}
