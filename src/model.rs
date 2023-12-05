use aig::{Aig, AigEdge};
use logic_form::{Clause, Cnf, Cube, Lit, Var};
use minisat::{SimpSolver, Solver};
use std::collections::HashMap;

pub struct Model {
    pub inputs: Vec<Var>,
    pub latchs: Vec<Var>,
    pub primes: Vec<Var>,
    pub init: HashMap<Var, bool>,
    pub constraints: Vec<Lit>,
    pub bad: Lit,
    pub trans: Cnf,
    num_var: usize,
    next_map: HashMap<Var, Var>,
    previous_map: HashMap<Var, Var>,
}

impl Model {
    pub fn from_aig(aig: &Aig) -> Self {
        let mut simp_solver = SimpSolver::new();
        let false_lit: Lit = simp_solver.new_var().into();
        simp_solver.add_clause(&[!false_lit]);
        for node in aig.nodes.iter().skip(1) {
            assert_eq!(Var::new(node.node_id()), simp_solver.new_var());
        }
        let inputs: Vec<Var> = aig.inputs.iter().map(|x| Var::new(*x)).collect();
        let latchs: Vec<Var> = aig.latchs.iter().map(|x| Var::new(x.input)).collect();
        let primes: Vec<Var> = latchs.iter().map(|_| simp_solver.new_var()).collect();
        let mut init = HashMap::new();
        for l in aig.latch_init_cube().to_cube() {
            init.insert(l.var(), l.polarity());
        }
        let constraints: Vec<Lit> = aig.constraints.iter().map(|c| c.to_lit()).collect();
        let aig_bad = if aig.bads.is_empty() {
            aig.outputs[0]
        } else {
            aig.bads[0]
        };
        let bad = aig_bad.to_lit();
        for v in inputs.iter().chain(latchs.iter()).chain(primes.iter()) {
            simp_solver.set_frozen(*v, true);
        }
        for l in constraints.iter() {
            simp_solver.set_frozen(l.var(), true);
        }
        simp_solver.set_frozen(bad.var(), true);
        let mut logic = Vec::new();
        for l in aig.latchs.iter() {
            logic.push(l.input.into());
            logic.push(l.next);
        }
        for c in aig.constraints.iter() {
            logic.push(*c);
        }
        logic.push(aig_bad);
        let trans = aig.get_optimized_cnf(&logic);
        // let trans = aig.get_cnf();
        for tran in trans.iter() {
            simp_solver.add_clause(tran);
        }
        for (l, p) in aig.latchs.iter().zip(primes.iter()) {
            let l = l.next.to_lit();
            let p = p.lit();
            simp_solver.add_clause(&Clause::from([l, !p]));
            simp_solver.add_clause(&Clause::from([!l, p]));
        }
        for c in constraints.iter() {
            simp_solver.add_clause(&Clause::from([*c]));
        }
        simp_solver.eliminate(true);
        let trans = simp_solver.clauses();

        let mut next_map = HashMap::new();
        let mut previous_map = HashMap::new();
        for (l, p) in latchs.iter().zip(primes.iter()) {
            next_map.insert(*l, *p);
            previous_map.insert(*p, *l);
        }
        Self {
            inputs,
            latchs,
            primes,
            init,
            constraints,
            bad,
            trans,
            num_var: simp_solver.num_var(),
            next_map,
            previous_map,
        }
    }

    #[inline]
    pub fn lit_previous(&self, lit: Lit) -> Lit {
        Lit::new(self.previous_map[&lit.var()], lit.polarity())
    }

    #[inline]
    pub fn lit_next(&self, lit: Lit) -> Lit {
        Lit::new(self.next_map[&lit.var()], lit.polarity())
    }

    pub fn cube_previous(&self, cube: &Cube) -> Cube {
        cube.iter().map(|l| self.lit_previous(*l)).collect()
    }

    pub fn cube_next(&self, cube: &Cube) -> Cube {
        cube.iter().map(|l| self.lit_next(*l)).collect()
    }

    pub fn cube_subsume_init(&self, x: &Cube) -> bool {
        for i in 0..x.len() {
            if let Some(init) = self.init.get(&x[i].var()) {
                if *init != x[i].polarity() {
                    return false;
                }
            }
        }
        true
    }

    pub fn load_trans(&self, solver: &mut Solver) {
        while solver.num_var() < self.num_var {
            solver.new_var();
        }
        for cls in self.trans.iter() {
            solver.add_clause(cls)
        }
    }
}
