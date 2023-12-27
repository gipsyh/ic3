use crate::Ic3;
use logic_form::{Cube, Lit};
use minisat::SatResult;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fmt::{self, Debug, Display},
    mem::take,
    ops::{Deref, DerefMut},
};

#[derive(Debug, Serialize, Default, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Lemma {
    cube: Cube,
    sign: u64,
}

impl Deref for Lemma {
    type Target = Cube;

    fn deref(&self) -> &Self::Target {
        &self.cube
    }
}

impl DerefMut for Lemma {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.cube
    }
}

impl Lemma {
    pub fn new(mut cube: Cube) -> Self {
        cube.sort();
        let mut sign = 0;
        for l in cube.iter() {
            sign |= 1 << (Into::<u32>::into(*l) % 63);
        }
        Self { cube, sign }
    }

    pub fn subsume(&self, other: &Lemma) -> bool {
        if self.cube.len() > other.cube.len() {
            return false;
        }
        if self.sign & other.sign != self.sign {
            return false;
        }
        self.cube.ordered_subsume(&other.cube)
    }

    pub fn subsume_set(&self, other: &Lemma, other_lits: &HashSet<Lit>) -> bool {
        if self.cube.len() > other.cube.len() {
            return false;
        }
        if self.sign & other.sign != self.sign {
            return false;
        }
        for l in self.iter() {
            if !other_lits.contains(l) {
                return false;
            }
        }
        true
    }
}

#[derive(Debug, Serialize, Default, Deserialize)]
pub struct Frames {
    frames: Vec<Vec<Lemma>>,
    early: usize,
}

impl Frames {
    pub fn new() -> Self {
        Self {
            frames: Vec::new(),
            early: 1,
        }
    }

    pub fn new_frame(&mut self) {
        self.frames.push(Vec::new());
    }

    pub fn trivial_contained(&self, frame: usize, lemma: &Lemma) -> bool {
        let mut lit_set: HashSet<Lit> = HashSet::with_capacity(lemma.len());
        for l in lemma.iter() {
            lit_set.insert(*l);
        }
        for i in frame..self.frames.len() {
            for l in self.frames[i].iter() {
                if l.subsume_set(lemma, &lit_set) {
                    return true;
                }
            }
        }
        false
    }

    pub fn early(&self) -> usize {
        self.early
    }

    pub fn reset_early(&mut self) {
        self.early = self.frames.len() - 1
    }

    pub fn similar(&self, cube: &Cube, frame: usize) -> Vec<Cube> {
        let cube_set: HashSet<Lit> = HashSet::from_iter(cube.iter().copied());
        let mut res = HashSet::new();
        for frame in self.frames[frame..].iter() {
            for lemma in frame.iter() {
                let sec: Cube = lemma
                    .iter()
                    .filter(|l| cube_set.contains(l))
                    .copied()
                    .collect();
                if sec.len() != cube.len() && sec.len() * 2 >= cube.len() {
                    res.insert(sec);
                }
            }
        }
        let mut res = Vec::from_iter(res.into_iter());
        res.sort_by(|x, y| {
            if x.len() == y.len() {
                x.cmp(y)
            } else {
                x.len().cmp(&y.len())
            }
        });
        res.reverse();
        if res.len() > 3 {
            res.truncate(3);
        }
        res
    }

    pub fn statistic(&self) {
        for frame in self.frames.iter() {
            print!("{} ", frame.len());
        }
        println!();
    }
}

impl Deref for Frames {
    type Target = Vec<Vec<Lemma>>;

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
    pub fn add_cube(&mut self, frame: usize, cube: Cube) {
        let lemma = Lemma::new(cube);
        if frame == 0 {
            assert!(self.frames.len() == 1);
            self.solvers[0].add_clause(&!&lemma.cube);
            self.frames[0].push(lemma);
            return;
        }
        if self.frames.trivial_contained(frame, &lemma) {
            return;
        }
        assert!(!self.share.model.cube_subsume_init(&lemma.cube));
        let mut begin = 1;
        for i in 1..=frame {
            let cubes = take(&mut self.frames[i]);
            for l in cubes {
                if l.subsume(&lemma) {
                    begin = i + 1;
                }
                if !lemma.subsume(&l) {
                    self.frames[i].push(l);
                }
            }
        }
        let clause = !&lemma.cube;
        self.frames[frame].push(lemma);
        for i in begin..=frame {
            self.solvers[i].add_clause(&clause);
        }
        self.frames.early = self.frames.early.min(begin);
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
