# snip

[![Build status](https://github.com/ryanfrishkorn/snip-rs/actions/workflows/build.yml/badge.svg)](https://github.com/ryanfrishkorn/snip-rs/actions/workflows/build.yml)
[![Test status](https://github.com/ryanfrishkorn/snip-rs/actions/workflows/test.yml/badge.svg)](https://github.com/ryanfrishkorn/snip-rs/actions/workflows/test.yml)

![Screenshot](./screenshot.png)

### A simple personal data tool, backed with SQLite, full-text searchable.
The snip utility stores and retrieves text and binary data. A basic document is plain text, and binary data can be attached to the document. Text in the body of the document is automatically analyzed and pushed to a term-matrix stored in SQLite. This allows for very fast searching. Documents have a UUID that is generated upon creation.

## Build / Install

start in project root directory
### build
This will produce a binary in `targets/release/`
```
cargo build --profile release
```

### install
If you are using `rustup`, this will install the binary to your `.cargo/bin/` directory.
```
cargo install --path .
```

## Subcommands / Actions
### add
You can add data from **standard input**, or read from a local file.
The data is stored in a local sqlite database in the user's home directory named `.snip.sqlite3`
```
echo "This is some data I'd like to remember" | snip add
```

```
snip add -f my_quick_note.txt
```

When a new document is added, it generates a new uuid by which it can be referred. This id will be reported upon creation.

```
added snip uuid: 26f15658-a648-4e4b-939e-a0500b2b9677
```

### list
You can list all items with either short or full uuids:
```
sh:~$ snip ls
uuid     name
99bc71c7 Wikipedia - Wren
ca808a9a Interesting files
fff22eb7 Odds of collisions for UUIDs
```

Add the `-l` option to display full id if needed.
```
99bc71c7-573c-403d-a560-996bde675030 Wikipedia - Wren
ca808a9a-ee52-4d1a-aa63-54673241a41b Interesting files
fff22eb7-4b7a-4914-9c1c-7b7c48fe7c26 Odds of collisions for UUIDs
```

### get
Partial ids are always allowed for convenience. Any part of the full uuid can
used, so long as it unique. In practice, the first few characters are often
sufficient.

For non-formatted text, the `fold` command is often useful.
```
sh:~$ snip get 99bc7 | fold -sw 80
uuid: 99bc71c7-573c-403d-a560-996bde675030
name: Wikipedia - Wren
timestamp: 2023-06-30T02:43:28.371895-07:00
----
https://en.wikipedia.org/wiki/Wren

Wrens are a family of brown passerine birds in the predominantly New World
family Troglodytidae. The family includes 88 species divided into 19 genera.
Only the Eurasian wren occurs in the Old World, where, in Anglophone regions,
----
attachments:
uuid                                      bytes name
ccd1627f-1e51-45be-980e-f6169cf49337      22276 Cistothorus_palustris_Iona.jpg
```

### attach
Attach binary files to a document.
```
sh:~$ snip attach add 644d6c1b Cistothorus_palustris_Iona.jpg
attaching files to snip 644d6c1b-c16c-4b85-b245-36b389f87476 Wikipedia - Wren
attached Cistothorus_palustris_Iona.jpg 22276 bytes
```
```
sh:~$ snip attach add ca808a9a "Glacier National Park.pdf"
attaching files to snip ca808a9a-ee52-4d1a-aa63-54673241a41b Interesting files
attached Glacier National Park.pdf 165448 bytes
```

You can list all known attachments.
```
sh:~$ snip attach ls -s
uuid         size name
ccd1627f    22276 Cistothorus_palustris_Iona.jpg
d0d68511   165448 Glacier National Park.pdf
```

You can write an attachment to a local file using the saved name, or a custom name.

```
sh:~$ snip attach write ccd1627f
Cistothorus_palustris_Iona.jpg written -> Cistothorus_palustris_Iona.jpg 22276 bytes
```

```
sh:~$ snip attach write ccd1627f wren_picture.jpg
Cistothorus_palustris_Iona.jpg written -> wren_picture.jpg 22276 bytes
```

### search
All documents are analyzed and stemmed terms are stored in a document term-matrix via SQLite.
The results will show matches and context of the match, along with word counts and total word count of the document.
```
sh:~$ snip search bird nature zealand
Wikipedia - Wren
  99bc71c7 (score: 0.347756, words: 148) [bird: 2, zealand: 1]
    [3-15] "are a family of brown passerine birds in the predominantly New World family"
    [58-70] "has been applied to other, unrelated birds, particularly the New Zealand wrens (Acanthisittidae)"
    [62-74] "other, unrelated birds, particularly the New Zealand wrens (Acanthisittidae) and the Australian wrens"

Interesting files
  ca808a9a (score: 0.266667, words: 23) [natur: 1]
    [11-23] "later. This is mostly information about nature, the environment, and other ecological conerns."
```

## Notes

### database location
The utility honors the environmental variable `SNIP_DB` for the location of the sqlite file.
You can modify this in order to store the database file in a different directory than `HOME`.
