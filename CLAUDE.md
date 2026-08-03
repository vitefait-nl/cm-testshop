# cm-testshop: instructions

A fake NL guest checkout served locally. One Cargo project, native target,
edition 2024.

## This is the only site the scanner may do anything to

The conduct rules the scanner enforces say to test against a site you own. This
is that site. Develop against it. Do not develop against a stranger's shop and
do not add a scenario that needs one.

It follows that this shop should keep growing in the direction of being
*harder*, not easier. Every scenario exists because some rule in the scanner
claims to handle a case, and a scenario is how that claim gets tested against a
real browser load rather than a hand-written fixture. When a run against real
shops turns up something the scanner gets wrong, the fix is a scenario here
first and then a rule change, so the rule change has something to be measured
against afterwards.

## Writing style

- No em dashes or en dashes anywhere, including code comments. Use commas,
  colons, semicolons, parentheses or separate sentences.
- Currency as `EUR 99`, not the symbol, in code and plain-text files.
- Dutch-facing copy is written in Dutch, not translated from English.

## This repository is public

It is the only public part of its project. Do not add operator addresses,
domains, hostnames, service accounts, key names or business detail to it. If a
change here needs one of those, it belongs in a private repository instead.
