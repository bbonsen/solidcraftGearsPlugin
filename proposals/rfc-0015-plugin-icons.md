# Draft comment for RFC 0015, open question 2 (icons)

A draft to post on SolidCraft's RFC 0015 discussion
(`docs/rfcs/0015-plugin-ui-workspaces.md`, "Open questions" 2). Not posted.

---

**Open question 2 (icons): a case for plugin-supplied single-colour SVGs, from a working plugin**

I maintain a `commands@0.1` plugin, Gears (https://github.com/bbonsen/solidcraftGearsPlugin), with
twelve commands: spur, helical, herringbone, internal, planetary, rack and pinion, bevel, hypoid,
worm, worm wheel, cycloidal and cycloidal drive. In 0.43.0 all twelve ribbon buttons show the
same `prefs_plugins` icon, so the label is the only way to tell them apart.

Registry icons alone would not fix that. The registry has no gear icons, and a domain plugin's
commands are exactly the ones the registry is least likely to cover (twelve kinds of gear here,
pocketing and drilling operations for the planned Manufacturing plugin). So I'd answer the
question with **both**: a registry name where one fits, or a plugin-supplied single-colour SVG
that the host tints and checks.

To show what that would look like, the repository has an icon for each command in `icons/`, drawn
from the plugin's own geometry: a 64 × 64 view box, artwork inside 5…59, 2.55-unit round-capped
lines and only `currentColor`, following `crates/look/icons/STYLE_GUIDE.md` where a plugin can.
`icons/contact-sheet.png` shows them at 24 px in both themes.

A shape that keeps the RFC's rules ("the host draws everything", "nothing the plugin sends is
code, pixels or egui"):

- **Declaration.** An optional `icon` on an entry (and on `button-spec` / `workspace-spec`): either
  `{"registry": "<name>"}` or `{"svg": "icons/spur.svg"}`, a file inside the package.
- **A strict subset, parsed by the host, never rendered as SVG.** `viewBox="0 0 64 64"`; only
  `path` (and perhaps `circle`) elements; `fill` and `stroke` only `none` or `currentColor`;
  `stroke-width`, `stroke-linecap`, `stroke-linejoin` and `fill-rule` as attributes; no `style`,
  `text`, `image`, `use`, gradients, filters, scripts, links or external references. The host
  converts it into its own vector form and tints it with the theme's plugin accent, so a plugin
  cannot draw a host-looking control or reach the network.
- **Bounded.** For example at most 8 KiB and a few thousand path commands per icon, and a package
  total.
- **Covered by the existing identity check.** Icon files are package content, so the `approval`
  and `archive` hashes already cover them; changing an icon needs review again like any other byte.
- **Placeholders, not refusal.** An icon that fails the checks falls back to the generic plugin
  icon, and `plugin.inspect` / `plugin.install` report why, in line with "unknown kinds get
  placeholders, never refusal".
- **Decoration only.** The label stays the button's accessible name and what agents see; the icon
  adds nothing to the command surface.

Happy to test a first version against this plugin.

---

## A separate, smaller point (could be its own issue)

`docs/plugins.md` ("Menu and ribbon entries") says a `ribbon` slot named for a Design tab places
the entry in that tab, and that a `menu` slot named File or Help places it in that menu. In 0.43.0
and on main, `crates/ui-egui/src/plugin_menu.rs` puts every plugin button in the Model tab's
Plugins group, labels it with the `ribbon` slot's name (`ribbon_buttons`), and uses the `menu`
slot's name as the command's title (`command_title`). A plugin that follows the docs and declares
`{"kind": "ribbon", "name": "Model"}` on every entry ends up with every button labelled "Model".
Either the docs or the code should change; the code's behaviour (slot names as titles and labels)
is the more useful of the two, so updating the table seems simplest.
