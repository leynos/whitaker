## Elementos de diagnóstico comunes, compartidos entre los crates de lints.

-brand-name-whitaker = Whitaker
-term-lint = lint
-term-module = módulo
-term-branch = rama
-term-test-coverage = cobertura de pruebas

common-lint-count = Conteo de lints: { $lint }.
    .note = Los mensajes de { $lint } están disponibles en este idioma.
    .help = Agregue traducciones para cada identificador de lint a fin de mantener intacta la cobertura de pruebas.
    .fallback-note = Los diagnósticos de respaldo se muestran en inglés de forma predeterminada.

# Shown in diagnostics to refer to the preceding attribute name.
common-attribute-fallback = el atributo anterior
