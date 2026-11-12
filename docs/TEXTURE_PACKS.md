# Natural pixel textures

The built-in artwork uses muted grass and water, layered soil and stone, bark ridges,
leaf clusters, mineral inclusions, a faceted diamond and a scored bread crust.
Sprite names, IDs, dimensions and UV layouts are unchanged.

Export editable PNGs from the repository root:

```sh
cargo run -p wolfcraft-assets --example export_pack -- texture-pack
```

Copy the exported category folders (`blocks`, `items`, `entities`, `gui`, `particles`)
into the game's data directory under `resourcepacks/`. PNGs override the generated
art by name, for example `resourcepacks/blocks/stone.png`.
Remove an override to return to the generated default. Water overrides are static
and are preserved at any resolution, including the native 16×16 size.
The supplied PNG pack contains static water; omit `blocks/water.png` to keep animation.

Validation: `cargo test -p wolfcraft-assets`. This tests atlas generation, PNG
round-trips, packing and native/high-resolution water override preservation.
Visual changes still need an in-game check on desktop and Android.
