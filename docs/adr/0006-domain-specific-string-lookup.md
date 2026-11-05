# `StringLinker::get_by_id` is a convenience, not the parity contract

The C# renderers look up a string in one domain-specific dictionary at a time. The
Rust `get_by_id` helper mirrors that by trying item → eqp → skill → map → mob → npc,
but that order is a convenience for callers that do not know the domain. Oracle ops
that promise a specific domain (for example `tooltip` for item names) must use the
domain-specific map, not `get_by_id`, because IDs can collide across domains.

**Consequences**: new oracle ops should name the domain they query. `get_by_id` may
change order without being a parity break; domain-specific lookups may not.
