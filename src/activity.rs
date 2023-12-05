use logic_form::{Cube, Lit, Var};
use std::collections::HashMap;

#[derive(Default)]
pub struct Activity {
    activity: HashMap<Var, f64>,
}

impl Activity {
    pub fn new() -> Self {
        Self::default()
    }

    fn decay(&mut self) {
        for (_, act) in self.activity.iter_mut() {
            *act *= 0.99
        }
    }

    fn var_activity(&self, lit: Lit) -> f64 {
        match self.activity.get(&lit.var()) {
            Some(a) => *a,
            None => 0.0,
        }
    }

    fn pump_lit_activity(&mut self, lit: &Lit) {
        match self.activity.get_mut(&lit.var()) {
            Some(a) => *a += 1.0,
            None => {
                self.activity.insert(lit.var(), 1.0);
            }
        }
    }

    pub fn pump_lit_activity_test(&mut self, lit: &Lit) {
        match self.activity.get_mut(&lit.var()) {
            Some(a) => *a += 100000.0,
            None => {
                self.activity.insert(lit.var(), 100000.0);
            }
        }
    }

    pub fn pump_cube_activity(&mut self, cube: &Cube) {
        self.decay();
        cube.iter().for_each(|l| self.pump_lit_activity(l));
    }

    pub fn sort_by_activity(&self, cube: &mut Cube, ascending: bool) {
        if ascending {
            cube.sort_by(|a, b| {
                self.var_activity(*a)
                    .partial_cmp(&self.var_activity(*b))
                    .unwrap()
            });
        } else {
            cube.sort_by(|a, b| {
                self.var_activity(*b)
                    .partial_cmp(&self.var_activity(*a))
                    .unwrap()
            });
        }
    }

    pub fn cube_average_activity(&self, cube: &Cube) -> f64 {
        let sum: f64 = cube.iter().map(|l| self.var_activity(*l)).sum();
        sum / cube.len() as f64
    }
}
