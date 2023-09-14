use sti::arena_pool::ArenaPool;
use sti::traits::{FromIn, CopyIt};

use crate::ast::{ItemId, adt::Inductive};
use crate::tt::*;

use super::*;


impl<'me, 'c, 'out> Elaborator<'me, 'c, 'out> {
    pub fn elab_inductive(&mut self, item_id: ItemId, ind: &Inductive) -> Option<SymbolId> {
        assert_eq!(self.locals.len(), 0);
        assert_eq!(self.level_params.len(), 0);

        let temp = ArenaPool::tls_get_rec();

        for level in ind.levels {
            self.level_params.push(*level);
        }

        let ind_symbol = self.env.new_symbol(self.root_symbol,
            ind.name, SymbolKind::Pending)?;

        // check params.
        let params = self.elab_binders(ind.params, &*temp)?;

        // check type.
        let type_former =
            if let Some(ty) = ind.ty.to_option() {
                self.elab_expr_as_type(ty)?.0
            }
            else { Term::SORT_1 };

        let has_indices = self.whnf_forall(type_former).is_some();

        // check type former has no ivars.
        if type_former.has_ivars() {
            // @todo: better source, context.
            self.error(item_id, ElabError::TypeFormerHasIvars);
            return None;
        }

        // check params have no ivars.
        for (param, ty, _) in params.iter().copied() {
            let ty = self.instantiate_term_vars(ty);
            if ty.has_ivars() {
                // @todo: better source, context.
                self.error(item_id, ElabError::TypeFormerHasIvars);
                return None;
            }
            self.lctx.lookup_mut(param).ty = ty;
        }


        // create a local for the inductive type,
        // for the idents in the ctors to bind to.
        let ind_local_id =
            self.lctx.push(BinderKind::Explicit, ind.name, type_former, None);
        self.locals.push((ind.name, ind_local_id));

        let ind_local = self.alloc.mkt_local(ind_local_id);

        // elab ctors.
        let mut ctors = Vec::with_cap_in(&*temp, ind.ctors.len());
        for ctor in ind.ctors {
            let args = self.elab_binders(&ctor.args, &*temp)?;

            let mut ty = match ctor.ty.to_option() {
                Some(ty) => self.elab_expr_as_type(ty)?.0,
                None => {
                    if has_indices {
                        // @todo: better source.
                        self.error(item_id, ElabError::CtorNeedsTypeCauseIndices);
                        return None;
                    }
                    ind_local
                }
            };

            for (arg, _, _) in args.iter().rev().copied() {
                ty = self.mk_binder(ty, arg, true);
                self.lctx.pop(arg);
            }
            self.locals.truncate(self.locals.len() - args.len());


            let ty = self.instantiate_term_vars(ty);

            if ty.has_ivars() {
                // @todo: better source.
                self.error(item_id, ElabError::CtorTypeHasIvars);
                return None;
            }

            let symbol = self.env.new_symbol(ind_symbol, ctor.name, SymbolKind::Pending)?;
            ctors.push(inductive::CtorSpec { name: ctor.name, symbol, ty });
        }

        let param_ids = Vec::from_in(&*temp,
            params.copy_map_it(|(id, _, _)| id));

        let rec_symbol = self.env.new_symbol(ind_symbol, atoms::rec, SymbolKind::Pending)?;

        // check spec.
        let spec = inductive::MutualSpec {
            temp_source: item_id,
            levels: ind.levels,
            params: &param_ids,
            types: &[
                inductive::TypeSpec {
                    name: ind.name,
                    symbol: ind_symbol,
                    local: ind_local_id,
                    ctors: &ctors,
                    rec_symbol,
                }
            ],
        };
        inductive::Check::check(self, spec)?;

        return Some(ind_symbol);
    }
}

