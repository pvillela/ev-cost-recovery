# Archive

Historical documents. Kept as a record of how the software got here, not as a description of what it
is now.

**Every path, file name, module name and command in this directory is expected to be broken**, and
they are left that way deliberately. Repairing a reference here would turn a historical record into
a document describing something that never existed, which is worse than a name that plainly does
not resolve.

`api-conversions.md` is the clearest case. It was written on 2026-08-25 as a journal of design
decisions, and the refactoring of 2026-08-26 reversed one of them and deleted the function another
was about. What is still true of the two conversions is in the `api::io` module documentation,
where it is maintained; what is only true of that one day is here.

If you are looking for how the software works today, start from the root `README.md`.
