# cm-testshop

A fake Dutch guest checkout, served from five local origins, for developing and
proving a third-party-script scanner against something you own.

It exists because a scanner can pass on invented input and fail on a real page.
This shop is built to contain the hard cases:

- a tag manager that inserts its children after the parse, including one inline
  script that has no URL and appears in no HTML source
- a bundle filename whose content hash changes on every restart, which is what a
  deploy looks like from outside
- cache-buster query parameters that change on every single request
- three real consent platforms' markup, and one house-built banner that cannot
  be handled, to prove a scanner says so instead of guessing
- security headers that can be taken away
- a `robots.txt` that can say no

## Running it

```sh
cargo run
cargo run -- --scenario new-origin
cargo run -- --cmp onetrust
```

The first-party origin defaults to port 8081; the four vendor origins take the
next four ports. `--help` lists every scenario and what each one should produce.

## Why it is public

Scanning rules worth trusting say to develop against a site you own. This is
that site, published so the claim can be checked: the scenarios here are the
cases a scanner has to survive, and anyone can run them.

Nothing in this repository talks to a real shop, and it has no network egress
beyond serving itself on localhost.

## Licence

`Cargo.toml` currently declares `UNLICENSED`, which means all rights reserved.
Public visibility on its own grants no rights to reuse. Pick a licence before
treating this as open source.
