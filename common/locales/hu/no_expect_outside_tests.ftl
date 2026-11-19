## Az expect hívások korlátozása a tesztkörnyezeten kívül.

no_expect_outside_tests = Kerülje az `expect` hívását a(z) { $receiver } értéken a csak tesztelésre szolgáló kódon kívül.
    .note = A hívás a következő kontextusból származik, amely nem minősül tesztnek: „{ $context }”.
    .help = { $handling ->
        [option] Kezelje a(z) { $receiver } `None` variánsát, vagy helyezze át a kódot egy tesztbe.
        [result] Kezelje a(z) { $receiver } `Err` variánsát, vagy helyezze át a kódot egy tesztbe.
       *[other] Kezelje a(z) { $receiver } hibaútvonalát, vagy helyezze át a kódot egy tesztbe.
    }
