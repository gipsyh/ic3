use crate::Ic3;
use gipsat::{BlockResult, BlockResultNo};
use logic_form::{Cube, Lit};
use satif::{SatResult, Satif, SatifSat, SatifUnsat};
use std::time::Instant;
use transys::Model;

impl Ic3 {
    pub fn blocked_with_ordered(
        &mut self,
        frame: usize,
        cube: &Cube,
        ascending: bool,
        strengthen: bool,
        bucket: bool,
    ) -> BlockResult {
        let mut ordered_cube = cube.clone();
        self.activity.sort_by_activity(&mut ordered_cube, ascending);
        self.gipsat
            .blocked(frame, &ordered_cube, strengthen, bucket)
    }
}

impl Ic3 {
    pub fn unblocked_model(&mut self, unblock: BlockResultNo) -> Cube {
        self.minimal_predecessor(unblock)
    }
}

pub struct Lift {
    solver: minisat::Solver,
    num_act: usize,
}

impl Lift {
    pub fn new(model: &Model) -> Self {
        let mut solver = minisat::Solver::new();
        let false_lit: Lit = solver.new_var().into();
        solver.add_clause(&[!false_lit]);
        model.load_trans(&mut solver);
        Self { solver, num_act: 0 }
    }
}

impl Ic3 {
    pub fn minimal_predecessor(&mut self, unblock: BlockResultNo) -> Cube {
        let start = Instant::now();
        self.lift.num_act += 1;
        if self.lift.num_act > 1000 {
            self.lift = Lift::new(&self.model)
        }
        let act: Lit = self.lift.solver.new_var().into();
        let mut assumption = Cube::from([act]);
        let mut cls = !&unblock.assumption;
        cls.push(!act);
        self.lift.solver.add_clause(&cls);
        for input in self.model.inputs.iter() {
            let lit = input.lit();
            match unblock.sat.lit_value(lit) {
                Some(true) => assumption.push(lit),
                Some(false) => assumption.push(!lit),
                None => (),
            }
        }
        let mut latchs = Cube::new();
        for latch in self.model.latchs.iter() {
            let lit = latch.lit();
            match unblock.sat.lit_value(lit) {
                Some(true) => latchs.push(lit),
                Some(false) => latchs.push(!lit),
                None => (),
            }
        }
        self.activity.sort_by_activity(&mut latchs, false);
        assumption.extend_from_slice(&latchs);
        let res: Cube = match self.lift.solver.solve(&assumption) {
            SatResult::Sat(_) => panic!(),
            SatResult::Unsat(conflict) => latchs.into_iter().filter(|l| conflict.has(*l)).collect(),
        };
        self.lift.solver.add_clause(&[!act]);
        self.statistic.minimal_predecessor_time += start.elapsed();
        res
    }
}
