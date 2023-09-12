use sti::traits::CopyIt;
use sti::arena::Arena;
use sti::vec::Vec;
use sti::string::String;
use sti::boks::Box;
use sti::rc::Rc;
use sti::keyed::{KVec, KFreeVec};
use sti::hash::HashMap;

use crate::string_table::StringTable;
use crate::error::ErrorCtx;
use crate::ast::{SourceId, ParseId, Parse, SourceRange};
use crate::parser;
use crate::vfs::Vfs;


pub struct Compiler {
    #[allow(dead_code)]
    persistent: Box<Arena>,

    inner: Inner<'static>,
}

struct Inner<'c> {
    vfs: Rc<dyn Vfs>,

    path_to_source: HashMap<String, SourceId>,
    sources: KVec<SourceId, Source>,
    parses: KVec<ParseId, OptParseDataId>,
    parse_datas: KFreeVec<ParseDataId, ParseData>,

    strings: StringTable<'c>,
    errors: ErrorCtx<'c>,
}

struct Source {
    dirty: bool,
    path: String,
    data: Vec<u8>,
    parses: Vec<ParseId>,
}


sti::define_key!(u32, ParseDataId, opt: OptParseDataId);

struct ParseData {
    #[allow(dead_code)]
    arena: Arena,

    parse: Parse<'static>,
}

impl Compiler {
    pub fn new(vfs: &Rc<impl 'static + Vfs>) -> Self {
        let persistent = Box::new(Arena::new());

        let inner = Inner {
            vfs: unsafe { vfs.clone().cast(|p| p as *mut sti::rc::RcInner<dyn Vfs>) },
            path_to_source: HashMap::new(),
            sources: KVec::new(),
            parses: KVec::new(),
            parse_datas: KFreeVec::new(),
            strings: StringTable::new(&*persistent),
            errors: ErrorCtx::new(&*persistent),
        };
        let inner = unsafe { core::mem::transmute(inner) };

        Self { persistent, inner }
    }

    pub fn add_source(&mut self, path: &str) -> SourceId {
        self.inner.add_source(path)
    }

    pub fn file_changed(&mut self, path: &str) {
        self.inner.file_changed(path)
    }

    pub fn update(&mut self) {
        self.inner.update()
    }
}

impl<'c> Inner<'c> {
    pub fn add_source(&mut self, path: &str) -> SourceId {
        *self.path_to_source.get_or_insert_with_key(path, |_| {
            let source = Source {
                dirty: true,
                path: path.into(),
                data: Vec::new(),
                parses: Vec::new(),
            };
            (path.into(), self.sources.push(source))
        })
    }

    pub fn file_changed(&mut self, path: &str) {
        if let Some(id) = self.path_to_source.get(path) {
            self.sources[*id].dirty = true;
        }
        else {
            self.add_source(path);
        }
    }


    pub fn update(&mut self) {
        for id in self.sources.range() {
            self.update_source(id);
        }
    }

    fn update_source(&mut self, source_id: SourceId) {
        let source = &mut self.sources[source_id];
        if !source.dirty {
            return;
        }


        source.data = match self.vfs.read(&source.path) {
            Ok(data) => {
                if source.data.len() > u32::MAX as usize {
                    // @todo: error.
                    return;
                }
                data
            }

            Err(_) => {
                // @todo: error.
                return;
            }
        };


        for parse_id in source.parses.copy_it() {
            let data_id = self.parses[parse_id].take().unwrap();
            self.parse_datas.free(data_id);
        }
        source.parses.clear();


        let arena = Arena::new();

        let parse_id = self.parses.next_key();

        let mut parse = Parse {
            source: source_id,
            source_range: SourceRange {
                begin: 0,
                end: source.data.len() as u32,
            },
            numbers: KVec::new(),
            strings: KVec::new(),
            tokens:  KVec::new(),
            items:  KVec::new(),
            levels: KVec::new(),
            exprs:  KVec::new(),
        };

        parser::parse_file(&source.data, parse_id, &mut parse,
            &mut self.strings, &mut self.errors, &arena);

        // @todo: make this safer.
        let parse = unsafe { core::mem::transmute(parse) };

        let data_id = self.parse_datas.alloc(ParseData { arena, parse });
        let id = self.parses.push(data_id.some());
        assert_eq!(id, parse_id);

        source.parses.clear();
        source.parses.push(parse_id);
    }
}


