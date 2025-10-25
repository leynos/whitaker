## Modules require leading inner doc comments.

module_must_have_inner_docs = Module { $module } must start with an inner doc comment.
    .note = The first item in the module is not a `//!` style comment.
    .help = Explain the purpose of { $module } by adding an inner doc comment at the top.
