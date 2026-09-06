# The promise

Every project that ever changed its licence did so after it had something to
lose. The line moved once the old position became expensive, and in most cases
the people who had built on the old position forked rather than follow.

What those projects had in common was declaring the boundary late. cairn has no
users and nothing to sell, so declaring it now costs nothing — which is exactly
what makes it worth something. A promise made when it is inconvenient to break
is worth more than one made when it is convenient to keep.

This text is reproduced word for word in the README and in the manual, and a
test fails if the three ever disagree.

<!-- promise:begin -->
cairn is free software under the GNU General Public Licence, and will remain so.

Everything a single repository can do is part of cairn, and is free: items, the
schema, the board, the roadmap, agents, merging, import and export. No
capability that belongs in cairn will be held back for something else. If
anything ever built beside cairn disappeared tomorrow, no cairn user would be
affected.

cairn does everything a single repository can do, and nothing beyond it. That
is not a policy but a description. A repository cannot see other repositories.
It cannot serve somebody who has not cloned it. It has no notion of who is
permitted to close an item, and it cannot tell you something changed while you
were not looking. Those are not capabilities withheld from cairn; they are
capabilities a directory of files does not have.

So cairn will never grow accounts, authentication, or remotes. The day that
cairn login exists, this promise has been broken.
<!-- promise:end -->

## What this is not

It is not a promise never to charge for anything. Something may one day be
built beside cairn, and it may cost money. It is a promise about where the line
falls and that the line does not move: the difference between a business that
sits *beside* a free tool and one that sits *on top of* it.

It is also not a claim that the excluded things are unimportant. Notifying you
that something changed, showing work across several repositories, deciding who
may close an item — these are real needs. They are needs a program that runs
only when you invoke it, inside one clone, cannot meet. Something else should
meet them, and it should read cairn's [file format](spec/README.md) rather than
link cairn's code.

## Why it is worth having in writing

Because it answers feature requests by principle rather than by mood.

*Can cairn notify me when an item changes?* No. A program that runs when you
invoke it cannot notice anything while you are not running it.

*Can cairn show me work across all my repositories?* No. Run it in each, or
write something that reads the format.

*Can cairn tell me who is allowed to close this?* No. A repository has no
notion of permission; the thing that does is whatever governs writes to it.

Without the boundary written down, each of those is an argument about tiers.
With it, each is a fact about what a repository is.
