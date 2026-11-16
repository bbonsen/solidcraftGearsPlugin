# Gears: a SolidCraft plugin

Makes involute gears in SolidCraft. It adds nine entries to the Model ribbon. You fill in a
form, and the plugin computes the tooth profile, then builds it with ordinary SolidCraft
commands:

| Entry | Builds it as |
|---|---|
| `spur` | The profile (a closed sketch polyline, plus an optional bore circle), extruded |
| `helical` | The profile, swept along a straight line on the gear's axis with a twist |
| `herringbone` | Two helical halves swept outwards from mid-width with opposite twists, joined |
| `bevel` | The heel profile, swept towards the cone apex while it shrinks; the bore is cut after |
| `worm` | The worm's section square to its axis, swept with one turn per lead; long worms in joined sections |
| `worm-wheel` | A helical gear that meshes with a `worm` |
| `internal` | A ring with teeth on the inside: an outer circle and the toothed inner outline, extruded |
| `rack` | A straight rack, extruded, plus (optionally) a spur pinion extruded in mesh with it |
| `hypoid` | One member of a hypoid pair: the heel profile swept towards the apex while it shrinks and twists (an approximation, see below) |

The result is ordinary editable sketches, extrudes and sweeps, so you can modify it later. The
whole run is a single undo step. The plugin needs no file, network or settings access.

![Two meshing profiles](preview.png)

## Install

In SolidCraft (desktop), open **Preferences > Plugins > Install plugin…** and pick
`gears-0.6.0.zip`. It asks for no permissions. You can also install it from a script:

```json
[{"command":"plugin.install","params":{"path":"gears-0.6.0.zip"}}]
```

To upgrade an earlier version by script, add `"replace": true`. In Preferences, remove the old version first if installing over it is refused.

## Inputs (entry `spur`)

| Field | Default | Meaning |
|---|---|---|
| `teeth` | 20 | 6–150 |
| `module` | 2 | Tooth size in mm. Pitch diameter = module × teeth |
| `pressure_angle` | 20 | 14.5, 20 or 25 degrees |
| `thickness` | 10 | Face width (extrude distance) in mm |
| `bore` | 0 | Centre hole diameter in mm. 0 means no hole |
| `backlash` | 0 | Thins each tooth by this much at the pitch circle, in mm |
| `plane` | XY | XY, XZ or YZ |
| `flank_points` | 8 | Points per tooth flank (2–40). More points give a smoother tooth but a heavier sketch |

The plugin returns the pitch, tip, root and base diameters. Two gears with the same module
and pressure angle mesh when their centres are **(z1 + z2) × module / 2** apart.

## Extra inputs (entry `helical`)

`helical` takes every input above, plus:

| Field | Default | Meaning |
|---|---|---|
| `helix_angle` | 20 | Helix angle β, more than 0 and up to 45 degrees |
| `hand` | right | `right` or `left` |

For a helical gear, `module`, `pressure_angle` and `backlash` are **normal** values, as a
catalogue or hob gives them. The plugin draws the profile in the transverse plane:

- transverse module = module / cos β
- transverse pressure angle = atan(tan α / cos β)
- pitch diameter = teeth × module / cos β
- addendum and dedendum stay 1 and 1.25 normal modules

The sweep twists the profile by face width × tan β / pitch radius. A right-hand gear twists
right-handed about its axis as it advances along it. The plugin also returns the transverse
module and pressure angle, the twist and the lead.

Two helical gears on parallel shafts mesh when they have the same normal module, pressure angle
and helix angle, and **opposite hands**. Their centres sit (z1 + z2) × transverse module / 2
apart.

## Herringbone (entry `herringbone`)

Takes the same inputs as `helical`, with a default face width of 20 mm. `hand` is the hand of
the half on the +normal side; the other half has the opposite hand. A mating herringbone gear
needs the same helix angle and the opposite hand. The halves cancel each other's thrust, so the
gear needs no thrust bearing.

## Straight bevel gears (entry `bevel`, shafts at 90°)

| Field | Default | Meaning |
|---|---|---|
| `teeth` | 20 | This gear's teeth |
| `mate_teeth` | 20 | The mating gear's teeth (equal teeth make a 45° miter pair) |
| `module` | 2 | Module at the large end (heel) |
| `pressure_angle` | 20 | 14.5, 20 or 25 degrees |
| `face_width` | 0 | Tooth length along the cone in mm. 0 means a third of the cone distance; at most half |

`bore`, `backlash`, `plane` and `flank_points` work as for `spur`.

