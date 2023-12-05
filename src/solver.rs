use super::{basic::BasicShare, frames::Frames};
use crate::Ic3;
use logic_form::{Clause, Cube, Lit, Var};
use minisat::{SatResult, Solver};
use std::{mem::take, sync::Arc, time::Instant};

pub struct Ic3Solver {
    solver: Solver,
    num_act: usize,
    share: Arc<BasicShare>,
    frame: usize,
    temporary: Vec<Cube>,
}

impl Ic3Solver {
    pub fn new(share: Arc<BasicShare>, frame: usize) -> Self {
        let mut solver = Solver::new();
        if let Some(seed) = share.args.random {
            solver.set_random_seed(seed as f64);
            solver.set_rnd_init_act(true);
        }
        let false_lit: Lit = solver.new_var().into();
        solver.add_clause(&[!false_lit]);
        share.model.load_trans(&mut solver);
        Self {
            solver,
            frame,
            num_act: 0,
            share,
            temporary: Vec::new(),
        }
    }

    pub fn reset(&mut self, frames: &Frames) {
        let temporary = take(&mut self.temporary);
        *self = Self::new(self.share.clone(), self.frame);
        for t in temporary {
            self.solver.add_clause(&!&t);
            self.temporary.push(t);
        }
        let frames_slice = if self.frame == 0 {
            &frames[0..1]
        } else {
            &frames[self.frame..]
        };
        for dnf in frames_slice.iter() {
            for cube in dnf {
                self.add_clause(&!cube);
            }
        }
        self.simplify()
    }

    pub fn add_clause(&mut self, clause: &Clause) {
        let mut cube = !clause;
        cube.sort_by_key(|x| x.var());
        let temporary = take(&mut self.temporary);
        for t in temporary {
            if !cube.ordered_subsume(&t) {
                self.temporary.push(t);
            }
        }
        self.solver.add_clause(clause);
    }

    pub fn simplify(&mut self) {
        assert!(self.solver.simplify())
    }

    pub fn set_polarity(&mut self, var: Var, pol: Option<bool>) {
        self.solver.set_polarity(var, pol)
    }

    #[allow(unused)]
    pub fn solve<'a>(&'a mut self, assumptions: &[Lit]) -> SatResult<'a> {
        self.solver.solve(assumptions)
    }

    pub fn add_temporary_clause(&mut self, clause: &Clause) {
        let mut cube = !clause;
        cube.sort_by_key(|x| x.var());
        for t in self.temporary.iter() {
            if t.ordered_subsume(&cube) {
                return;
            }
        }
        let temporary = take(&mut self.temporary);
        for t in temporary {
            if !cube.ordered_subsume(&t) {
                self.temporary.push(t);
            }
        }
        self.temporary.push(cube);
        self.solver.add_clause(clause);
    }
}

impl Ic3 {
    pub fn get_bad(&mut self) -> Option<Cube> {
        let bad = if self.share.aig.bads.is_empty() {
            self.share.aig.outputs[0]
        } else {
            self.share.aig.bads[0]
        };
        if let SatResult::Sat(_) = self.solvers.last_mut().unwrap().solve(&[bad.to_lit()]) {
            self.statistic.num_get_bad_state += 1;
            let model = unsafe { self.solvers.last().unwrap().solver.get_model() };
            let bad = self.share.bad.clone();
            let cex = self.minimal_predecessor(&bad, model);
            // let cex = self.generalize_by_ternary_simulation(model, &bad);
            return Some(cex);
        }
        None
    }

    fn blocked_inner(&mut self, frame: usize, cube: &Cube) -> BlockResult {
        self.statistic.num_sat_inductive += 1;
        let solver_idx = frame - 1;
        let solver = &mut self.solvers[solver_idx].solver;
        let start = Instant::now();
        let mut assumption = self.share.model.cube_next(cube);
        let act = solver.new_var().into();
        assumption.push(act);
        let mut tmp_cls = !cube;
        tmp_cls.push(!act);
        solver.add_clause(&tmp_cls);
        let res = solver.solve(&assumption);
        let act = !assumption.pop().unwrap();
        let res = match res {
            SatResult::Sat(_) => BlockResult::No(BlockResultNo {
                solver_idx,
                assumption,
            }),
            SatResult::Unsat(_) => BlockResult::Yes(BlockResultYes {
                solver_idx,
                cube: cube.clone(),
                assumption,
            }),
        };
        solver.release_var(act);
        self.statistic.sat_inductive_time += start.elapsed();
        res
    }

