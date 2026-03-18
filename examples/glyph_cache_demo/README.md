# glyph_cache_demo

Runnable atlas-oriented glyph cache story for Cachet.

This example is aimed at future users evaluating the atlas path, especially for
Linebender-adjacent workloads such as glyph, icon, and small image-patch
caches.

What it demonstrates:

- describing glyphs with `cachet::atlas::ArtifactRequest`
- processing them through `cachet::atlas::AtlasCache`
- keeping logical residency policy in `cachet::residency`
- assigning physical atlas rects through `cachet::storage`
- handing resolved placement back to the caller as
  `cachet::atlas::ResolvedArtifact`

Run it with:

```text
cargo run -p glyph_cache_demo
```

The output is structured in phases so you can see the intended handoff between
atlas request vocabulary, the atlas cache controller, the residency kernel, and
the storage layer.
