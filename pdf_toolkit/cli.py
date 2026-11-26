"""Top-level CLI: builds the subcommand parser from the registry and dispatches."""

import argparse

from . import __version__
from .commands import COMMANDS


def build_parser():
    parser = argparse.ArgumentParser(
        prog="pdf-toolkit",
        description="A small multi-command toolkit for PDF files.",
        formatter_class=argparse.ArgumentDefaultsHelpFormatter,
    )
    parser.add_argument(
        "--version", action="version", version=f"%(prog)s {__version__}"
    )
    sub = parser.add_subparsers(dest="command", metavar="COMMAND")

    for name, command in COMMANDS.items():
        p = sub.add_parser(
            name,
            help=command.help,
            formatter_class=argparse.ArgumentDefaultsHelpFormatter,
        )
        command.add_arguments(p)
        p.set_defaults(_run=command.run)

    return parser


def main(argv=None):
    parser = build_parser()
    args = parser.parse_args(argv)
    if not getattr(args, "command", None):
        parser.print_help()
        return 0
    return args._run(args)
