# cachet_atlas

Atlas-oriented request and resolve vocabulary for Cachet.

## Role

`cachet_atlas` makes atlas-style workloads explicit on top of the generic
residency kernel.

It owns:

- artifact request vocabulary
- atlas compatibility classes
- artifact pixel sizes
- atlas-facing resolved metadata

It does not own glyph rasterization, image decoding, GPU uploads, or atlas
binding policy.

## Main Types

- `ArtifactRequest<K>`: atlas-facing request wrapper passed onward into
  `cachet_residency`
- `AtlasClass`: compatibility bucket for atlas content
- `ArtifactSize`: requested extent in atlas pixels
- `ResolvedArtifact<K>`: metadata handed back once storage assigned a slot

## Notes

- `AtlasClass` is not a built-in multi-page routing mechanism yet.
- The first slice still assumes a single-page allocator from `cachet_storage`.
