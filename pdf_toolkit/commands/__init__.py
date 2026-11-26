"""Command registry.

Add a command by creating a module in this package that exposes a `command`
instance (see `pdf_toolkit.command.Command`), then import it and `register()`
it below.
"""

from . import compress, merge

COMMANDS = {}


def register(command):
    COMMANDS[command.name] = command
    return command


register(compress.command)
register(merge.command)
