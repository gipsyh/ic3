use super::{solver::BlockResult, Ic3};
use crate::solver::BlockResultNo;
use logic_form::{Cube, Lit};
use std::collections::HashSet;
#[derive(Debug)]
enum DownResult {
    Success(Cube),
    Fail(BlockResultNo),
    IncludeInit,
}

impl Ic3 {
    fn ctg_down(
        &mut self,
        frame: usize,
        cube: &Cube,
        keep: &HashSet<Lit>,
        level: usize,
    ) -> DownResult {
        let mut cube = cube.clone();
        self.statistic.num_down += 1;
        let mut ctgs = 0;
        loop {
            if self.share.model.cube_subsume_init(&cube) {
                return DownResult::IncludeInit;
            }
            match self.blocked_with_ordered(frame, &cube, false) {
                BlockResult::Yes(blocked) => {
                    return DownResult::Success(self.blocked_conflict(&blocked))
                }
                BlockResult::No(unblocked) => {
                    if level == 0 {
                        return DownResult::Fail(unblocked);
                    }
                    let model = self.unblocked_model(&unblocked);
                    if ctgs < 3 && frame > 1 && !self.share.model.cube_subsume_init(&model) {
                        if let BlockResult::Yes(blocked) =
                            self.blocked_with_ordered(frame - 1, &model, false)
                        {
                            ctgs += 1;
                            // let conflict = self.blocked_conflict(&blocked);
                            let conflict =
                                self.test_conflict(model, frame - 1, cube.clone(), blocked);
                            let mut i = frame;
                            while i <= self.depth() {
                                if let BlockResult::No(_) = self.blocked(i, &conflict) {
                                    break;
                                }
                                i += 1;
                            }
                            let conflict = self.mic(i - 1, conflict, level - 1);
                            self.add_cube(i - 1, conflict);
                            continue;
                        }
                    }
                    ctgs = 0;
                    let cex_set: HashSet<Lit> = HashSet::from_iter(model);
                    let mut cube_new = Cube::new();
                    for lit in cube {
                        if cex_set.contains(&lit) {
                            cube_new.push(lit);
                        } else if keep.contains(&lit) {
                            return DownResult::Fail(unblocked);
                        }
                    }
                    cube = cube_new;
                }
            }
        }
    }

    fn add_temporary_cube(&mut self, mut frame: usize, cube: &Cube) {
        frame = frame.min(self.depth());
        for solver in self.solvers[1..=frame].iter_mut() {
            solver.add_temporary_clause(&!cube);
        }
    }

    fn handle_down_success(
        &mut self,
        frame: usize,
        cube: Cube,
        i: usize,
        mut new_cube: Cube,
    ) -> (Cube, usize) {
        new_cube = cube
            .iter()
            .filter(|l| new_cube.contains(l))
            .cloned()
            .collect();
        let new_i = new_cube
            .iter()
            .position(|l| !(cube[0..i]).contains(l))
            .unwrap_or(new_cube.len());
        if new_i < new_cube.len() {
            assert!(!(cube[0..=i]).contains(&new_cube[new_i]))
        }
        self.add_temporary_cube(frame, &new_cube);
        (new_cube, new_i)
    }

    pub fn mic(&mut self, frame: usize, mut cube: Cube, level: usize) -> Cube {
        self.statistic.average_mic_cube_len += cube.len();
        self.statistic.num_mic += 1;
        if level > 0 {
            self.add_temporary_cube(frame, &cube);
        }
        self.activity.sort_by_activity(&mut cube, true);
        let mut keep = HashSet::new();
        let mut i = 0;
        while i < cube.len() {
            if keep.contains(&cube[i]) {
                i += 1;
                continue;
            }
            let mut removed_cube = cube.clone();
            removed_cube.remove(i);
            match self.ctg_down(frame, &removed_cube, &keep, level) {
                DownResult::Success(new_cube) => {
                    self.statistic.mic_drop.success();
                    (cube, i) = self.handle_down_success(frame, cube, i, new_cube);
                }
                _ => {
                    self.statistic.mic_drop.fail();
                    keep.insert(cube[i]);
                    i += 1;
                }
            }
        }
        self.activity.pump_cube_activity(&cube);
        cube
    }
}
