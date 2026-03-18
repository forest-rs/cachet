# cachet_atlas

Atlas-oriented request and resolve vocabulary for Cachet.

## Role

`cachet_atlas` makes atlas-style workloads explicit on top of the generic
residency kernel.

It owns:

- artifact request vocabulary
- atlas compatibility classes
- atlas-side routing across compatible storage pages
- atlas composition over residency, routing, and storage
- artifact pixel sizes
- atlas-facing resolved metadata

It does not own glyph rasterization, image decoding, GPU uploads, or atlas
binding policy.

## Main Types

- `ArtifactRequest<K>`: atlas-facing request wrapper passed onward into
  `cachet_residency`
- `AtlasClass`: compatibility bucket for atlas content
- `AtlasPageRouter`: atlas-side routing policy over storage pages
- `AtlasCache<K>`: calmer atlas composition path over the lower layers
- `AtlasCacheStats`: queue and page-pressure summary for the controller
- `AtlasQueueError`: queue-time validation failure for contradictory atlas
  metadata on one key
- `ArtifactSize`: requested extent in atlas pixels
- `ResolvedArtifact<K>`: metadata handed back once storage assigned a slot

## Notes

- `AtlasClass` is a compatibility bucket, not a concrete storage page id.
- `AtlasPageRouter` chooses among compatible `cachet_storage` pages without
  turning page routing into storage policy.
- `AtlasCache` is the intended integration path once you want queue ->
  process -> resolve -> evict behavior in one place.
- One logical key is expected to have stable `AtlasClass` and `ArtifactSize`.
  If those need to differ, model that distinction in the key instead of
  queueing contradictory requests.
- `AtlasCache::stats()` and `AtlasCache::page_stats()` are the intended first
  diagnostics hooks for page spill and fragmentation pressure.
