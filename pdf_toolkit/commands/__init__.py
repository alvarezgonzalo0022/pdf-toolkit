"""Command registry.

Add a command by creating a module in this package that exposes a `command`
instance (see `pdf_toolkit.command.Command`), then import it and `register()`
it below.
"""

from . import merge

COMMANDS = {}


def register(command):
    COMMANDS[command.name] = command
    return command


register(merge.command)
