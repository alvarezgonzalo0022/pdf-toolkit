"""The `Command` contract every subcommand follows.

Each command module in `pdf_toolkit.commands` exposes a `command` instance
with:

  * name          -- subcommand name used on the command line
  * help          -- one-line description shown in the top-level help
  * add_arguments -- callable(parser) that adds the command's flags
  * run           -- callable(args) that does the work and returns an exit code
"""


class Command:
    def __init__(self, name, help, add_arguments, run):
        self.name = name
        self.help = help
        self.add_arguments = add_arguments
        self.run = run
