#![feature(assert_matches, is_sorted, get_mut_unchecked, format_args_nl)]

mod activity;
#[allow(dead_code)]
mod analysis;
mod basic;
mod command;
mod frames;
mod mic;
mod model;
mod simulate;
mod solver;
mod statistic;
mod verify;

use crate::basic::ProofObligation;
use crate::{basic::BasicShare, statistic::Statistic};
use crate::{basic::ProofObligationQueue, solver::Lift};
use activity::Activity;
use aig::Aig;
pub use command::Args;
use frames::Frames;
use logic_form::{Cube, Lit};
use model::Model;
use solver::{BlockResult, Ic3Solver};
use std::collections::HashMap;
use std::panic::{self, AssertUnwindSafe};
use std::process::exit;
use std::{sync::Arc, time::Instant};

pub struct Ic3 {
    pub solvers: Vec<Ic3Solver>,
    pub frames: Frames,
    pub share: Arc<BasicShare>,
    pub activity: Activity,
    pub cav23_activity: Activity,
    pub obligations: ProofObligationQueue,
    pub lift: Lift,
    pub statistic: Statistic,
    pub push_fail: HashMap<(Cube, usize), Cube>,
}

impl Ic3 {
    pub fn depth(&self) -> usize {
        self.solvers.len() - 1
    }

    pub fn new_frame(&mut self) {
        self.frames.new_frame();
        self.solvers
            .push(Ic3Solver::new(self.share.clone(), self.solvers.len()));
    }

    fn generalize(&mut self, frame: usize, cube: Cube) -> (usize, Cube) {
        let mut cube = self.mic(frame, cube, !self.share.args.ctg);
        for i in frame + 1..=self.depth() {
            match self.blocked(i, &cube) {
                BlockResult::Yes(block) => cube = self.blocked_conflict(&block),
                BlockResult::No(unblock) => {
                    let mut cex = Cube::new();
                    for p in self.share.model.primes.iter() {
                        cex.push(Lit::new(
                            *p,
                            self.unblocked_model_lit_value(&unblock, p.lit()),
                        ));
                    }
                    let mut tmp = cube.clone();
                    tmp.sort();
                    let cex = self.share.model.cube_previous(&cex);
                    debug_assert!(tmp.ordered_subsume(&cex));
                    self.push_fail.insert((tmp, i - 1), cex);
                    return (i, cube);
                }
            }
        }
        (self.depth() + 1, cube)
    }

    pub fn handle_blocked(&mut self, po: ProofObligation, conflict: Cube) {
        let (frame, core) = self.generalize(po.frame, conflict);
        if frame <= self.depth() {
            self.obligations
                .add(ProofObligation::new(frame, po.cube, po.depth, po.successor));
        }
        self.add_cube(frame - 1, core);
    }

    pub fn block(&mut self, frame: usize, cube: Cube) -> bool {
        assert!(self.obligations.is_empty());
        self.obligations
            .add(ProofObligation::new(frame, cube, 0, None));
        while let Some(po) = self.obligations.pop() {
            if po.frame == 0 {
                return false;
            }
            assert!(!self.share.model.cube_subsume_init(&po.cube));
            if self.share.args.verbose_all {
                self.statistic();
            }
            if self.frames.trivial_contained(po.frame, &po.cube) {
                continue;
            }
            // if self.sat_contained(po.frame, &po.cube) {
            //     continue;
            // }
            match self.blocked(po.frame, &po.cube) {
                BlockResult::Yes(blocked) => {
                    let conflict = self.blocked_conflict(&blocked);
                    self.handle_blocked(po, conflict);
                }
                BlockResult::No(unblocked) => {
                    let model = self.unblocked_model(&unblocked);
                    self.obligations.add(ProofObligation::new(
                        po.frame - 1,
                        model,
                        po.depth + 1,
                        Some(po.cube.clone()),
                    ));
                    self.obligations.add(po);
                }
            }
        }
        true
    }

