use super::{solver::BlockResult, Ic3};
use crate::solver::BlockResultNo;
use logic_form::{Cube, Lit};
use std::{
    collections::{HashSet, VecDeque},
    time::Instant,
};
#[derive(Debug)]
enum DownResult {
    Success(Cube),
    Fail(BlockResultNo),
    IncludeInit,
}

impl Ic3 {
    fn down(&mut self, frame: usize, cube: &Cube) -> DownResult {
        if self.share.model.cube_subsume_init(cube) {
            return DownResult::IncludeInit;
        }
        self.statistic.num_down_blocked += 1;
        match self.blocked_with_ordered(frame, cube, false) {
            BlockResult::Yes(blocked) => DownResult::Success(self.blocked_conflict(&blocked)),
            BlockResult::No(unblock) => DownResult::Fail(unblock),
        }
    }

    fn ctg_down(&mut self, frame: usize, cube: &Cube, keep: &HashSet<Lit>) -> DownResult {
        let mut cube = cube.clone();
        self.statistic.num_ctg_down += 1;
        let mut ctgs = 0;
        loop {
            if self.share.model.cube_subsume_init(&cube) {
                return DownResult::IncludeInit;
            }
            match self.blocked(frame, &cube) {
                BlockResult::Yes(blocked) => {
                    return DownResult::Success(self.blocked_conflict(&blocked))
                }
                BlockResult::No(unblocked) => {
                    let mut model = self.unblocked_model(&unblocked);
                    if ctgs < 3 && frame > 1 && !self.share.model.cube_subsume_init(&model) {
                        if self.share.args.cav23 {
                            self.cav23_activity.sort_by_activity(&mut model, false);
                        }
                        if let BlockResult::Yes(blocked) = self.blocked(frame - 1, &model) {
                            ctgs += 1;
                            let conflict = self.blocked_conflict(&blocked);
                            let mut i = frame;
                            while i <= self.depth() {
                                if let BlockResult::No(_) = self.blocked(i, &conflict) {
                                    break;
                                }
                                i += 1;
                            }
                            let conflict = self.mic(i - 1, conflict, true);
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

    fn ctg_down_test(
        &mut self,
        frame: usize,
        cube: &Cube,
        keep: &HashSet<Lit>,
        diff: &mut VecDeque<Lit>,
    ) -> DownResult {
        let mut cube = cube.clone();
        self.statistic.num_ctg_down += 1;
        let mut ctgs = 0;
        let mut changed = false;
        let diff_clone = diff.clone();
        loop {
            if self.share.model.cube_subsume_init(&cube) {
                return DownResult::IncludeInit;
            }
            match self.blocked(frame, &cube) {
                BlockResult::Yes(blocked) => {
                    return DownResult::Success(self.blocked_conflict(&blocked))
                }
                BlockResult::No(unblocked) => {
                    let mut model = self.unblocked_model(&unblocked);
                    if !changed {
                        *diff = diff_clone.clone();
                        diff.retain(|l| {
                            let pl = self.share.model.lit_next(*l);
                            !self.unblocked_model_lit_value(&unblocked, pl)
                        });
                    }
                    if ctgs < 3 && frame > 1 && !self.share.model.cube_subsume_init(&model) {
                        if self.share.args.cav23 {
                            self.cav23_activity.sort_by_activity(&mut model, false);
                        }
                        if let BlockResult::Yes(blocked) = self.blocked(frame - 1, &model) {
                            ctgs += 1;
                            let conflict = self.blocked_conflict(&blocked);
                            let mut i = frame;
                            while i <= self.depth() {
                                if let BlockResult::No(_) = self.blocked(i, &conflict) {
                                    break;
                                }
                                i += 1;
                            }
                            let conflict = self.mic(i - 1, conflict, true);
                            self.add_cube(i - 1, conflict);
                            continue;
                        }
                    }
                    changed = true;
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

    pub fn mic(&mut self, frame: usize, mut cube: Cube, simple: bool) -> Cube {
        let start = Instant::now();
        self.statistic.average_mic_cube_len += cube.len();
        self.statistic.num_mic += 1;
        if !simple {
            self.add_temporary_cube(frame, &cube);
        }
        self.activity.sort_by_activity(&mut cube, true);
        let mut keep = HashSet::new();
        let cav23_parent = self.share.args.cav23.then(|| {
            self.cav23_activity.sort_by_activity(&mut cube, true);
            let mut parent = self.frames.parent(&cube, frame);
            parent.sort_by(|a, b| {
                self.cav23_activity
                    .cube_average_activity(b)
                    .partial_cmp(&self.cav23_activity.cube_average_activity(a))
                    .unwrap()
            });
            let parent = parent.into_iter().nth(0);
            if let Some(parent) = &parent {
                for l in parent.iter() {
                    keep.insert(*l);
                }
            }
            parent
        });
        let mut parent = self.frames.parent(&cube, frame);
        self.statistic
            .test_parent_found
            .statistic(!parent.is_empty());
        if !parent.is_empty() {
            let mut ordered_cube = cube.clone();
            ordered_cube.sort();
            for p in parent.iter() {
                if p.eq(&ordered_cube) {
                    self.statistic.test_eq_parent.success();
                    self.statistic.sr_adv.success();
                    return cube;
                }
            }
            self.statistic.test_eq_parent.fail();
            parent.retain(|p| self.push_fail.contains_key(&(p.clone(), frame - 1)));

            self.statistic
                .test_push_fail_found
                .statistic(!parent.is_empty());

            self.statistic.sr_fp.statistic(!parent.is_empty());

            for p in parent {
                let cex = self.push_fail.get(&(p.clone(), frame - 1)).unwrap();
                let mut same = Cube::new();
                let mut diff = Cube::new();
                for l in cube.iter() {
                    if cex.binary_search(l).is_ok() {
                        same.push(*l);
                    } else {
                        diff.push(*l);
                    }
                }
                self.activity.sort_by_activity(&mut diff, false);
                // println!("cube {:?}", cube);
                // println!("parent {:?}", p);
                // println!("same {:?}", same);
                // println!("diff {:?}", diff);
                let keep = HashSet::from_iter(p.iter().copied());
                self.statistic.test_diff_empty.statistic(diff.is_empty());
                if !diff.is_empty() {
                    let mut diff = VecDeque::from_iter(diff);
                    while let Some(d) = diff.pop_front() {
                        let mut pp = p.clone();
                        pp.push(d);
                        // println!("try {:?}", pp);
                        let time = Instant::now();
                        let res = if simple {
                            self.down(frame, &pp)
                        } else {
                            self.ctg_down_test(frame, &pp, &keep, &mut diff)
                        };
                        match res {
                            DownResult::Success(res) => {
                                // dbg!("success");
                                self.statistic.sr_lp.success();
                                self.statistic.test_down_diff.success();
                                self.statistic.sr_adv.success();
                                return res;
                            }
                            DownResult::Fail(unblock) => {
                                self.statistic.sr_lp.fail();
                                // dbg!("fail");
                                self.statistic.test_down_diff.fail();
                                if simple {
                                    diff.retain(|l| {
                                        let pl = self.share.model.lit_next(*l);
                                        !self.unblocked_model_lit_value(&unblock, pl)
                                    });
                                }
                            }
                            DownResult::IncludeInit => (),
                        }
                        self.statistic.test_fail_time += time.elapsed();
                        // dbg!("try fail");
                    }
                } else {
                    match self.down(frame, &p) {
                        DownResult::Success(res) => {
                            self.statistic.test_down_parent.success();
                            self.statistic.sr_lp.success();
                            self.statistic.sr_adv.success();
                            return res;
                        }
                        DownResult::Fail(unblock) => {
                            self.statistic.sr_lp.fail();
                            let mut cex = Cube::new();
                            for p in self.share.model.primes.iter() {
                                cex.push(Lit::new(
                                    *p,
                                    self.unblocked_model_lit_value(&unblock, p.lit()),
                                ));
                            }
                            let cex = self.share.model.cube_previous(&cex);
                            self.push_fail.insert((p.clone(), frame - 1), cex);
                        }
                        DownResult::IncludeInit => (),
                    }
                    self.statistic.test_down_parent.fail();
                }
            }
        } else {
            self.statistic.sr_fp.fail();
        }
        self.statistic.sr_adv.fail();
        let mut i = 0;
        while i < cube.len() {
            let mut removed_cube = cube.clone();
            removed_cube.remove(i);
            let res = if simple {
                self.down(frame, &removed_cube)
            } else {
                self.ctg_down(frame, &removed_cube, &keep)
            };
            match res {
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
        if let Some(Some(cav23)) = cav23_parent {
            cube.sort_by_key(|x| *x.var());
            if cube.ordered_subsume(&cav23) {
                self.cav23_activity.pump_cube_activity(&cube);
            }
        }
        self.activity.pump_cube_activity(&cube);
        if simple {
            self.statistic.simple_mic_time += start.elapsed()
        } else {
            self.statistic.mic_time += start.elapsed()
        }
        cube
    }
}
