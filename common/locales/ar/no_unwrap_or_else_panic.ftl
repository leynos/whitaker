## تجنّب بدائل unwrap_or_else التي تُحدث هلعًا.

no_unwrap_or_else_panic = استعض عن unwrap_or_else على { $receiver } ببديل احتياطي لا يُحدث هلعًا.
    .note = المغلق الممرَّر إلى unwrap_or_else يُحدث هلعًا.
    .help = مرّر الخطأ إلى المستدعي أو استخدم `expect` مع رسالة وصفية بدلًا من إحداث هلع.
