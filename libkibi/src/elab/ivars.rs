use crate::tt::*;
use local_ctx::OptScopeId;

use super::*;


sti::define_key!(u32, pub LevelVarId);

pub struct LevelVar<'a> {
    value: Option<Level<'a>>,
}


sti::define_key!(u32, pub TermVarId);

pub struct TermVar<'a> {
    scope: OptScopeId,
    ty: Term<'a>,
    value: Option<Term<'a>>,
}


impl<'me, 'err, 'a> Elab<'me, 'err, 'a> {
    pub fn new_level_var_id(&mut self) -> LevelVarId {
        self.level_vars.push(LevelVar {
            value: None,
        })
    }

    pub fn new_level_var(&mut self) -> Level<'a> {
        let id = self.new_level_var_id();
        self.alloc.mkl_ivar(id)
    }


    pub fn new_term_var_id(&mut self, ty: Term<'a>, scope: OptScopeId) -> TermVarId {
        self.term_vars.push(TermVar {
            scope,
            ty,
            value: None,
        })
    }

    pub fn new_term_var_core(&mut self, ty: Term<'a>, scope: OptScopeId) -> Term<'a> {
        let id = self.new_term_var_id(ty, scope);
        self.alloc.mkt_ivar(id)
    }

    pub fn new_term_var_of_type(&mut self, ty: Term<'a>) -> Term<'a> {
        self.new_term_var_core(ty, self.lctx.current())
    }

    pub fn new_term_var(&mut self) -> (Term<'a>, Term<'a>) {
        let l = self.new_level_var();
        let tyty = self.alloc.mkt_sort(l);
        let ty = self.new_term_var_core(tyty, self.lctx.current());
        (self.new_term_var_core(ty, self.lctx.current()), ty)
    }

    pub fn new_ty_var(&mut self) -> (Term<'a>, Level<'a>) {
        let l = self.new_level_var();
        let ty = self.alloc.mkt_sort(l);
        (self.new_term_var_core(ty, self.lctx.current()), l)
    }
}


impl LevelVarId {
    #[inline(always)]
    pub fn value<'a>(self, elab: &Elab<'_, '_, 'a>) -> Option<Level<'a>> {
        elab.level_vars[self].value
    }


    #[track_caller]
    #[inline]
    pub unsafe fn assign_core<'a>(self, value: Level<'a>, elab: &mut Elab<'_, '_, 'a>) {
        debug_assert!(self.value(elab).is_none());
        elab.level_vars[self].value = Some(value);
    }

    #[track_caller]
    #[must_use]
    pub fn assign<'a>(self, value: Level<'a>, elab: &mut Elab<'_, '_, 'a>) -> bool {
        let value = elab.instantiate_level_vars(value);

        // occurs check.
        if value.find(|at| Some(at.try_ivar()? == self)).is_some() {
            return false;
        }

        unsafe { self.assign_core(value, elab) }
        return true;
    }

}

impl TermVarId {
    #[inline(always)]
    pub fn scope(self, elab: &Elab) -> OptScopeId {
        elab.term_vars[self].scope
    }

    #[inline(always)]
    pub fn ty<'a>(self, elab: &Elab<'_, '_, 'a>) -> Term<'a> {
        elab.term_vars[self].ty
    }

    #[inline(always)]
    pub fn value<'a>(self, elab: &Elab<'_, '_, 'a>) -> Option<Term<'a>> {
        elab.term_vars[self].value
    }


    #[track_caller]
    #[inline]
    pub unsafe fn assign_core<'a>(self, value: Term<'a>, elab: &mut Elab<'_, '_, 'a>) {
        debug_assert!(self.value(elab).is_none());
        debug_assert!(value.closed());
        debug_assert!(elab.lctx.all_locals_in_scope(value, self.scope(elab)));
        debug_assert!(elab.all_term_vars_in_scope(value, self.scope(elab)));
        elab.term_vars[self].value = Some(value);
    }

    // process `var(args) := value`
    #[must_use]
    pub fn assign<'a>(self, args: &[ScopeId], mut value: Term<'a>, elab: &mut Elab<'_, '_, 'a>) -> Option<bool> {
        //println!("{:?}({:?}) := {:?}", var, args, value);

        // abstract out `args`.
        for arg in args {
            value = elab.lctx.abstract_lambda(value, *arg);
        }

        let Some(value) = elab.check_value_for_assign(value, self) else {
            return (args.len() == 0).then_some(false);
        };

        if args.len() > 0 {
            // type correct check.
            //println!("@todo: check lambda type correct");
        }

        // type check.
        let var_ty = self.ty(elab);
        let value_ty = elab.infer_type(value).unwrap();
        if !elab.def_eq(var_ty, value_ty) {
            println!("type check failed");
            println!("{:?}", var_ty);
            println!("{:?}", value_ty);
            return Some(false);
        }

        unsafe { self.assign_core(value, elab) }
        return Some(true);
    }
}


impl<'me, 'err, 'a> Elab<'me, 'err, 'a> {
    pub fn term_var_in_scope(&self, var: TermVarId, scope: OptScopeId) -> bool {
        self.lctx.scope_is_prefix(var.scope(self), scope)
    }

    pub fn all_term_vars_in_scope(&self, t: Term<'a>, scope: OptScopeId) -> bool {
        t.find(|at, _| {
            if let TermData::IVar(var) = at.data() {
                return Some(!self.term_var_in_scope(var, scope));
            }
            None
        }).is_none()
    }

    fn check_value_for_assign(&mut self, value: Term<'a>, var: TermVarId) -> Option<Term<'a>> {
        Some(match value.data() {
            TermData::Local(id) => {
                // scope check.
                let scope = var.scope(self);
                if !self.lctx.local_in_scope(id, scope) {
                    println!("scope check failed (for local)");
                    return None;
                }

                value
            }

            TermData::IVar(other) => {
                // instantiate:
                if let Some(value) = other.value(self) {
                    return self.check_value_for_assign(value, var);
                }

                // occurs check.
                if other == var {
                    println!("occurs check failed");
                    return None;
                }

                // scope check.
                if !self.term_var_in_scope(other, var.scope(self)) {
                    // scope approx.
                    println!("scope check failed (for ivar)");
                    println!("@todo");
                }

                value
            }

            TermData::Forall(b) |
            TermData::Lambda(b) =>
                b.update(value, self.alloc,
                    self.check_value_for_assign(b.ty,  var)?,
                    self.check_value_for_assign(b.val, var)?),

            TermData::Apply(a) =>
                a.update(value, self.alloc,
                    self.check_value_for_assign(a.fun, var)?,
                    self.check_value_for_assign(a.arg, var)?),

            TermData::Sort(_)   | TermData::Bound(_) | TermData::Global(_) |
            TermData::Nat       | TermData::NatZero  | TermData::NatSucc   |
            TermData::NatRec(_) | TermData::Eq(_)    | TermData::EqRefl(_) |
            TermData::EqRec(_, _) =>
                value,
        })
    }
}

