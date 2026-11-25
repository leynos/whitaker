## Tests mogen geen voorbeeldsecties in hun documentatie bevatten.

test_must_not_have_example = Verwijder voorbeeldsecties uit de documentatie van test { $test }.
    .note = De documentatie van { $test } bevat het volgende: “{ $reason }”.
    .help = Verwijder het voorbeeld of verplaats het naar zelfstandige gebruikersdocumentatie.