    pub fn propagate(&mut self, trivial: bool) -> bool {
        self.push_fail.clear();
        let start = if trivial {
            (self.depth() - 1).max(1)
        } else {
            1
        };
        for frame_idx in start..self.depth() {
            let mut frame = self.frames[frame_idx].clone();
            frame.sort_by_key(|x| x.len());
            for cube in frame {
                if !self.frames[frame_idx].contains(&cube) {
                    continue;
                }
                match self.blocked(frame_idx + 1, &cube) {
                    BlockResult::Yes(blocked) => {
                        let conflict = self.blocked_conflict(&blocked);
                        self.add_cube(frame_idx + 1, conflict);
                        if self.share.args.cav23 {
                            self.cav23_activity.pump_cube_activity(&cube);
                        }
                    }
                    BlockResult::No(unblock) => {
                        let mut cex = Cube::new();
                        for p in self.share.model.primes.iter() {
                            cex.push(Lit::new(
                                *p,
                                self.unblocked_model_lit_value(&unblock, p.lit()),
                            ));
                        }
                        let cex = self.share.model.cube_previous(&cex);
                        debug_assert!(cube.ordered_subsume(&cex));
                        self.push_fail.insert((cube.clone(), frame_idx), cex);
                    }
                }
            }
            self.solvers[frame_idx + 1].simplify();
            if self.frames[frame_idx].is_empty() {
                return true;
            }
        }
        false
    }
}

impl Ic3 {
    pub fn new(args: Args) -> Self {
        let aig = Aig::from_file(args.model.as_ref().unwrap()).unwrap();
        let model = Model::from_aig(&aig);
        let bad = Cube::from([if aig.bads.is_empty() {
            aig.outputs[0]
        } else {
            aig.bads[0]
        }
        .to_lit()]);
        let share = Arc::new(BasicShare {
            aig,
            args,
            model,
            bad,
        });
        let mut res = Self {
            solvers: Vec::new(),
            frames: Frames::new(),
            activity: Activity::new(),
            cav23_activity: Activity::new(),
            lift: Lift::new(share.clone()),
            statistic: Statistic::new(share.args.model.as_ref().unwrap()),
            share,
            obligations: ProofObligationQueue::new(),
            push_fail: HashMap::new(),
        };
        res.new_frame();
        for i in 0..res.share.aig.latchs.len() {
            let l = &res.share.aig.latchs[i];
            if let Some(init) = l.init {
                let cube = Cube::from([Lit::new(l.input.into(), !init)]);
                res.add_cube(0, cube)
            }
        }
        res
    }

    fn check_inner(&mut self) -> bool {
        loop {
            let start = Instant::now();
            let mut trivial = true;
            while let Some(cex) = self.get_bad() {
                trivial = false;
                if !self.block(self.depth(), cex) {
                    self.statistic();
                    return false;
                }
            }
            let blocked_time = start.elapsed();
            if self.share.args.verbose {
                println!(
                    "[{}:{}] frame: {}, time: {:?}",
                    file!(),
                    line!(),
                    self.depth(),
                    blocked_time,
                );
            }
            self.statistic.overall_block_time += blocked_time;
            self.new_frame();
            let start = Instant::now();
            let propagate = self.propagate(trivial);
            self.statistic.overall_propagate_time += start.elapsed();
            if propagate {
                self.statistic();
                if self.share.args.save_frames {
                    self.save_frames();
                }
                if self.share.args.verify {
                    assert!(self.verify());
                }
                return true;
            }
        }
    }

    pub fn check(&mut self) -> bool {
        let ic3 = self as *mut Ic3 as usize;
        ctrlc::set_handler(move || {
            let ic3 = unsafe { &mut *(ic3 as *mut Ic3) };
            ic3.statistic();
            exit(130);
        })
        .unwrap();
        panic::catch_unwind(AssertUnwindSafe(|| self.check_inner())).unwrap_or_else(|_| {
            self.statistic();
            panic!();
        })
    }
}
