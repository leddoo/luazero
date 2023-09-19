use sti::keyed::KVec;
use sti::hash::HashMap;

use crate::string_table::{Atom, atoms};
use crate::tt::*;
use crate::tt::inductive::InductiveInfo;


pub struct Env<'a> {
    symbols: KVec<SymbolId, Symbol<'a>>,
}


sti::define_key!(pub, u32, SymbolId, opt: OptSymbolId);


#[derive(Debug)]
pub struct Symbol<'a> {
    pub parent: SymbolId,

    pub kind: SymbolKind<'a>,
    pub name: Atom,

    pub children: HashMap<Atom, SymbolId>,
}

#[derive(Debug)]
pub enum SymbolKind<'a> {
    Root,
    Predeclared,
    Pending,
    IndAxiom(symbol::IndAxiom<'a>),
    Def(symbol::Def<'a>),
}


#[allow(non_upper_case_globals)]
impl SymbolId {
    pub const ROOT: SymbolId = SymbolId(0);

    pub const Nat: SymbolId = SymbolId(1);
    pub const Nat_zero: SymbolId = SymbolId(2);
    pub const Nat_succ: SymbolId = SymbolId(3);

    pub const Eq: SymbolId = SymbolId(4);

    pub const Add: SymbolId = SymbolId(5);
    pub const Add_add: SymbolId = SymbolId(6);

    pub const Unit: SymbolId = SymbolId(7);
    pub const Unit_mk: SymbolId = SymbolId(8);

    pub const Bool: SymbolId = SymbolId(9);
    pub const Bool_false: SymbolId = SymbolId(10);
    pub const Bool_true: SymbolId = SymbolId(11);
    pub const ite: SymbolId = SymbolId(12);

    pub const ax_sorry: SymbolId = SymbolId(13);
    pub const ax_uninit: SymbolId = SymbolId(14);
    pub const ax_unreach: SymbolId = SymbolId(15);
}


pub mod symbol {
    use super::*;

    #[derive(Clone, Copy, Debug, PartialEq)]
    pub enum IndAxiomKind {
        TypeFormer,
        Constructor(u32),
        Eliminator,
    }

    #[derive(Clone, Copy, Debug)]
    pub struct IndAxiom<'a> {
        pub kind: IndAxiomKind,
        pub info: &'a InductiveInfo<'a>,
        pub num_levels: u32,
        pub ty: Term<'a>,
        pub mutual_infos: &'a [InductiveInfo<'a>],
    }

    #[derive(Clone, Copy, Debug)]
    pub struct Def<'a> {
        pub num_levels: u32,
        pub ty:  Term<'a>,
        pub val: Option<Term<'a>>,
    }
}



impl<'a> Env<'a> {
    pub fn new() -> Env<'static> {
        let mut symbols = KVec::new();
        let root = symbols.push(Symbol {
            parent: SymbolId::ROOT,
            kind: SymbolKind::Root,
            name: Atom::NULL,
            children: HashMap::new(),
        });
        assert_eq!(root, SymbolId::ROOT);

        let mut env = Env { symbols };

        env.predeclare(SymbolId::ROOT, atoms::Nat, SymbolId::Nat);
        env.predeclare(SymbolId::Nat, atoms::zero, SymbolId::Nat_zero);
        env.predeclare(SymbolId::Nat, atoms::succ, SymbolId::Nat_succ);

        env.predeclare(SymbolId::ROOT, atoms::Eq, SymbolId::Eq);

        env.predeclare(SymbolId::ROOT, atoms::Add, SymbolId::Add);
        env.predeclare(SymbolId::Add, atoms::add, SymbolId::Add_add);

        env.predeclare(SymbolId::ROOT, atoms::Unit, SymbolId::Unit);
        env.predeclare(SymbolId::Unit, atoms::mk, SymbolId::Unit_mk);

        env.predeclare(SymbolId::ROOT, atoms::Bool, SymbolId::Bool);
        env.predeclare(SymbolId::Bool, atoms::_false, SymbolId::Bool_false);
        env.predeclare(SymbolId::Bool, atoms::_true, SymbolId::Bool_true);
        env.predeclare(SymbolId::ROOT, atoms::ite, SymbolId::ite);

        env.predeclare(SymbolId::ROOT, atoms::ax_sorry, SymbolId::ax_sorry);
        env.predeclare(SymbolId::ROOT, atoms::ax_uninit, SymbolId::ax_uninit);
        env.predeclare(SymbolId::ROOT, atoms::ax_unreach, SymbolId::ax_unreach);

        return env
    }

    #[inline]
    fn predeclare(&mut self, parent: SymbolId, name: Atom, symbol: SymbolId) {
        let id = self.symbols.push(Symbol {
            parent,
            kind: SymbolKind::Predeclared,
            name,
            children: HashMap::new(),
        });
        assert_eq!(id, symbol);

        let none = self.symbols[parent].children.insert(name, symbol);
        assert!(none.is_none());
    }


    #[inline(always)]
    pub fn new_symbol(&mut self, parent: SymbolId, name: Atom, kind: SymbolKind<'a>) -> Option<SymbolId> {
        let mut predeclared = None;
        if let Some(symbol) = self.lookup(parent, name) {
            if matches!(self.symbols[symbol].kind, SymbolKind::Predeclared) {
                predeclared = Some(symbol);
            }
            else { return None }
        }

        match &kind {
            SymbolKind::Root |
            SymbolKind::Predeclared => unreachable!(),

            SymbolKind::Pending => (),

            SymbolKind::IndAxiom(it) => {
                assert!(it.ty.closed_no_local_no_ivar());
            }

            SymbolKind::Def(it) => {
                assert!(it.ty.closed_no_local_no_ivar());
                if let Some(val) = it.val {
                    assert!(val.closed_no_local_no_ivar());
                }
            }
        }

        if let Some(symbol) = predeclared {
            self.symbols[symbol].kind = kind;
            return Some(symbol);
        }
        else {
            let id = self.symbols.push(Symbol {
                parent,
                kind,
                name,
                children: HashMap::new(),
            });

            self.symbols[parent].children.insert(name, id);

            return Some(id);
        }
    }

    #[inline(always)]
    pub fn symbol(&self, id: SymbolId) -> &Symbol<'a> {
        &self.symbols[id]
    }

    pub fn lookup(&self, parent: SymbolId, name: Atom) -> Option<SymbolId> {
        let p = &self.symbols[parent];
        p.children.get(&name).copied()
    }

    pub fn resolve_pending(&mut self, id: SymbolId, kind: SymbolKind<'a>) {
        match &kind {
            SymbolKind::Root |
            SymbolKind::Predeclared |
            SymbolKind::Pending => unreachable!(),

            SymbolKind::IndAxiom(it) => {
                assert!(it.ty.closed_no_local_no_ivar());
            }

            SymbolKind::Def(it) => {
                assert!(it.ty.closed_no_local_no_ivar());
                if let Some(val) = it.val {
                    assert!(val.closed_no_local_no_ivar());
                }
            }
        }

        let symbol = &mut self.symbols[id];
        assert!(matches!(symbol.kind, SymbolKind::Pending));
        symbol.kind = kind;
    }
}