The heel lies on the sketch plane, and the teeth run along +normal towards the apex. The pitch
angle is atan(teeth / mate_teeth). The heel profile uses Tredgold's approximation, the usual way
straight bevel teeth are drawn: it is the profile of a spur gear with teeth / cos(pitch angle)
teeth on the back cone, laid into the plane square to the axis. A scaled sweep then takes every
point of it straight towards the cone apex.

To make the mate, swap `teeth` and `mate_teeth`. The two axes meet at 90° at the cone apex, which
is `apex_distance_mm` (returned) along each gear's axis from its heel plane.

## Worms (entry `worm`) and worm wheels (entry `worm-wheel`)

| Field | Default | Meaning |
|---|---|---|
| `module` | 2 | Axial module: the thread's axial pitch is π × module |
| `starts` | 1 | Thread starts, 1–6. Gear ratio = wheel teeth / starts |
| `diameter_factor` | 10 | q, 5–30: worm pitch diameter = q × module |
| `pressure_angle` | 20 | Axial pressure angle |
| `hand` | right | `right` or `left` |
| `length` | 6 axial pitches | Worm length in mm (`worm` only; at most 40 turns of the thread) |
| `teeth` | 30 | Wheel teeth (`worm-wheel` only) |
| `thickness` | 10 | Wheel face width in mm (`worm-wheel` only) |

`bore`, `backlash`, `plane` and `flank_points` work as for `spur`.

The worm has a straight-sided axial profile (type ZA, Archimedean), with addendum 1 × module and
dedendum 1.25 × module. Cut square to its axis, such a worm is a gear outline with one tooth per
start. The plugin draws that section and sweeps it with a twist of one turn per lead, which is
the exact thread. A SolidCraft sweep twists at most four turns, so a longer worm is built from
sections of up to 3.5 turns. Each section starts where the last one ended, turned to match, and
joins the body.

The wheel is a helical gear with the same transverse module and pressure angle as the worm's
axial ones, and a helix angle equal to the worm's lead angle, with the same hand. Its axis
crosses the worm's at 90°, `centre_distance_mm` (returned) away. A real worm wheel is throated:
its rim curves to wrap around the worm. This one is not, so the teeth touch at points rather
than along lines. That is fine for light loads and 3D printing.

## Internal gears (entry `internal`)

| Field | Default | Meaning |
|---|---|---|
| `teeth` | 60 | 12–300 |
| `module` | 2 | |
| `pressure_angle` | 20 | 14.5, 20 or 25 degrees |
| `thickness` | 10 | Face width, mm |
| `rim` | 3 × module | Solid rim outside the tooth roots, mm |
| `pinion_teeth` | 0 | Optional: the pinion you plan to run inside, to warn about tip interference |

`backlash`, `plane` and `flank_points` work as for `spur`.

The ring's teeth point inwards: their tips are at pitch − module and their roots at
pitch + 1.25 × module. A ring's tooth spaces have the shape of an external gear's teeth, so the
inner outline is built as one, and a tooth is centred on +x. Any spur gear with the same module
and pressure angle meshes inside, its centre (ring teeth − pinion teeth) × module / 2 from the
ring's centre. Keep the ring at least 12 teeth larger than the pinion (15 below 20°), or the tips
interfere. Below about 34 teeth at 20°, the ring's tips lie inside the base circle, so their
innermost part is radial rather than involute; the reply warns.

## Rack and pinion (entry `rack`)

| Field | Default | Meaning |
|---|---|---|
| `teeth` | 20 | Rack teeth, 1–500. Rack length = teeth × π × module |
| `module` | 2 | Module of the rack and pinion |
| `pressure_angle` | 20 | 14.5, 20 or 25 degrees |
| `thickness` | 10 | Face width of both parts, mm |
| `back` | 2.5 × module | Depth of solid bar below the tooth roots, mm |
| `pinion_teeth` | 16 | Teeth of the matching spur pinion. 0 builds the rack alone |
| `pinion_bore` | 0 | Pinion bore diameter, mm |

`backlash`, `plane` and `flank_points` work as for `spur`. Backlash thins the teeth of both parts.

The rack's teeth have straight flanks at the pressure angle, so its outline is exact. In the
sketch plane, the rack runs along +x from x = 0 and its pitch line is y = 0. The tips are at
y = module and the roots at y = −1.25 × module. The pinion is placed over the middle rack tooth,
with its pitch circle touching the pitch line, and turned so one of its tooth gaps straddles that
tooth. Each part is its own body. One pinion turn moves the rack π × module × pinion teeth.
Any spur gear from the `spur` entry with the same module and pressure angle meshes with the rack.

