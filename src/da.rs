use std::{
    collections::HashSet,
    fs::File,
    io::{Read, Write},
};

use aig::{Aig, AigEdge};
use logic_form::Cube;

use crate::{activity::Activity, Ic3};

impl Ic3 {
    pub fn save_da(&mut self) {
        dbg!(self.testda.len());
        self.testda.retain(|_, v| v.len() > 4);
        let mut sum = 0;
        let mut sumb = 0;
        let mut da = Vec::new();
        for t in self.testda.iter() {
            // dbg!(t.0);
            sum += 1;
            let mut c = Cube::from_iter(t.1.iter().copied());
            sumb += c.len();
            c.sort();
            // println!("{:?}", c);
            da.push(c);
        }
        dbg!(sum);
        dbg!(sumb);
        let json = serde_json::to_string(&da).unwrap();
        let mut file = File::create("da.json").unwrap();
        file.write_all(json.as_bytes()).unwrap();
    }

    pub fn read_da() -> Vec<Cube> {
        let mut file = File::open("da.json").unwrap();
        let mut json = String::new();
        file.read_to_string(&mut json).unwrap();
        serde_json::from_str(&json).unwrap()
    }
}

fn get_latch_next(aig: &Aig, e: AigEdge) -> (AigEdge, bool) {
    let latch = aig
        .latchs
        .iter()
        .find(|latch| latch.input == e.node_id())
        .unwrap();
    if e.compl() {
        (!latch.next, !latch.init.unwrap())
    } else {
        (latch.next, latch.init.unwrap())
    }
}

pub fn da_aig(mut aig: Aig, activity: &mut Activity) -> Aig {
    let mut da = Ic3::read_da();
    da.iter_mut().for_each(|da| da.sort());
    da = da.into_iter().collect::<HashSet<_>>().into_iter().collect();
    da.sort_by_key(|da| da.len());
    for dls in da.iter() {
        let mut sum = 0;
        for i in 0..da.len() {
            if dls.subsume(&da[i]) {
                sum += 1;
            }
        }
        if sum == 1 {
            continue;
        }
        let mut now = AigEdge::from_lit(dls[0]);
        let (mut now_next, mut now_init) = get_latch_next(&aig, now);
        for dl in dls[1..].iter() {
            let e = AigEdge::from_lit(*dl);
            let (e_next, e_init) = get_latch_next(&aig, e);
            now = aig.new_or_node(e, now);
            now_next = aig.new_or_node(e_next, now_next);
            now_init = now_init | e_init;
        }
        if now.compl() {
            now_next = !now_next;
            now_init = !now_init;
        } else {
            todo!();
        }
        dbg!(now.node_id());
        aig.new_latch(now.node_id(), now_next, Some(now_init));
        activity.pump_lit_activity_test(&now.to_lit());
    }
    aig
}
