use crate::command::Args;
use crate::frames::Lemma;
use crate::model::Model;
use crate::Ic3;
use std::cmp::Ordering;
use std::collections::BTreeSet;

pub struct BasicShare {
    pub args: Args,
    pub model: Model,
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct ProofObligation {
    pub frame: usize,
    pub lemma: Lemma,
    pub depth: usize,
}

impl ProofObligation {
    pub fn new(frame: usize, lemma: Lemma, depth: usize) -> Self {
        Self {
            frame,
            lemma,
            depth,
        }
    }
}

impl PartialOrd for ProofObligation {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ProofObligation {
    fn cmp(&self, other: &Self) -> Ordering {
        match other.frame.cmp(&self.frame) {
            Ordering::Equal => match other.depth.cmp(&self.depth) {
                Ordering::Equal => match other.lemma.len().cmp(&self.lemma.len()) {
                    Ordering::Equal => other.lemma.cmp(&self.lemma),
                    ord => ord,
                },
                ord => ord,
            },
            ord => ord,
        }
    }
}

#[derive(Default, Debug)]
pub struct ProofObligationQueue {
    pub obligations: BTreeSet<ProofObligation>,
    num: Vec<usize>,
}

impl ProofObligationQueue {
    pub fn new() -> Self {
        Self::default()
    }

    fn add(&mut self, po: ProofObligation) {
        if self.num.len() <= po.frame {
            self.num.resize(po.frame + 1, 0);
        }
        self.num[po.frame] += 1;
        if !self.obligations.insert(po.clone()) {
            self.num[po.frame] -= 1;
            // dbg!(po);
            // panic!();
        }
    }

    pub fn pop(&mut self, depth: usize) -> Option<ProofObligation> {
        if let Some(po) = self.obligations.last().filter(|po| po.frame <= depth) {
            self.num[po.frame] -= 1;
            self.obligations.pop_last()
        } else {
            None
        }
    }

    pub fn statistic(&self) {
        println!("{:?}", self.num);
    }
}

impl Ic3 {
    pub fn add_obligation(&mut self, po: ProofObligation) {
        self.statistic.average_po_cube_len += po.lemma.len();
        self.obligations.add(po)
    }
}
