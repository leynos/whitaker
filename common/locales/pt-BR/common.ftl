## Elementos de diagnóstico comuns, compartilhados entre os crates de lints.

-brand-name-whitaker = Whitaker
-term-lint = lint
-term-module = módulo
-term-branch = ramificação
-term-test-coverage = cobertura de testes

common-lint-count = Contagem de lints: { $lint }.
    .note = As mensagens de { $lint } estão disponíveis neste idioma.
    .help = Adicione traduções para cada identificador de lint para manter a cobertura de testes intacta.
    .fallback-note = Os diagnósticos de fallback usam o inglês por padrão.

# Shown in diagnostics to refer to the preceding attribute name.
common-attribute-fallback = o atributo anterior
