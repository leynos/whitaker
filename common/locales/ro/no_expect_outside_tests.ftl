## Restricționarea apelurilor `expect` în afara contextelor de test.

no_expect_outside_tests = Evitați apelarea `expect` pe { $receiver } în afara codului destinat exclusiv testelor.
    .note = Apelul provine din „{ $context }”, care nu este recunoscut ca test.
    .help = { $handling ->
        [option] Tratați varianta `None` pentru { $receiver } sau mutați codul într-un test.
        [result] Tratați varianta `Err` pentru { $receiver } sau mutați codul într-un test.
       *[other] Tratați calea de eroare pentru { $receiver } sau mutați codul într-un test.
    }
