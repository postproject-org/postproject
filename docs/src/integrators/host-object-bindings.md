# Host-object bindings

An editor, scene application, or automation host usually keeps its own document
format. When that document refers to PostProject knowledge, persist the complete
production-scoped identity rather than an object UUID by itself:

```text
https://postproject.org/ref/v1/<production UUID>/<object kind>/<object UUID>
```

The supported object-kind tokens are `production`, `asset`, `representation`,
`resource`, `activity`, and `job`. Treat the serialized value as opaque identity text.
Its project-controlled HTTPS namespace can point to documentation, but parsing
and using a binding never performs a network request.

Formatting and parsing are pure value operations available in every library
surface. The example binds a representation, stores the text, and parses it
back:

```{code-variants} host-binding
:::{no-variant} cli
The CLI does not format or parse host-object bindings. Use one of the library
surfaces.
:::
```

A formatted C string is caller-owned and must be released exactly once with
`pp_string_release`; parsed UUID and object-reference values are copied
into caller-owned output structs. The other surfaces return ordinary values.

Parsing is deliberately strict: versions and object kinds must be known, UUIDs
must use lowercase hyphenated canonical text, and extra fields are rejected. A
future format can therefore be introduced without interpreting ambiguous old
text. The former development-only private-scheme spelling is not accepted.

## Fallback information

A host may separately retain a display name, production path, or last known
locator to help a person repair an unavailable binding. That fallback is never
part of identity. If the production cannot be opened or the object does not
exist, preserve the binding and report an explicit rebind state; do not silently
select a production or object from fallback text.

## The opposite direction

When PostProject needs to find an object owned by the host, attach the host's
stable identifier through the external-identifier API. Host identifiers are
lookup aids and assertions. They do not become globally unique merely because a
host supplied them, and they do not replace the production-scoped binding above.
