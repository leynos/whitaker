## Les tests ne doivent pas contenir de sections d’exemple dans leur documentation.

test_must_not_have_example = Supprimez les sections d’exemple de la documentation du test { $test }.
    .note = La documentation de { $test } contient l’élément suivant : « { $reason } ».
    .help = Supprimez l’exemple ou déplacez-le dans une documentation autonome destinée aux utilisateurs.
