use sti::arena::Arena;
use sti::keyed::KVec;

use super::syntax::*;


// @todo: debug version with (global) generational indices.
sti::define_key!(u32, pub ScopeId, opt: OptScopeId);


pub struct LocalCtx<'a> {
    alloc: &'a Arena,

    scopes: KVec<ScopeId, Scope<'a>>,
    current: OptScopeId,
}

pub struct Scope<'a> {
    pub parent: OptScopeId,
    pub ty:    TermRef<'a>,
    pub value: Option<TermRef<'a>>,
}

#[derive(Clone)]
pub struct SavePoint {
    num_scopes: usize,
    current: OptScopeId,
}


impl<'a> LocalCtx<'a> {
    #[inline(always)]
    pub fn new(alloc: &'a Arena) -> Self {
        Self {
            alloc,
            scopes: KVec::new(),
            current: None.into(),
        }
    }


    #[track_caller]
    pub fn push(&mut self, ty: TermRef<'a>, value: Option<TermRef<'a>>) -> ScopeId {
        assert!(ty.closed());
        if let Some(v) = value { assert!(v.closed()); }

        let parent = self.current;
        let id = self.scopes.push(Scope { parent, ty, value });
        self.current = id.some();
        id
    }

    #[track_caller]
    #[inline(always)]
    pub fn pop(&mut self, id: ScopeId) {
        assert_eq!(self.current, id.some());
        self.current = self.scopes[id].parent;
    }

    #[track_caller]
    #[inline(always)]
    pub fn lookup(&self, id: ScopeId) -> &Scope<'a> {
        &self.scopes[id]
    }


    #[track_caller]
    #[inline(always)]
    pub fn abstract_forall(&self, ret: TermRef<'a>, id: ScopeId) -> TermRef<'a> {
        // @temp: binder name.
        let entry = self.lookup(id);
        let ret = ret.abstracc(id, self.alloc);
        self.alloc.mkt_forall(0, entry.ty, ret)
    }

    #[track_caller]
    #[inline(always)]
    pub fn abstract_lambda(&self, value: TermRef<'a>, id: ScopeId) -> TermRef<'a> {
        // @temp: binder name.
        let entry = self.lookup(id);
        let value = value.abstracc(id, self.alloc);
        self.alloc.mkt_lambda(0, entry.ty, value)
    }


    #[inline(always)]
    pub fn save(&self) -> SavePoint {
        SavePoint {
            num_scopes: self.scopes.len(),
            current:    self.current,
        }
    }

    #[track_caller]
    #[inline(always)]
    pub fn restore(&mut self, save: SavePoint) {
        assert!(save.num_scopes <= self.scopes.len());
        // @temp: sti kvec truncate.
        unsafe { self.scopes.inner_mut().truncate(save.num_scopes) }
        self.current = save.current;
    }
}

