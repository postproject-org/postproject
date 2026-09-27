# External identifiers

Attach an industry, vendor, or application identifier to a PostProject object
when another system needs to find that object by its own key. An identifier is
a [scheme, value, and optional qualifier](../concepts/external-identifiers.md);
PostProject stores all three exactly as supplied and never rewrites or
normalizes them.

The example attaches a camera serial number to an asset, reads the identifiers
attached to that asset, and finds every object carrying the exact scheme and
value:

```{code-variants} external-identifiers
```

Attachment is a transactional mutation like any other: it stays pending until
commit and appears in the [revision feed](revision-feed.md).

## Remove an identifier

An object can carry several identifiers, from the same scheme or from different
ones. Removing an identifier requires the same exact scheme, value, and
qualifier; other attachments stay in place:

```{code-variants} remove-identifier
```

## Lookup is exact

A scheme is not a namespace prefix, values are compared
byte-for-byte, and a lookup may return several objects because an external
system can reuse a value. A lookup can also require an exact qualifier:

- **Pass the qualifier whenever you know it.** Under a shared scheme such as the
  application scheme `https://postproject.org/id/application`, the qualifier is
  what separates one application's identifiers from another's. Use a qualifier
  rooted in a domain you control, for example `org.example.editor:clip_uuid`.
- **Omit it to match any qualifier.** The lookup then also returns objects
  whose identifier has no qualifier.
- A qualified lookup never matches an unqualified identifier.

Treat the result as candidates for the integration to
interpret, not as proof of identity.

To refer from a host document *to* a PostProject object, persist a
[host-object binding](host-object-bindings.md) instead; external identifiers
are lookup aids and assertions, not PostProject identity.
