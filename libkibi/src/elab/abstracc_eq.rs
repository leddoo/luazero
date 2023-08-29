use crate::tt::*;

use super::*;


impl<'me, 'err, 'a> Elab<'me, 'err, 'a> {
    pub fn abstract_eq(&mut self, t: Term<'a>, pat: Term<'a>) -> Term<'a> {
        let pat = self.instantiate_term_vars(pat);

        if let Some(local) = pat.try_local() {
            t.abstracc(local, self.alloc)
        }
        else {
            //println!("WARN: may not work");
            if 0==1 {
                let val = self.instantiate_term_vars(t);
                let mut pp = TermPP::new(&self.env, &self.strings, self.alloc);
                let val = pp.pp_term(val);
                let val = pp.render(val, 50);
                let val = val.layout_string();
                println!("t: {}", val);
            }
            if 0==1 {
                let val = self.instantiate_term_vars(pat);
                let mut pp = TermPP::new(&self.env, &self.strings, self.alloc);
                let val = pp.pp_term(val);
                let val = pp.render(val, 50);
                let val = val.layout_string();
                println!("pat: {}", val);
            }
            self.abstract_def_eq(t, pat)
        }
    }

    fn abstract_def_eq(&mut self, t: Term<'a>, pat: Term<'a>) -> Term<'a> {
        t.replace(self.alloc, |at, offset, alloc| {
            if at.syntax_eq(pat) {
                return Some(alloc.mkt_bound(BVar { offset }));
            }
            None
        })
    }
}

