use kibi::pp::*;
use kibi::error::*;
use kibi::ast::*;
use kibi::tt::TermPP;
use kibi::env::*;


fn main() {
    let arena = sti::growing_arena::GrowingArena::new();
    arena.min_block_size.set(1024*1024);

    let input = "
reduce (λ(a: Nat, b: Nat) =>
    Nat::rec.{1}(
        b,
        λ(_: Nat) => Nat,
        a,
        λ(_: Nat, r: Nat) => Nat::succ(r))
    )(1, 2)

def Nat::add (a: Nat, b: Nat): Nat :=
    Nat::rec.{1}(
        b,
        λ(_: Nat) => Nat,
        a,
        λ(_: Nat, r: Nat) => Nat::succ(r))

reduce Nat::add(1, 2)
".as_bytes();

    let input = {
        let args = std::env::args();
        if args.len() == 2 {
            let path = args.into_iter().nth(1).unwrap();
            Vec::leak(std::fs::read(path).unwrap())
        }
        else { input }
    };

    let p0 = arena.alloc_ptr::<u8>().as_ptr() as usize;
    let tokens = kibi::parser::Tokenizer::tokenize(&arena, 0, input);

    let mut env = Env::new();
    let nat = env.create_nat();
    let ns = env.create_initial(nat);

    let errors = ErrorCtx::new(&arena);

    let mut elab = kibi::elab::Elab::new(&mut env, ns, &errors, &arena);

    let mut parser = kibi::parser::Parser::new(&arena, &errors, &tokens);
    while !parser.tokens.is_empty() {
        let Some(item) = parser.parse_item() else { break };

        match &item.kind {
            ItemKind::Def(def) => {
                let Some(_) = elab.elab_def(def) else { break };
                println!("def {:?}", def.name);
            }

            ItemKind::Reduce(expr) => {
                let Some((term, _)) = elab.elab_expr(expr) else { break };
                let r = elab.tc().reduce(term);

                let mut pp = TermPP::new(&arena, &elab.env);
                let r = pp.pp_term(r);
                let r = pp.indent(9, r);
                let r = pp.render(r, 50);
                let r = r.layout_string();
                println!("reduced: {}", r);
            }
        }
    }

    let p1 = arena.alloc_ptr::<u8>().as_ptr() as usize;
    println!("total: {:?}", p1 - p0 - 16);

    errors.with(|errors| {
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
                            ElabError::SymbolShadowedByLocal(_) => {
                            }

                            ElabError::UnresolvedName {..} => {
                            }

                            ElabError::LevelMismatch {..} => {
                            }

                            ElabError::TypeMismatch {..} => {
                                println!("type mismatch.");
                            }

                            ElabError::TypeExpected {..} => {
                            }
                        }
                    }
                }
            }

            // code:
            {
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
                match e.kind {
                    ErrorKind::Parse(e) => {
                        match e {
                            ParseError::Expected(what) => {}
                            ParseError::Unexpected(what) => {}
                        }
                    }

                    ErrorKind::Elab(e) => {
                        match e {
                            ElabError::SymbolShadowedByLocal(_) => {
                            }

                            ElabError::UnresolvedName { base, name } => {
                            }

                            ElabError::LevelMismatch { expected, found } => {
                            }

                            ElabError::TypeMismatch { expected, found } => {
                                let pp = PP::new(&arena);
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
                            }
                        }
                    }
                }
            }

            println!();
        });
    });

    /*

    let alloc = kibi::tt::Alloc::new(&arena);
    let l = alloc.mkl_max(
        alloc.mkl_nat(5),
        alloc.mkl_imax(
            alloc.mkl_nat(7),
            alloc.mkl_nat(0)));

    let pp = kibi::pp::PP::new(&arena);
    let mut tpp = kibi::tt::TermPP::new(&arena);

    let nat_add = {
        let input = "λ(a: Nat, b: Nat) =>
            Nat::rec.{1}(b, λ(_: Nat) => Nat, a, λ(_: Nat, r: Nat) => Nat::succ(r))";

        let tokens = kibi::parser::Tokenizer::tokenize(&arena, 0, input.as_bytes());

        let errors = ErrorCtx::new(&arena);

        let mut parser = kibi::parser::Parser::new(&arena, &errors, &tokens);
        let ast = parser.parse_expr().unwrap();
        errors.with(|errors| assert!(errors.empty()));

        let mut elab = kibi::elab::Elab::new(&mut env, ns, &errors, &arena);
        let (term, _) = elab.elab_expr(&ast).unwrap();

        errors.with(|errors| assert!(errors.empty()));

        term
    };

    let doc = tpp.pp_term(nat_add);

    let _doc = 
        pp.group(pp.cats(&[
            pp.text("("),
            pp.indent(1,
                pp.group(pp.cats(&[
                    pp.text("aaaa"),
                    pp.line(),
                    pp.text("bbb"),
                ]))),
            pp.text(")("),
            pp.group(pp.indent(2, pp.cats(&[
                pp.line(),
                pp.text("bbbbb"),
            ]))),
            pp.text(")"),
        ]));

    let print = |doc: &kibi::pp::Doc, width: i32| {
        let doc = pp.render(doc, width);

        let mut buffer = String::new();
        doc.layout_string(&mut buffer);

        for _ in 0..width { print!("-") } println!();
        println!("{}", buffer);
    };

    for i in (10..40).step_by(7) {
        print(doc, i);
    }
    */
}

