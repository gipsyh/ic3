#![feature(assert_matches, is_sorted, get_mut_unchecked, format_args_nl)]

mod activity;
#[allow(dead_code)]
mod analysis;
mod basic;
mod command;
mod frames;
mod mic;
mod model;
mod solver;
mod statistic;
mod verify;

use crate::basic::ProofObligation;
use crate::frames::Lemma;
use crate::{basic::BasicShare, statistic::Statistic};
use crate::{basic::ProofObligationQueue, solver::Lift};
use activity::Activity;
use aig::Aig;
pub use command::Args;
use frames::Frames;
use logic_form::Cube;
use model::Model;
use solver::{BlockResult, BlockResultYes, Ic3Solver};
use std::panic::{self, AssertUnwindSafe};
use std::process::exit;
use std::{sync::Arc, time::Instant};

pub struct Ic3 {
    pub solvers: Vec<Ic3Solver>,
    pub frames: Frames,
    pub share: Arc<BasicShare>,
    pub activity: Activity,
    pub obligations: ProofObligationQueue,
    pub lift: Lift,
    pub statistic: Statistic,
    enable_restart: bool,
}

impl Ic3 {
    pub fn depth(&self) -> usize {
        self.solvers.len() - 1
    }

    pub(crate) fn new_frame(&mut self) {
        self.frames.new_frame();
        self.solvers
            .push(Ic3Solver::new(self.share.clone(), self.solvers.len()));
    }

    fn generalize(&mut self, frame: usize, cube: Cube) -> (usize, Cube) {
        let level = if self.share.args.ctg { 1 } else { 0 };
        let mut cube = self.mic(frame, cube, level);
        for i in frame + 1..=self.depth() {
            match self.blocked(i, &cube) {
                BlockResult::Yes(block) => cube = self.blocked_conflict(&block),
                BlockResult::No(_) => return (i, cube),
            }
        }
        (self.depth() + 1, cube)
    }

    pub fn handle_blocked(&mut self, mut po: ProofObligation, blocked: BlockResultYes) {
        // let conflict = self.blocked_conflict(&blocked);
        let conflict = if let Some(successor) = &po.successor {
            self.test_conflict(
                (*po.lemma).clone(),
                po.frame,
                (**successor).clone(),
                blocked,
            )
        } else {
            self.blocked_conflict(&blocked)
        };
        let (frame, core) = self.generalize(po.frame, conflict);
        self.statistic.average_po_cube_len += po.lemma.len();
        po.frame = frame;
        self.add_obligation(po);
        self.add_cube(frame - 1, core);
    }

    pub(crate) fn block(&mut self) -> bool {
        while let Some(po) = self.obligations.pop(self.depth()) {
            if po.frame == 0 {
                return false;
            }
            assert!(!self.share.model.cube_subsume_init(&po.lemma));
            if self.share.args.verbose_all {
                self.statistic();
            }
            if self.frames.trivial_contained(po.frame, &po.lemma) {
                self.add_obligation(ProofObligation::new(
                    po.frame + 1,
                    po.lemma,
                    po.depth,
                    po.successor,
                ));
                continue;
            }
            // if self.sat_contained(po.frame, &po.cube) {
            //     continue;
            // }
            match self.blocked(po.frame, &po.lemma) {
                BlockResult::Yes(blocked) => {
                    self.handle_blocked(po, blocked);
                }
                BlockResult::No(unblocked) => {
                    let model = self.unblocked_model(&unblocked);
                    self.add_obligation(ProofObligation::new(
                        po.frame - 1,
                        Lemma::new(model),
                        po.depth + 1,
                        Some(po.lemma.clone()),
                    ));
                    self.add_obligation(po);
                }
            }
        }
        true
    }

    pub(crate) fn propagate(&mut self) -> bool {
        for frame_idx in self.frames.early()..self.depth() {
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
                    }
                    BlockResult::No(_) => {}
                }
            }
            self.solvers[frame_idx + 1].simplify();
            if self.frames[frame_idx].is_empty() {
                return true;
            }
        }
        self.frames.reset_early();
        false
    }
}

impl Ic3 {
    pub fn new(args: Args) -> Self {
        let aig = Aig::from_file(args.model.as_ref().unwrap()).unwrap();
        let model = Model::from_aig(&aig, !args.backward);
        let share = Arc::new(BasicShare { args, model });
        let mut res = Self {
            solvers: Vec::new(),
            frames: Frames::new(),
            activity: Activity::new(),
            lift: Lift::new(share.clone()),
            statistic: Statistic::new(share.args.model.as_ref().unwrap()),
            share,
            obligations: ProofObligationQueue::new(),
            enable_restart: true,
        };
        res.new_frame();
        for cube in res.share.model.inits() {
            res.add_cube(0, cube)
        }
        res
    }

    pub fn check(&mut self) -> bool {
        loop {
            let start = Instant::now();
            loop {
                if !self.block() {
                    self.statistic.overall_block_time += start.elapsed();
                    self.statistic();
                    return false;
                }
                if let Some(cex) = self.get_bad() {
                    self.add_obligation(ProofObligation::new(
                        self.depth(),
                        Lemma::new(cex),
                        0,
                        None,
                    ));
                } else {
                    break;
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
            let propagate = self.propagate();
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

    pub fn check_with_int_hanlder(&mut self) -> bool {
        let ic3 = self as *mut Ic3 as usize;
        ctrlc::set_handler(move || {
            let ic3 = unsafe { &mut *(ic3 as *mut Ic3) };
            ic3.statistic();
            exit(130);
        })
        .unwrap();
        panic::catch_unwind(AssertUnwindSafe(|| self.check())).unwrap_or_else(|_| {
            self.statistic();
            panic!();
        })
    }
}
