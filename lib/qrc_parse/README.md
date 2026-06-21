# `qrc_parse`

`qrc_parse` is a parser for [Qt resource files](https://doc.qt.io/archives/qt-5.7/qresource.html).

`qrc_parse` was written for `chomik_extract` to pull the images and XML files from the `.anim` files used by [ChomikBox](https://chomikuj.pl/chomikbox),
but it was designed to be useful outside of this context.
Because of its limited initial scope, some features are missing,
but could be added on request.
If you're interested in
exposing the filesystem tree (rather than the flat list of files that's currently publicly exposed),
localization support,
or parsing QRC file format versions greater than 1 (i.e., QRC files for Qt 5.8.0+),
please reach out!

An example of how to use `qrc_parse` can be found in [`examples/extract.rs`](./examples/extract.rs).

## License

`qrc_parse` is licensed under the Mozilla Public License,
version 2.0 or (as the license stipulates) any later version.
A copy of the license should be distributed with `qrc_parse`,
located at [`../../LICENSE-MPL`](../../LICENSE-MPL),
or you can obtain one at <https://mozilla.org/MPL/2.0/>.
