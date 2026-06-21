# `chomik`

`chomik` is a desktop buddy that is a hamster.
It helps you eat your files.
We hope this has made everything clear.

> [!IMPORTANT]
> 🐹

To be more specific,
`chomik` is a desktop buddy that uses the graphics of [ChomikBox](https://chomikuj.pl/chomikbox)
without replicating the actual functionality of ChomikBox.
We haven't thought hard about `chomik`'s actual functionality yet,
all we've got right now is that the eating animation (used by ChomikBox for file uploads) will probably delete files.

## Layout

`chomik` is the crate here at the root of the repository.
This is the actual GUI application.

`chomik` uses [`chomik_extract`](./lib/chomik_extract) to pull animations files out of the ChomikBox MSI file provided by Chomikuj.pl.
ChomikBox's resources are not available under an open source license,
so it's necessary for them to be pulled from a legitimate ChomikBox installer at runtime.

`chomik_extract`, in turn, uses [`qrc_parse`](./lib/qrc_parse) to parse the `.anim` files obtained from the MSI file into their constituent image and XML files.
`qrc_parse` is a parser for [Qt resource files](https://doc.qt.io/archives/qt-5.7/qresource.html), as that's all a `.anim` file is.

> [!NOTE]
> Unlike `chomik_extract`, `qrc_parse` is designed to be useful outside of `chomik`.
> It currently `qrc_parse` only parses the Qt resource file format version 1, as that's what ChomikBox uses,
> but you're encouraged to reach out of you're interested in parsing newer versions with `qrc_parse`!

## Licenses

### `qrc_parse` and `chomik_extract`

`qrc_parse` and `chomik_extract` are licensed under the Mozilla Public License,
version 2.0 or (as the license stipulates) any later version.
A copy of the license should be distributed with `qrc_parse` and `chomik_extract`,
located at [`LICENSE-MPL`](./LICENSE-MPL),
or you can obtain one at <https://mozilla.org/MPL/2.0/>.

### `chomik`

`chomik` is free software: you can redistribute it and/or modify it
under the terms of the GNU General Public License as published by the Free Software Foundation,
either version 3 of the License, or (at your option) any later version.

`chomik` is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.
See the GNU General Public License for more details.

You should have received a copy of the GNU General Public License along with `chomik` (located within [LICENSE-GPL](./LICENSE-GPL)).
If not, see <https://www.gnu.org/licenses/>.

Powered by transgender spiders 🕷️ 🕸️ 🏳️‍⚧️.