## Hypoid gears (entry `hypoid`, an approximation)

A hypoid pair is a spiral bevel pair whose pinion axis is offset from the gear's, so the shafts
cross without meeting. This entry builds one member per run; run it twice, with `member` set
to `gear` and then `pinion` and the other inputs unchanged.

| Field | Default | Meaning |
|---|---|---|
| `member` | gear | `gear` (the ring gear) or `pinion` |
| `gear_teeth` | 37 | Ring gear teeth |
| `pinion_teeth` | 11 | Pinion teeth (not more than the gear's) |
| `module` | 2 | The gear's module at the heel |
| `pressure_angle` | 20 | Normal pressure angle |
| `spiral_angle` | 30 | The gear's spiral angle, 0–45 degrees |
| `offset` | 8 | Distance between the axes, mm, up to a quarter of the gear's pitch diameter. 0 gives a spiral bevel pair |
| `hand` | left | The pinion's hand; the gear takes the other |
| `face_width` | 0 | Tooth length along the cone, mm. 0 means 0.3 × the gear's cone distance |

`bore`, `backlash` (normal), `plane` and `flank_points` work as for `spur`.

How the pair is sized (the usual first-order rules):

- offset angle ε: sin ε = offset / gear pitch radius
- pinion spiral angle = gear spiral angle + ε
- both members share the normal module, so the pinion's heel module is
  module × cos(gear spiral) / cos(pinion spiral): the offset makes the pinion larger
- pitch cone angles as for a bevel pair with the same tooth counts

Each member is built like a `bevel` gear, from a Tredgold heel profile in the transverse plane,
but the sweep also twists. The twist is chosen so the teeth cross the mean cone at the member's
spiral angle, which makes each tooth a conical spiral.

**This is not a machine-generated hypoid tooth.** In a real hypoid pair the offset also changes
the pitch cones, and the flanks come from a cutting machine's motion. Here the cones are those
of a bevel pair and the flanks are spiral sweeps. The parts look right and suit visualisation
and 3D-printed prototypes with some backlash; they will not run like a cut automotive hypoid.
Expect to tune backlash and placement. The reply gives both members' pitch angles, cone and apex
distances to place them: the axes cross at 90°, `offset_mm` apart.

## Geometry

Spur, helical and herringbone gears have standard full-depth teeth:

- addendum = 1 × module
- dedendum = 1.25 × module
- no profile shift

Each flank is an involute of the base circle, approximated by a polyline. When the root
circle lies inside the base circle, the flank continues to the root as a straight radial
line. Root and tip lands are short polyline arcs. Undercut is not modelled: below about 17
teeth at 20° (fewer for a helical gear), the plugin returns a warning, and a real cut gear
would differ near the root.

## Build

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-tools
./build.sh          # tests, builds the component, writes gears-0.6.0.zip
```

`host-check/` runs the packaged component inside SolidCraft's real runtime crate
(`craft-plugin-wasm`), using a host that records each command call. It expects a SolidCraft
checkout next to this repository:

```bash
git clone --depth 1 https://github.com/tangency-labs/solidcraft ../solidcraft
cd host-check && cargo run --release
```

`wit/plugin.wit` is copied unchanged from SolidCraft (`crates/craft-plugin/wit/plugin.wit`,
MIT OR Apache-2.0).

## How SolidCraft plugins work (craft:plugin@0.1.0, experimental)

- A plugin is a WebAssembly **component** (not a plain core module) plus `plugin.json`.
- It runs sandboxed: no WASI, no network, and fuel, memory and deadline limits.
- It can only call the SolidCraft commands listed in the manifest's `runtime.commands`.
  These are the same commands MCP clients use. It gets JSON results back.
- `runtime.entries` declares each entry's input schema. SolidCraft draws the form from that
  schema and calls the component's `execute(entry, params)` export.
- A successful call is committed as one undo step. Any error rolls back the whole call.
- Plugins can't keep data between runs yet. The `storage` capability will add that.
  There are no custom panels and no parametric re-run: changing the gear means running
  the plugin again.

## Status

The plugin has been tested in SolidCraft's plugin runtime with a host that records each
command call, not yet in the SolidCraft app. Sweeping a profile with hundreds of segments,
with a twist or a scale, is the heaviest step. If it is slow or fails, lower `flank_points`.
The joins between worm sections and between herringbone halves meet face to face, which is the
case most likely to trouble a CAD kernel; if one fails, send the error.
