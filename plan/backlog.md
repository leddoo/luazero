
### fixes & completeness:

- `Ident(Atom, TokenId)`.
    - hover info: symbol types.
    - no symbol token info for last part of path.
      but optional, only for exprs, not for defs.
    - inductive sources. should be able to get rid of `temp_source`.
- `SymbolKind::Axiom`.
- spall flush.
- error for symbol already defined.
- error for missing predeclared symbol.
- precise hit for exprs, when not hit a token. or make whitespace a token? yeah prob not.
- elab `foo` using `elab_app`.
- type errors use `Term` instead of `Doc`.
- query semantic tokens: option to split multi-line tokens.
- `validate_string`: `>= 0x20`. do we need simd?
- debug tracing.

- vfs:
    - directories, create/delete/write.
    - `vfs::mem`, `vfs::std`.

- sti
    - `HashMap::insert_new`.



### lsp:

- go to definition.
- proper semantic tokens.



### language features:

- type-less binders syntax.
    - `fn foo<A, B>(...)` cause it can infer `Type`.
    - though it can't figure out the level, i guess, which should be `1`.
    - `'r` could just be a valid identifier.
- dot-idents.
- methods.
    - lookup in type, needs `self` arg.



### compiler features:

- error resilient parsing.
    - parser skips comment tokens (add) & error tokens.
    - unterminated `/-` is error token and does not consume input until eof.
    - sep-by until terminator, but not if sync point in between.
    - use indentation for recovery of unmatched parens errors.
    - single line strings. (indented, multi-line later)
    - can we get neovim to draw skipped tokens in italics? maybe w/ modifiers.
- error resilient elab.
    - error to sorry.
    - type unknown for env -> treat as silent error in uses.

- incremental parse.
    - each item is a `Parse`.
    - if token range dirty, re-parse, otherwise, keep old result.
    - option for non-incr -> items don't start new `Parse`.
- incremental elab.
    - track dependencies.
    - re-elab if dependencies or item changed.
    - should be able to keep refs into env,
      cause need to rerun if anything used from env changed.
- testing incremental.
    - serialization or hash function.
    - do non-incr compile & compare.

- ability to use `Elaborator` after analysis.
    - cause that's useful. eg: type sensitive completions.
    - kinda tricky w/ current setup, but should be trivial with incr setup.

- callgraph for:
    - termination checking.
    - find references.
    - "highlight everything that can allocate."

- compiler: source id based functions (alternative to strings)?



### design:

- self-reference safety.



### old backlog:

- stuff:
    - trace debugging.
    - interactivity.
    - very basic tactics.
    - executable code.

- sti:
    - string and formatting stuff, write trait.
    - KVec::truncate, clone.
    - reader `revert(n)`, rename `offset -> position`.

- features:
    - mutual inductive:
        - mutual syntax.
        - add existing inductives to def, for non-strict-pos occ.
    - type annotation.
    - let.
    - named args.
    - explicit args.
    - check/print.
    - dot-idents.
    - method call syntax.
    - error to sorry.

- completeness:
    - `is_type_correct`.
        - lambda type check.
    - kernel type checker.
        - use for inductive.
        - env does type check before insertion.
    - level elab: use `.imax` & friends.
    - constant approx.
    - proper `abstract_def_eq`.
        - "key matching": only call `is_def_eq`, if the head symbol matches.
        - and try `syntax_eq` first, common case.

- cleanup:
    - `SymbolKind::Axiom`.
    - move `assign` into `def_eq_*`.
    - move `instantiate` into `ivars`.
    - `sep_by_ex` takes vec.
    - document inference.

- optimization:
    - metadata.
    - custom memory layout.
    - reset lctx after whnf/defeq?
    - pp caching.

- parser:
    - labels.
    - combined match/if.
    - optional do block.

- pp:
    - binder kinds & merging.
    - dedup local names.
    - `A -> B`.
    - config:
        - indent width.
        - args on separate lines.

- elab:
    - var to let.

- macros:
    - figure out compilation order.

- architecture:
    - error resilience.
    - shared data structures & multi-threading.
        - symbol table & namespaces.
        - error context.
        - consider fork/join around ordering points.
          populating symbol table synchronously.
    - more immutability:
        - for lsp and other queries.
        - but would like to compute ad-hoc, cause seems
          like it could require *a lot* of memory.
        - namespaces - they're kinda immutable by default,
          once we're doing ordering.
        - scopes: the single local + parent id seems nice.

