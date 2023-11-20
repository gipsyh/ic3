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
    pub num_blocked: usize,
    pub num_mic: usize,
    pub num_solver_restart: usize,
    pub num_down_blocked: usize,
    pub mic_drop: SuccessRate,
    pub num_ctg_down: usize,
    pub num_get_bad_state: usize,
    pub average_mic_cube_len: StatisticAverage,

    pub simple_mic_time: Duration,
    pub mic_time: Duration,
    pub blocked_check_time: Duration,

    pub overall_block_time: Duration,
    pub overall_propagate_time: Duration,

    pub test_fail_time: Duration,
    pub test_parent_found: SuccessRate,
    pub test_eq_parent: SuccessRate,
    pub test_push_fail_found: SuccessRate,
    pub test_diff_empty: SuccessRate,
    pub test_down_diff: SuccessRate,
    pub test_down_parent: SuccessRate,

    pub sr_lp: SuccessRate,
    pub sr_fp: SuccessRate,
    pub sr_adv: SuccessRate,
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
