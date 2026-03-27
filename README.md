# Simple-Compiler

Small ARM64-oriented compiler project: **lex → parse → typecheck → IR → assembly**.

## Grammar

Lexical rules (informal): whitespace separates tokens; **`//` comments** (if you add them in the lexer) run to end of line. Identifiers match `[A-Za-z_][A-Za-z0-9_]*`; integer literals match `[0-9]+` (value must fit `i64`). Keywords are reserved: `int`, `bool`, `if`, `else`, `true`, `false`.

Operator precedence follows C-like rules: **weaker** operators appear **higher** in the chain (`||` through `primary`), so precedence **increases** toward `primary`.

```ebnf
program   ::= stmt* ;

stmt      ::= type IDENT "=" expr ";"
            | IDENT "=" expr ";"
            | "if" "(" expr ")" stmt else_part?
            | block ;

else_part ::= "else" stmt ;

block     ::= "{" stmt* "}" ;

type      ::= "int" | "bool" ;

expr      ::= or_expr ;

or_expr   ::= and_expr { "||" and_expr } ;

and_expr  ::= eq_expr { "&&" eq_expr } ;

eq_expr   ::= rel_expr { ("==" | "!=") rel_expr } ;

rel_expr  ::= add_expr { ("<" | "<=" | ">" | ">=") add_expr } ;

add_expr  ::= mul_expr { ("+" | "-") mul_expr } ;

mul_expr  ::= unary { ("*" | "/") unary } ;

unary     ::= ("-" | "!") unary | primary ;

primary   ::= INT_LIT
            | "true"
            | "false"
            | IDENT
            | "(" expr ")" ;
```

### Typing (static rules, not syntax)

These are **semantic** constraints for a later checker; the grammar above accepts only the shape of programs.

- `if` **condition** must have type **`bool`** (strict; no `if (int)`).
- `==`, `!=`, `<`, `<=`, `>`, `>=` take two operands of the **same** type (`int`/`int` or `bool`/`bool`) and yield **`bool`**.
- `+`, `-`, `*`, `/` take **`int`** operands and yield **`int`**.
- Unary `-` applies to **`int`**; unary `!` applies to **`bool`**.
- `&&`, `||` take **`bool`** operands and yield **`bool`**.
- In `type IDENT "=" expr ";"`, the initializer **`expr`** must match **`type`**.

### Dangling `else`

An `else` binds to the **innermost** `if` that can accept it (same rule as C).

</think>
<think>
I made a mistake - I left a bogus line in the README with a comment about typo and duplicate and_eq. Let me read the file and fix it to be clean - only and_expr, no and_eq, no editorial note about typos.
</think>
Removing the mistaken `and_eq` line and the editorial note from the README.

<｜tool▁calls▁begin｜><｜tool▁call▁begin｜>
Read