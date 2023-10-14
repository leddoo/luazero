use sti::traits::CopyIt;
use sti::arena_pool::ArenaPool;
use sti::vec::Vec;
use sti::keyed::{Key, KVec};

use crate::bit_set::{BitSet, BitSetMut};
use crate::bit_relation::BitRelation;
use crate::string_table::StringTable;
use crate::ast::expr::RefKind;
use crate::env::Env;

use super::*;


pub fn borrow_check<'a>(func: Function<'a>, env: &Env<'a>, strings: &StringTable, alloc: &'a Arena) -> Result<(), ()> {
    let temp = ArenaPool::tls_get_temp();

    let mut this = BrCk {
        alloc,
        env,
        strings,
        temp: &*temp,
        func,
        ref_infos: KVec::with_cap(func.vars.len()),
        block_infos: KVec::with_cap(func.blocks.len()),
        next_region: RegionId::ZERO,
        region_subsets: BitRelation::default(),
    };


    // count regions.
    for (_, var) in this.func.vars.iter() {
        // @todo: count regions in reduced type.
        if let Some([_r, kind, _ty]) = var.ty.try_ref_app() {
            // @todo: whnf.
            let Some(kind) = kind.try_global() else {
                return Err(());
            };

            let kind = match kind.id {
                SymbolId::Ref_Kind_mut   => RefKind::Mut,
                SymbolId::Ref_Kind_shr   => RefKind::Shared,
                SymbolId::Ref_Kind_const => RefKind::Const,
                _ =>
                    return Err(())
            };

            this.ref_infos.push(RefInfo {
                num_regions: 1,
                kind: Some(kind),
            });
        }
        else {
            this.ref_infos.push(RefInfo {
                num_regions: 0,
                kind: None,
            });
        }
    }
    dbg!(&this.ref_infos);


    // create region vars.
    for (_, block) in this.func.blocks.iter() {
        //let mut entry_regions = Vec::with_cap_in(this.temp, this.ref_infos.len());
        let mut entry_regions = Vec::new_in(this.temp);
        for var in block.vars_entry.iter() {
            let info = &this.ref_infos[var];

            let mut regions = Vec::with_cap_in(this.temp, info.num_regions as usize);
            for _ in 0..info.num_regions {
                regions.push(this.next_region);
                this.next_region = this.next_region.add(1).unwrap();
            }
            entry_regions.push(&*regions.leak());
        }
        // @todo: sti vec shrink to fit.
        //entry_regions.shrink_to_fit();

        let mut ref_regions = Vec::new_in(this.temp);
        for stmt in block.stmts {
            if let Stmt::Ref(_) = stmt {
                ref_regions.push(&*sti::vec_in!(this.temp; this.next_region).leak());
                this.next_region = this.next_region.add(1).unwrap();
            }
        }
        // @todo: sti vec shrink to fit.
        //ref_regions.shrink_to_fit();

        this.block_infos.push(BlockInfo {
            entry_regions: entry_regions.leak(),
            ref_regions: ref_regions.leak(),
        });
    }
    dbg!(&this.block_infos);


    // compute region subsets.
    let mut stack = Vec::with_cap(32);
    let mut latest_var_regions = KVec::with_cap(this.func.vars.len());
    let mut subset_builder = Vec::with_cap(2*this.ref_infos.len());
    for (bb, block) in this.func.blocks.iter() {
        let info = &this.block_infos[bb];

        println!("{}:\n{}", bb, block);

        assert_eq!(stack.len(), 0);

        // init latest var regions.
        // @todo: clear, resize.
        latest_var_regions.truncate(0);
        for _ in 0..this.func.vars.len() {
            latest_var_regions.push(&[][..]);
        }
        for (i, var) in block.vars_entry.iter().enumerate() {
            latest_var_regions[var] = info.entry_regions[i];
        }

        let mut ref_region = 0;
        for stmt in block.stmts {
            match *stmt {
                Stmt::Error |
                Stmt::Axiom |
                Stmt::Const(_) |
                Stmt::ConstUnit |
                Stmt::ConstBool(_) |
                Stmt::ConstNat(_) => stack.push(&[][..]),

                Stmt::Pop => { stack.pop().unwrap(); }

                Stmt::Ref(_) => {
                    let regions = info.ref_regions[ref_region];
                    ref_region += 1;
                    stack.push(regions);
                }

                Stmt::Read(path) => {
                    // @todo: proper region.
                    stack.push(&[][..]);
                    _ = path;
                }

                Stmt::Write(path) => {
                    let regions = stack.pop().unwrap();
                    if path.projs.len() == 0 {
                        latest_var_regions[path.base] = regions;
                    }
                }

                Stmt::Call { func: _, num_args } => {
                    // @todo: pop_n?
                    stack.truncate(stack.len() - num_args);
                    stack.push(&[][..]);
                }
            }
        }

        dbg!((bb, &latest_var_regions));

        let mut add_succ_constraints = |succ: BlockId| {
            for (i, var) in this.func.blocks[succ].vars_entry.iter().enumerate() {
                let exit_regions = latest_var_regions[var];
                let succ_regions = this.block_infos[succ].entry_regions[i];
                assert_eq!(exit_regions.len(), succ_regions.len());

                for (r1, r2) in exit_regions.copy_it().zip(succ_regions.copy_it()) {
                    subset_builder.push((r1, r2));
                }
            }
        };

        match block.terminator {
            Terminator::Jump { target } => {
                add_succ_constraints(target);
            }

            Terminator::Ite { on_true, on_false } => {
                stack.pop().unwrap();

                add_succ_constraints(on_true);
                add_succ_constraints(on_false);
            }

            Terminator::Return => {
                stack.clear();
            }
        }
    }
    drop((stack, latest_var_regions));
    dbg!(&subset_builder);
    this.region_subsets = BitRelation::transitive_from(this.temp, this.next_region.usize(), &subset_builder);
    eprint!("subsets: ");
    for i in 0..this.next_region.usize() {
        for j in 0..this.next_region.usize() {
            if this.region_subsets.has(RegionId::from_usize_unck(i), RegionId::from_usize_unck(j)) {
                eprint!("r{i}<:r{j}, ");
            }
        }
    }
    eprintln!();


    // liveness.
    #[derive(Debug)]
    struct BlockLiveInfo<'a> {
        succs: &'a [BlockId],
        preds: Vec<BlockId, &'a Arena>,
        gen:  BitSetMut<'a, LocalVarId>,
        kill: BitSetMut<'a, LocalVarId>,
        live_in:  BitSetMut<'a, LocalVarId>,
        live_out: BitSetMut<'a, LocalVarId>,
        queued: bool,
    }
    let mut blocks = KVec::with_cap(this.func.blocks.len());
    for _ in 0..this.func.blocks.len() {
        blocks.push(BlockLiveInfo {
            succs: &[],
            preds: Vec::with_cap_in(this.temp, 2),
            gen:  BitSetMut::new(this.temp, this.func.vars.len()),
            kill: BitSetMut::new(this.temp, this.func.vars.len()),
            live_in:  BitSetMut::new(this.temp, this.func.vars.len()),
            live_out: BitSetMut::new(this.temp, this.func.vars.len()),
            queued: false,
        });
    }
    for (bb, block) in this.func.blocks.iter() {
        let info = &mut blocks[bb];

        for stmt in block.stmts {
            match *stmt {
                Stmt::Error |
                Stmt::Axiom |
                Stmt::Const(_) |
                Stmt::ConstUnit |
                Stmt::ConstBool(_) |
                Stmt::ConstNat(_) |
                Stmt::Pop |
                Stmt::Ref(_) |
                Stmt::Call { func: _, num_args: _ } => (),

                Stmt::Read(path) => {
                    if path.projs.len() == 0 {
                        if !info.kill.has(path.base) {
                            info.gen.insert(path.base);
                        }
                    }
                }

                Stmt::Write(path) => {
                    if path.projs.len() == 0 {
                        info.kill.insert(path.base);
                    }
                }

            }
        }

        match block.terminator {
            Terminator::Jump { target } => {
                info.succs = sti::vec_in!(this.temp; target).leak();
                blocks[target].preds.push(bb);
            }

            Terminator::Ite { on_true, on_false } => {
                info.succs = sti::vec_in!(this.temp; on_true, on_false).leak();
                blocks[on_true].preds.push(bb);
                blocks[on_false].preds.push(bb);
            }

            Terminator::Return => (),
        };
    }

    let mut worklist = Vec::with_cap(this.func.blocks.len());
    // @todo: sti kslice range, iter_mut.
    // @speed: reverse post order. should be able to compute ad-hoc using cursor, queued & terminator match.
    for (bb, _) in this.func.blocks.iter() {
        let info = &mut blocks[bb];
        info.live_in.assign(info.gen.borrow());
        info.queued = true;
        worklist.push(bb);
    }

    while let Some(bb) = worklist.pop() {
        let block = &mut blocks[bb];
        block.queued = false;

        // propagate successor live_in.
        let mut live_out = core::mem::take(&mut block.live_out);
        for succ in block.succs.copy_it() {
            live_out.union(blocks[succ].live_in.borrow());
        }

        let block = &mut blocks[bb];
        block.live_out = live_out;
        let changed =
            block.live_in.diff_union(
                block.live_out.borrow(), block.kill.borrow(),
                block.gen.borrow());

        if changed {
            for pred in blocks[bb].preds.copy_it() {
                if !blocks[pred].queued {
                    worklist.push(pred);
                }
            }
        }
    }
    dbg!(&blocks);


    Err(())
}


struct BrCk<'me, 'a> {
    alloc: &'a Arena,
    env: &'me Env<'a>,
    #[allow(dead_code)] strings: &'me StringTable<'me>,

    temp: &'me Arena,

    func: Function<'a>,

    ref_infos: KVec<LocalVarId, RefInfo>,
    block_infos: KVec<BlockId, BlockInfo<'me>>,
    next_region: RegionId,

    region_subsets: BitRelation<'me, RegionId>,
}


#[derive(Debug)]
struct RefInfo {
    num_regions: u32,
    kind: Option<RefKind>,
}


sti::define_key!(u32, RegionId);

#[derive(Debug)]
struct BlockInfo<'a> {
    entry_regions: &'a [&'a [RegionId]],
    ref_regions: &'a [&'a [RegionId]],
}


impl<'me, 'a> BrCk<'me, 'a> {
}
