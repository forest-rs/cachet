# cachet_atlas

Atlas-oriented request and resolve vocabulary for Cachet.

## Role

`cachet_atlas` makes atlas-style workloads explicit on top of the generic
residency kernel.

It owns:

- artifact request vocabulary
- atlas compatibility classes
- atlas-side routing across compatible storage pages
- artifact pixel sizes
- atlas-facing resolved metadata

It does not own glyph rasterization, image decoding, GPU uploads, or atlas
binding policy.

## Main Types

- `ArtifactRequest<K>`: atlas-facing request wrapper passed onward into
  `cachet_residency`
- `AtlasClass`: compatibility bucket for atlas content
- `AtlasPageRouter`: atlas-side routing policy over storage pages
- `ArtifactSize`: requested extent in atlas pixels
- `ResolvedArtifact<K>`: metadata handed back once storage assigned a slot

## Notes

- `AtlasClass` is a compatibility bucket, not a concrete storage page id.
- `AtlasPageRouter` chooses among compatible `cachet_storage` pages without
  turning page routing into storage policy.
