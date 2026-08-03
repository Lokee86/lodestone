## Repository role

Lodestone owns immutable content-addressed vector objects, versioned materialized search snapshots, exact vector search, and narrow language bindings. It does not own embedding generation, document interpretation, hybrid ranking, or application memory policy.

## Architecture rules

- Preserve immutable object identity and reject conflicting vector bytes at an existing model/source address.
- Validate every object and snapshot before exposing it to search.
- Treat snapshot formats and ABI symbols as versioned compatibility contracts.
- Keep caller and Rust allocation ownership explicit across the C ABI.
- Search operations must retain a safe snapshot reference independent of concurrent handle closure.
- Do not silently rewrite v1 objects or snapshots into a new format identity.
- Record publication, corruption, migration, and compatibility behavior in the canonical docs and ADRs.

## Documentation

Documentation is part of the implementation. Update focused storage, snapshot, ABI, operations, maintainer-map, behavioral-contract, and decision owners with the code they describe.
