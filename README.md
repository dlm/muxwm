# muxwm

A window-manager workflow layer that provides tmux-like project, task, and pin
semantics on top of i3 (and compatible WMs), without replacing the window
manager.

## Random thoughts for the final readme

i3 manages workspaces, but I manage projects. Projects have multiple views, and
I need to switch between them without remembering workspace names. This tool
bridges that gap."

Further off things I want to add:
- muxwm status - show current project, active view, all pins
- muxwm doctor - check for orphaned workspaces, invalid pins, DB consistency
- other observability tooling?


Consider adding some of my i3 nushell scrips to show how this works in practice
Consider adding some discussion of "error recovery" some questions that have
come up are: What happens if:
- i3 dies and restarts?
- Someone manually renames a workspace?
- The DB gets corrupted?


## TODO notes
- currently, if the data folder is missing we get an panic.

- Currently, we take in a config file but don't use it for anything. Currently,
the only configurable option in the tool is the path to the database. Will
there be anything else?  If so a config file would be a good option, else,
something like and env var would be sufficient and env var would be sufficient.
Note that the "better" error handling would also likely use some config.

- Currently, errors are reported to stderr.  It would be nice we had a mode (or
a configuration option) that would make it so that we could use something like
system notifications to report errors.

- Currently, we have a debug mode but we don't really use it.  Either remove it
or actually use it.  Some ways that could be useful and a good learning
experience would be to add some logging so that I get to explore the logging
tooling in the rust world.

- it could be nice to add doc strings to repository functions

- on commands that take names, (add project and add view), it could be nice to
add a little "validation" to the names so that we check that names do not
contain `#`.  As the only user of this tool, I don't think it's a big deal, but
I could imagine that I could fat finger a name and then it would be a pain to
fix.

- currently, we create the db schema in line, but we don't do any sort of
schema versioning or migrations.  At the very least, we should add a schema version, even if it is something like
```sql
const SCHEMA_VERSION: i64 = 1; would be workable
```
And check expected version and db version at startup would be a nice to have.