    pub fn blocked(&mut self, frame: usize, cube: &Cube) -> BlockResult {
        assert!(!self.share.model.cube_subsume_init(cube));
        let solver = &mut self.solvers[frame - 1];
        solver.num_act += 1;
        if solver.num_act > 1000 {
            self.statistic.num_solver_restart += 1;
            solver.reset(&self.frames)
        }
        self.blocked_inner(frame, cube)
    }

    pub fn blocked_with_ordered(
        &mut self,
        frame: usize,
        cube: &Cube,
        ascending: bool,
    ) -> BlockResult {
        let mut ordered_cube = cube.clone();
        self.activity.sort_by_activity(&mut ordered_cube, ascending);
        self.blocked(frame, &ordered_cube)
    }
}

pub enum BlockResult {
    Yes(BlockResultYes),
    No(BlockResultNo),
}

#[derive(Debug)]
pub struct BlockResultYes {
    solver_idx: usize,
    cube: Cube,
    assumption: Cube,
}

#[derive(Debug)]
pub struct BlockResultNo {
    solver_idx: usize,
    assumption: Cube,
}

impl Ic3 {
    pub fn blocked_conflict(&mut self, block: &BlockResultYes) -> Cube {
        let conflict = unsafe { self.solvers[block.solver_idx].solver.get_conflict() };
        let mut ans = Cube::new();
        for i in 0..block.cube.len() {
            if conflict.has(!block.assumption[i]) {
                ans.push(block.cube[i]);
            }
        }
        if self.share.model.cube_subsume_init(&ans) {
            ans = Cube::new();
            let new = *block
                .cube
                .iter()
                .find(|l| {
                    self.share
                        .model
                        .init
                        .get(&l.var())
                        .is_some_and(|i| *i != l.polarity())
                })
                .unwrap();
            for i in 0..block.cube.len() {
                if conflict.has(!block.assumption[i]) || block.cube[i] == new {
                    ans.push(block.cube[i]);
                }
            }
            assert!(!self.share.model.cube_subsume_init(&ans));
        }
        ans
    }

    pub fn unblocked_model(&mut self, unblock: &BlockResultNo) -> Cube {
        let model = unsafe { self.solvers[unblock.solver_idx].solver.get_model() };
        self.minimal_predecessor(&unblock.assumption, model)
        // self.generalize_by_ternary_simulation(model, &unblock.assumption)
    }

    pub fn unblocked_model_lit_value(&self, unblock: &BlockResultNo, lit: Lit) -> bool {
        unsafe { self.solvers[unblock.solver_idx].solver.get_model() }.lit_value(lit)
    }
}

pub struct Lift {
    solver: Solver,
    num_act: usize,
}

impl Lift {
    pub fn new(share: Arc<BasicShare>) -> Self {
        let mut solver = Solver::new();
        if let Some(seed) = share.args.random {
            solver.set_random_seed(seed as f64);
            solver.set_rnd_init_act(true);
        }
        let false_lit: Lit = solver.new_var().into();
        solver.add_clause(&[!false_lit]);
        share.model.load_trans(&mut solver);
        Self { solver, num_act: 0 }
    }
}

impl Ic3 {
    pub fn minimal_predecessor(&mut self, successor: &Cube, model: minisat::Model) -> Cube {
        self.lift.num_act += 1;
        if self.lift.num_act > 1000 {
            self.lift = Lift::new(self.share.clone())
        }
        let act: Lit = self.lift.solver.new_var().into();
        let mut assumption = Cube::from([act]);
        let mut cls = !successor;
        cls.push(!act);
        self.lift.solver.add_clause(&cls);
        for input in self.share.aig.inputs.iter() {
            let mut lit: Lit = Var::from(*input).into();
            if !model.lit_value(lit) {
                lit = !lit;
            }
            assumption.push(lit);
        }
        let mut latchs = Cube::new();
        for latch in &self.share.aig.latchs {
            let mut lit: Lit = Var::from(latch.input).into();
            if !model.lit_value(lit) {
                lit = !lit;
            }
            latchs.push(lit);
        }
        self.activity.sort_by_activity(&mut latchs, false);
        assumption.extend_from_slice(&latchs);
        let res: Cube = match self.lift.solver.solve(&assumption) {
            SatResult::Sat(_) => panic!(),
            SatResult::Unsat(conflict) => {
                latchs.into_iter().filter(|l| conflict.has(!*l)).collect()
            }
        };
        self.lift.solver.release_var(!act);
        if res.len() > 0 {
            let v: usize = res[0].var().into();
            self.statistic.test_succ.statistic(v > self.origin);
        }
        res
    }
}