/*
use sti::arena::Arena;
use sti::vec::Vec;
use sti::boks::Box;

use crate::string_table::StringTable;
use crate::source_map::SourceMap;
use crate::error::ErrorCtx;
use crate::parser::{Parser, Tokenizer};
use crate::env::{Env, SymbolId};
use crate::elab::Elab;
use crate::traits::Traits;


pub struct Compiler {
    #[allow(dead_code)]
    alloc: Box<Arena>,
    inner: Inner<'static>,
}

struct Inner<'a> {
    alloc: &'a Arena,
    source_map: SourceMap<'a>,
    elab: Elab<'a, 'a, 'a>,
}

impl Compiler {
    pub fn new() -> Self {
        let alloc = Box::new(Arena::new());
        alloc.min_block_size.set(1*1024*1024);

        let strings = alloc.alloc_new(StringTable::new(&alloc));

        let source_map = SourceMap::new();

        let errors = alloc.alloc_new(ErrorCtx::new(&alloc));

        let env = alloc.alloc_new(Env::new());

        let traits = alloc.alloc_new(Traits::new());

        let elab = Elab::new(env, traits, SymbolId::ROOT, errors, strings, &alloc);

        let inner = Inner {
            alloc: &alloc,
            source_map,
            elab,
        };

        let inner: Inner<'static> = unsafe { core::mem::transmute(inner) };

        Self {
            alloc,
            inner,
        }
    }

    pub fn do_file(&mut self, name: &str, source: &[u8]) -> Result<(), ()> {
        self.inner.do_file(name, source)?;
        self.inner.dump_errors();
        Ok(())
    }

    #[inline(always)]
    pub fn with_elab<R, F: FnOnce(&mut Elab) -> R>(&mut self, f: F) -> R {
        f(&mut self.inner.elab)
    }
}

impl<'a> Inner<'a> {
    fn do_file(&mut self, name: &str, source: &[u8]) -> Result<(), ()> {
        let name   = self.alloc.alloc_str(name);
        let source = Vec::from_slice_in(self.alloc, source).leak();

        let offset = self.source_map.add_file(name, source).ok_or(())?;

        let tokens = {
            spall::trace_scope!("kibi/tok");

            Tokenizer::tokenize(source, offset, self.elab.strings, &self.alloc)
        };

        let mut items = Vec::new();
        {
            spall::trace_scope!("kibi/parse");

            let mut parser = Parser::new(&tokens,
                self.elab.errors, self.elab.strings, &self.alloc);

            while !parser.tokens.is_empty() {
                if let Some(item) = parser.parse_item() {
                    items.push(item);
                }
            }
        }

        let printing = false;

        for item in &items {
            use crate::ast::*;
            use crate::env::*;
            use crate::tt::TermPP;

            self.elab.reset();

            match &item.kind {
                ItemKind::Axiom(axiom) => {
                    spall::trace_scope!("kibi/elab/axiom"; "{}",
                        axiom.name.display(self.elab.strings));

                    let Some(_) = self.elab.elab_axiom(axiom) else { break };

                    if printing {
                        print!("axiom ");
                        match axiom.name {
                            IdentOrPath::Ident(name) => {
                                println!("{}", &self.elab.strings[name]);
                            }

                            IdentOrPath::Path(path) => {
                                print!("{}", &self.elab.strings[path.parts[0]]);
                                for part in path.parts[1..].iter().copied() {
                                    print!("::{}", &self.elab.strings[part]);
                                }
                                println!();
                            }
                        }
                    }

                    let Some(()) = self.elab.check_no_unassigned_variables() else {
                        println!("error: unassigned inference variables");
                        break;
                    };
                }

                ItemKind::Def(def) => {
                    spall::trace_scope!("kibi/elab/def"; "{}",
                        def.name.display(self.elab.strings));

                    let Some(_) = self.elab.elab_def(def) else { break };

                    if printing {
                        print!("def ");
                        match def.name {
                            IdentOrPath::Ident(name) => {
                                println!("{}", &self.elab.strings[name]);
                            }

                            IdentOrPath::Path(path) => {
                                print!("{}", &self.elab.strings[path.parts[0]]);
                                for part in path.parts[1..].iter().copied() {
                                    print!("::{}", &self.elab.strings[part]);
                                }
                                println!();
                            }
                        }
                    }

                    let Some(()) = self.elab.check_no_unassigned_variables() else {
                        println!("error: unassigned inference variables");
                        break;
                    };
                }

                ItemKind::Reduce(expr) => {
                    spall::trace_scope!("kibi/elab/reduce");

                    let Some((term, _)) = self.elab.elab_expr(expr) else { break };
                    let r = self.elab.reduce(term);

                    if printing {
                        let temp = sti::arena_pool::ArenaPool::tls_get_temp();
                        let mut pp = TermPP::new(&self.elab.env, &self.elab.strings, &*temp);
                        let r = pp.pp_term(r);
                        let r = pp.indent(9, r);
                        let r = pp.render(r, 80);
                        let r = r.layout_string();
                        println!("reduced: {}", r);
                    }
                }

                ItemKind::Inductive(ind) => {
                    spall::trace_scope!("kibi/elab/inductive"; "{}",
                        &self.elab.strings[ind.name]);

                    let Some(_) = self.elab.elab_inductive(ind) else { break };

                    if printing {
                        println!("inductive {}", &self.elab.strings[ind.name]);
                    }
                }

                ItemKind::Trait(trayt) => {
                    match trayt {
                        item::Trait::Inductive(ind) => {
                            spall::trace_scope!("kibi/elab/trait-ind",
                                &self.elab.strings[ind.name]);

                            let Some(symbol) = self.elab.elab_inductive(ind) else { break };

                            self.elab.traits.new_trait(symbol);

                            if printing {
                                println!("trait inductive {}", &self.elab.strings[ind.name]);
                            }
                        }
                    }
                }

                ItemKind::Impl(impel) => {
                    spall::trace_scope!("kibi/elab/impl");

                    let Some((ty, val)) = self.elab.elab_def_core(
                        impel.levels, impel.params, Some(&impel.ty), &impel.value) else { break };

                    let trayt = ty.forall_ret().0.app_fun().0;
                    if let Some(g) = trayt.try_global() {
                        if self.elab.traits.is_trait(g.id) {
                            let impls = self.elab.traits.impls(g.id);
                            // @speed: arena.
                            let name = self.elab.strings.insert(&format!("impl_{}", impls.len()));
                            let symbol = self.elab.env.new_symbol(g.id, name, SymbolKind::Def(symbol::Def {
                                num_levels: impel.levels.len() as u32,
                                ty,
                                val: Some(val),
                            })).unwrap();
                            self.elab.traits.add_impl(g.id, symbol);
                        }
                        else {
                            println!("error: must impl a trait");
                            break;
                        }
                    }
                    else {
                        println!("error: must impl a trait");
                        break;
                    }

                    if printing {
                        println!("impl");
                    }

                    let Some(()) = self.elab.check_no_unassigned_variables() else {
                        println!("error: unassigned inference variables");
                        break;
                    };
                }
            }
        }

        Ok(())
    }

    fn dump_errors(&self) {
        use crate::error::*;

        self.elab.errors.with(|errors| {
            errors.iter(|e| {
                // error line:
                {
                    print!("error: ");

                    match e.kind {
                        ErrorKind::Parse(e) => {
                            match e {
                                ParseError::Expected(what) => {
                                    println!("expected: {what}");
                                }

                                ParseError::Unexpected(what) => {
                                    println!("unexpected: {what}");
                                }
                            }
                        }

                        ErrorKind::Elab(e) => {
                            match e {
                                ElabError::SymbolShadowedByLocal(name) => {
                                    println!("symbol {:?} shadowed by a local variable", name);
                                }

                                ElabError::UnresolvedLevel(name) => {
                                    println!("unresolved level: {name:?}");
                                }

                                ElabError::UnresolvedName { base, name } => {
                                    if base != "" {
                                        println!("unresolved name. cannot find {name:?} in {base:?}");
                                    }
                                    else {
                                        println!("unresolved name: {name:?}");
                                    }
                                }

                                ElabError::LevelMismatch { expected, found } => {
                                    println!("level count mismatch. expected {expected} levels, found {found}");
                                }

                                ElabError::TypeMismatch {..} => {
                                    println!("type mismatch.");
                                }

                                ElabError::TypeExpected {..} => {
                                    println!("type expected.");
                                }

                                ElabError::TooManyArgs => {
                                    println!("too many args.");
                                }
                            }
                        }
                    }
                }

                // code:
                {
                    let (_, input) = self.source_map.lookup(e.source.begin);

                    let err_begin = e.source.begin as usize;
                    let err_end   = e.source.end   as usize;
                    let mut begin = err_begin;
                    let mut end   = err_end;
                    while begin > 0 && input[begin - 1] != b'\n' { begin -= 1 }
                    while end < input.len() && input[end] != b'\n' { end += 1 }

                    let begin_line = {
                        let mut line = 1;
                        let mut at = begin;
                        while at > 0 {
                            if input[at] == b'\n' { line += 1 }
                            at -= 1;
                        }
                        line
                    };

                    let string = unsafe { core::str::from_utf8_unchecked(&input[begin..end]) };

                    let mut line = begin_line;
                    let mut at = begin;
                    for l in string.lines() {
                        println!("{:4} | {}", line, l);

                        let end = at + l.len();
                        if err_begin < end && err_end > at {
                            let b = err_begin.max(at) - at;
                            let e = err_end.min(end)  - at;
                            for _ in 0..b+7 { print!(" ") }
                            for _ in 0..(e-b).max(1) { print!("^") }
                            println!();
                        }

                        line += 1;
                        at = end + 1;
                    }
                }

                // extra info.
                {
                    use crate::pp::PP;
                    let temp = sti::arena_pool::ArenaPool::tls_get_temp();

                    match e.kind {
                        ErrorKind::Parse(e) => {
                            match e {
                                ParseError::Expected(_) => {}
                                ParseError::Unexpected(_) => {}
                            }
                        }

                        ErrorKind::Elab(e) => {
                            match e {
                                ElabError::SymbolShadowedByLocal(_) => {
                                }

                                ElabError::UnresolvedName {..} => {}

                                ElabError::UnresolvedLevel(_) => {}

                                ElabError::LevelMismatch {..} => {}

                                ElabError::TypeMismatch { expected, found } => {
                                    let pp = PP::new(&*temp);
                                    let expected = pp.render(expected, 50);
                                    let expected = expected.layout_string();
                                    let found = pp.render(found, 50);
                                    let found = found.layout_string();
                                    println!("expected: {}", expected.lines().next().unwrap());
                                    for line in expected.lines().skip(1) {
                                        println!("          {}", line);
                                    }
                                    println!("found:    {}", found.lines().next().unwrap());
                                    for line in found.lines().skip(1) {
                                        println!("          {}", line);
                                    }
                                }

                                ElabError::TypeExpected { found } => {
                                    let pp = PP::new(&*temp);
                                    let found = pp.render(found, 50);
                                    let found = found.layout_string();
                                    println!("found: {}", found.lines().next().unwrap());
                                    for line in found.lines().skip(1) {
                                        println!("       {}", line);
                                    }
                                }

                                ElabError::TooManyArgs => (),
                            }
                        }
                    }
                }

                println!();
            });
        });
    }
}

*/

