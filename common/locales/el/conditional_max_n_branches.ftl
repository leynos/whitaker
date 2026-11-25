## Οι συνθήκες πρέπει να παραμένουν ρηχές.

# `branches` and `limit` are numbers; select on them for plural agreement.
conditional_max_n_branches = { $limit ->
        [one] Περιορίστε την έκφραση «{ $name }» σε { $limit } κλάδο το πολύ.
       *[other] Περιορίστε την έκφραση «{ $name }» σε { $limit } κλάδους το πολύ.
    }
    .note = { $branches ->
        [one] Η έκφραση «{ $name }» περιέχει αυτήν τη στιγμή { $branches } κλάδο.
       *[other] Η έκφραση «{ $name }» περιέχει αυτήν τη στιγμή { $branches } κλάδους.
    }
    .help = Εξαγάγετε βοηθητικές συναρτήσεις ή απλοποιήστε την έκφραση «{ $name }», για να μειώσετε τη διακλάδωση.
