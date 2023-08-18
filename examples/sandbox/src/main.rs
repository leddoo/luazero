use kibi::error::ErrorCtx;

use kibi::ast::*;
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
";

    let p0 = arena.alloc_ptr::<u8>().as_ptr() as usize;
    let tokens = kibi::parser::Tokenizer::tokenize(&arena, 0, input.as_bytes());

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
                let red = elab.tc().reduce(term);
                println!("reduced: {:?}", red);
            }
        }
    }

    let p1 = arena.alloc_ptr::<u8>().as_ptr() as usize;
    println!("total: {:?}", p1 - p0 - 16);

    errors.with(|errors| {
        errors.iter(|e| {
            println!("{:?}", e);
            let mut begin = (e.source.begin - 10.min(e.source.begin)) as usize;
            let mut end = (e.source.end   + 10).min(input.len() as u32) as usize;
            while input.as_bytes()[begin] & 0xc0 == 0x80 { begin -= 1 }
            while input.as_bytes()[end]   & 0xc0 == 0x80 { end   -= 1 }
            println!("{:?}", &input[begin..end]);
        });
    });


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
}

