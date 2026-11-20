# ripcloc

Counts lines of code very fast.

    make check    # fmt, clippy with warnings denied, and every test

## Commits

  - A commit message is one line, `type(scope): ` and a sentence saying what the change does, with no body and no
    attribution trailer such as `Co-Authored-By`. The type is one of `feat`, `fix`, `perf`, `refactor`, `docs`,
    `test` or `chore`.

## Prose

  - Comments and docs are ordinary English sentences. A doc comment says what the code and names cannot, such as an
    invariant, an edge case or a reason, and has no history.
  - No telegraphic style. A phrase is never joined by a colon to a fragment, as in "`p` may not have a default:
    modified in place". Write a sentence, "`p` may not have a default because `f` modifies it in place".
  - No nominal style. That means no bare object relative clause ("the file a request writes"), and no negation put on
    the noun where English puts it on the verb ("names no function"). Write "the file that a request writes" and
    "does not name a function".
  - No "we", "us" or "our".
  - Say what a thing is, never who reads it.
  - No counts of what the reader can already see, such as "all eight arms", which go wrong the moment one is added.
    A number that is the constraint stays.
  - An identifier names what it contains, such as `groups` or `group_count`, never a past-tense verb standing in for a noun.
