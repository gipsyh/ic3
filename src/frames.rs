use crate::Ic3;
use logic_form::Cube;
use minisat::SatResult;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fmt::{self, Debug, Display},
    mem::take,
    ops::{Deref, DerefMut},
};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Frames {
    frames: Vec<Vec<Cube>>,
}

impl Frames {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn new_frame(&mut self) {
        self.frames.push(Vec::new());
    }

    pub fn trivial_contained(&self, frame: usize, cube: &Cube) -> bool {
        for i in frame..self.frames.len() {
            for c in self.frames[i].iter() {
                if c.ordered_subsume(cube) {
                    return true;
                }
            }
        }
        false
    }

    pub fn statistic(&self) {
        for frame in self.frames.iter() {
            print!("{} ", frame.len());
        }
        println!();
    }
}

impl Deref for Frames {
    type Target = Vec<Vec<Cube>>;

    fn deref(&self) -> &Self::Target {
        &self.frames
    }
}

impl DerefMut for Frames {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.frames
    }
}

impl Ic3 {
    pub fn add_cube(&mut self, frame: usize, mut cube: Cube) {
        if frame == 0 {
            assert!(self.frames.len() == 1);
            self.solvers[0].add_clause(&!&cube);
            self.frames[0].push(cube);
            return;
        }
        cube.sort_by_key(|x| x.var());
        if self.frames.trivial_contained(frame, &cube) {
            return;
        }
        assert!(!self.share.model.cube_subsume_init(&cube));
        let mut begin = 1;
        for i in 1..=frame {
            let cubes = take(&mut self.frames[i]);
            for c in cubes {
                if c.ordered_subsume(&cube) {
                    begin = i + 1;
                }
                if !cube.ordered_subsume(&c) {
                    self.frames[i].push(c);
                }
            }
        }
        let clause = !&cube;
        if cube.len() > 1 {
            for i in 0..cube.len() {
                let mut c = cube.clone();
                c.remove(i);
                let entry = self.testda.entry(c).or_insert(HashSet::default());
                entry.insert(cube[i]);
            }
        }
        self.frames[frame].push(cube);
        for i in begin..=frame {
            self.solvers[i].add_clause(&clause);
        }
    }

    pub fn sat_contained(&mut self, frame: usize, cube: &Cube) -> bool {
        matches!(self.solvers[frame].solve(cube), SatResult::Unsat(_))
    }
}

impl Display for Frames {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for i in 1..self.frames.len() {
            f.write_fmt(format_args_nl!("frame {}", i))?;
            let mut frame = self.frames[i].clone();
            frame.sort();
            for c in frame.iter() {
                f.write_fmt(format_args_nl!("{:?}", c))?;
            }
        }
        Ok(())
    }
}
