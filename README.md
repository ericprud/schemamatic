# schemamatic
Bi-di conversions between ShEx and other schema languages like LinkML, SHACL, and JSON schema

## objective

Allow shared schemas across different expressions.


## motivation

Allow folks to work in their favorite schema language.


## limitations

- expressivity - different schema languages have more or less expressivity. Some stuff will be have to captured as maybe annotations or comments or an auxiliary docs, potentially machine-readable in a hightly expressive language.


## mission plan

1. map purely conjunctive schemas from ShEx to LinkML and JSON Schema.
2. map back.
3. survey deployed schemas.
4. survey currently-unexpressed schema requirements.
5. use products of 3 and 4 in tests for 1 and 2.
