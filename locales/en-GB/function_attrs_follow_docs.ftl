## Function attribute ordering lint diagnostics.

function_attrs_follow_docs = Doc comments on { $subject } must precede other outer attributes.
    .note = The outer attribute { $attribute } appears before the doc comment.
    .help = Move the doc comment so it appears before { $attribute } on the item.
