## Îndrumări privind lungimea modulelor.

module_max_lines = { $lines ->
        [one] Modulul { $module } se întinde pe { $lines } linie, depășind limita permisă de { $limit }.
        [few] Modulul { $module } se întinde pe { $lines } linii, depășind limita permisă de { $limit }.
       *[other] Modulul { $module } se întinde pe { $lines } de linii, depășind limita permisă de { $limit }.
    }
    .note = Modulele mari sunt mai greu de parcurs și de revizuit.
    .help = Împărțiți { $module } în module mai mici sau reduceți-i responsabilitățile.
