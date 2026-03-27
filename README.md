# Simple-Compiler

Small ARM64-oriented compiler project: **lex → parse → typecheck → IR → assembly**.

## Grammar

Lexical rules (informal): whitespace separates tokens; **`//` comments** (if you add them in the lexer) run to end of line. Identifiers match `[A-Za-z_][A-Za-z0-9_]*`; integer literals match `[0-9]+` (value must fit `i64`). **Character literals** use **`'`** … **`'`** around one byte, with escapes `\'`, `\\`, `\n`, `\t`, `\r`, `\0`. Keywords are reserved: `int`, `bool`, `char`, `if`, `while`, `else`, `true`, `false`, `print_int`, `print_bool`, `print_char`.

Operator precedence follows C-like rules: **weaker** operators appear **higher** in the chain (`||` through `primary`), so precedence **increases** toward `primary`.

```ebnf
program   ::= stmt* ;

stmt      ::= type IDENT "=" expr ";"
            | IDENT "=" expr ";"
            | "print_int" "(" expr ")" ";"
            | "print_bool" "(" expr ")" ";"
            | "print_char" "(" expr ")" ";"
            | "if" "(" expr ")" stmt else_part?
            | "while" "(" expr ")" stmt
            | block ;

else_part ::= "else" stmt ;

block     ::= "{" stmt* "}" ;

type      ::= "int" | "bool" | "char" ;

expr      ::= or_expr ;

or_expr   ::= and_expr { "||" and_expr } ;

and_expr  ::= eq_expr { "&&" eq_expr } ;

eq_expr   ::= rel_expr { ("==" | "!=") rel_expr } ;

rel_expr  ::= add_expr { ("<" | "<=" | ">" | ">=") add_expr } ;

add_expr  ::= mul_expr { ("+" | "-") mul_expr } ;

mul_expr  ::= unary { ("*" | "/") unary } ;

unary     ::= ("-" | "!") unary | primary ;

primary   ::= INT_LIT
            | CHAR_LIT
            | "true"
            | "false"
            | IDENT
            | "(" expr ")" ;
```

### Typing (static rules, not syntax)

These are enforced while **lowering** to IR; the grammar above accepts only the shape of programs.

- `if` **condition** must be **`bool`** (no `if (int)`). Same for **`while`**.
- **`char`** is a **single byte** (0–255). It is stored in a 64-bit stack word (zero-extended) in IR, like a tiny integer for codegen.
- **`char` literals** have type **`char`**. An **`int`** literal **0..=255** may initialize or assign to **`char`** (C-style narrowing for constants).
- **`char`** promotes like C: with **`int`** in **`+` `-` `*` `/`** (operands **`int`** or **`char`**, result **`int`**). Unary **`-`** on **`char`** yields **`int`**.
- Comparisons **`==` … `>=`**: either **both `bool`** or **both numeric** (**`int`** / **`char`** in any mix); result **`bool`**.
- Unary **`!`** only on **`bool`**. **`&&` / `||`**: **`bool`** only.
- For **`int` / `bool` / `char`** declarations, the initializer must match the type, except **`char`** may use an **`int`** literal in **0..=255**.
- **`print_int`** → **`int`**; **`print_bool`** → **`bool`**; **`print_char`** → **`char`**.

### Dangling `else`

An `else` binds to the **innermost** `if` that can accept it (same rule as C).

</think>
<think>
I made a mistake - I left a bogus line in the README with a comment about typo and duplicate and_eq. Let me read the file and fix it to be clean - only and_expr, no and_eq, no editorial note about typos.
</think>
Removing the mistaken `and_eq` line and the editorial note from the README.

<｜tool▁calls▁begin｜><｜tool▁call▁begin｜>
Read