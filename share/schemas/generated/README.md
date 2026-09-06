# Schemas of the generated documents

One JSON Schema per structured document `majordomus generate` writes, matched to the
document by the `const` of its `schema` member. They are the *contract* of a generated
document — the members a reader may rely on — and not a restatement of the Rust types:
they fix the top level and the shape of each entry, and stay open below it, so that adding
a field to a descriptor is not a schema change while removing one is.

These files are read at run time, from this directory, by `majordomus generate` (which
validates every document it writes before writing it) and by `majordomus generate --check`.
They are **not** kind schemas: the directory above holds one schema per object kind of the
`.ai/` layer, discovered by `share/kinds.yaml`, and that discovery does not recurse, so a
schema here can never be mistaken for a kind's.

Nothing here is generated. The proof that a document still satisfies its schema is
`majordomus generate --check` and the test suite, both of which fail on the document, not
on the file that describes it.
