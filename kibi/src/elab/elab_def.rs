use sti::arena_pool::ArenaPool;

use crate::ast::{self, *};
use crate::tt::*;

use super::*;


impl<'me, 'c, 'out> Elaborator<'me, 'c, 'out> {
    pub fn elab_axiom(&mut self, item_id: ItemId, axiom: &item::Axiom) -> Option<SymbolId> {
        spall::trace_scope!("kibi/elab/axiom"; "{}",
            axiom.name.display(self.strings));

        assert_eq!(self.locals.len(), 0);
        assert_eq!(self.level_params.len(), 0);

        let temp = ArenaPool::tls_get_rec();

        for level in axiom.levels {
            self.level_params.push(level.value);
        }

        let locals = self.elab_binders(axiom.params, &*temp);

        // type.
        let mut ty = self.elab_expr_as_type(axiom.ty).0;

        assert_eq!(self.locals.len(), locals.len());
        for (_, id) in self.locals.iter().copied().rev() {
            ty = self.mk_binder(ty,  id, true);
        }

        if self.locals.len() == 0 {
            ty = self.instantiate_term_vars(ty);
        }

        debug_assert!(ty.syntax_eq(self.instantiate_term_vars(ty)));

        let (parent, name) = match &axiom.name {
            IdentOrPath::Ident(name) => (self.root_symbol, *name),

            IdentOrPath::Path(path) => {
                let (name, parts) = path.parts.split_last().unwrap();
                let parent = self.elab_path(parts)?;
                (parent, *name)
            }
        };

        if !ty.closed() || ty.has_locals() {
            let mut pp = TermPP::new(self.env, &self.strings, &self.lctx, self.alloc);
            let ty  = pp.pp_term(self.instantiate_term_vars(ty));
            eprintln!("{}", pp.render(ty,  50).layout_string());
        }

        assert!(ty.closed());

        assert!(!ty.has_locals());

        if ty.has_ivars() {
            eprintln!("unresolved inference variables");
            let mut pp = TermPP::new(self.env, &self.strings, &self.lctx, self.alloc);
            let ty  = self.instantiate_term_vars(ty);
            let ty  = pp.pp_term(ty);
            eprintln!("{}", pp.render(ty,  50).layout_string());
            //return None;
        }

        let _ = self.check_no_unassigned_variables(item_id.into())?;

        let symbol = self.env.new_symbol(parent, name.value,
            SymbolKind::Def(symbol::Def {
                num_levels: axiom.levels.len() as u32,
                ty,
                val: None,
            })
        )?;

        Some(symbol)
    }

    pub fn elab_def_core(&mut self, item_id: ItemId, levels: &[Ident], params: &[ast::Binder], ty: OptExprId, value: ExprId) -> Option<(Term<'out>, Term<'out>)> {
        assert_eq!(self.locals.len(), 0);
        assert_eq!(self.level_params.len(), 0);

        let temp = ArenaPool::tls_get_rec();

        for level in levels {
            self.level_params.push(level.value);
        }

        let locals = self.elab_binders(params, &*temp);

        // type.
        let ty =
            if let Some(t) = ty.to_option() {
                Some(self.elab_expr_as_type(t).0)
            }
            else { None };

        // value.
        let (mut val, val_ty) = self.elab_expr_checking_type(value, ty);


        let mut ty = ty.unwrap_or(val_ty);

        assert_eq!(self.locals.len(), locals.len());
        for (_, id) in self.locals.iter().copied().rev() {
            ty  = self.mk_binder(ty,  id, true);
            val = self.mk_binder(val, id, false);
        }

        if self.locals.len() == 0 {
            ty  = self.instantiate_term_vars(ty);
            val = self.instantiate_term_vars(val);
        }

        debug_assert!(ty.syntax_eq(self.instantiate_term_vars(ty)));
        debug_assert!(val.syntax_eq(self.instantiate_term_vars(val)));

        if !ty.closed() || !val.closed() || ty.has_locals() || val.has_locals() {
            let mut pp = TermPP::new(self.env, &self.strings, &self.lctx, self.alloc);
            let ty  = pp.pp_term(self.instantiate_term_vars(ty));
            let val = pp.pp_term(self.instantiate_term_vars(val));
            eprintln!("{}", pp.render(ty,  50).layout_string());
            eprintln!("{}", pp.render(val, 50).layout_string());
        }

        assert!(ty.closed());
        assert!(val.closed());

        assert!(!ty.has_locals());
        assert!(!val.has_locals());

        if ty.has_ivars() || val.has_ivars() {
            eprintln!("unresolved inference variables");
            let mut pp = TermPP::new(self.env, &self.strings, &self.lctx, self.alloc);
            let ty  = self.instantiate_term_vars(ty);
            let val = self.instantiate_term_vars(val);
            let ty  = pp.pp_term(ty);
            let val = pp.pp_term(val);
            eprintln!("{}", pp.render(ty,  50).layout_string());
            eprintln!("{}", pp.render(val, 50).layout_string());
        }

        let _ = self.check_no_unassigned_variables(item_id.into())?;

        Some((ty, val))
    }

    pub fn elab_def(&mut self, item_id: ItemId, def: &item::Def) -> Option<SymbolId> {
        spall::trace_scope!("kibi/elab/def"; "{}",
            def.name.display(self.strings));

        eprintln!("!!! {}", def.name.display(self.strings));

        let (parent, name) = match def.name {
            IdentOrPath::Ident(name) => (self.root_symbol, name),

            IdentOrPath::Path(path) => {
                let (name, parts) = path.parts.split_last().unwrap();
                let parent = self.elab_path(parts)?;
                (parent, *name)
            }
        };

        let Some((ty, val)) = self.elab_def_core(item_id, def.levels, def.params, def.ty, def.value) else {
            return self.env.new_symbol(parent, name.value, SymbolKind::Error);
        };

        let symbol = self.env.new_symbol(parent, name.value,
            SymbolKind::Def(symbol::Def {
                num_levels: def.levels.len() as u32,
                ty,
                val: Some(val),
            })
        )?;

        let none = self.elab.token_infos.insert(name.source, TokenInfo::Symbol(symbol));
        debug_assert!(none.is_none());

        Some(symbol)
    }
}

