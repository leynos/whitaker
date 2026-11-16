## Elementos de diagnóstico comuns, partilhados entre os crates de lints.

-brand-name-whitaker = Whitaker
-term-lint = lint
-term-module = módulo
-term-branch = ramo
-term-test-coverage = cobertura de testes

common-lint-count = Contagem de lints: { $lint }.
    .note = As mensagens de { $lint } estão disponíveis neste idioma.
    .help = Adicionar traduções para cada identificador de lint para manter intacta a cobertura de testes.
    .fallback-note = Os diagnósticos de contingência utilizam o inglês por predefinição.

# Shown in diagnostics to refer to the preceding attribute name.
common-attribute-fallback = o atributo anterior
