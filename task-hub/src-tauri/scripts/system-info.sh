#!/bin/sh
set -eu
# No parameter is evaluated as a command. This portable diagnostic records JSON verbatim.
printf 'System snapshot\nGenerated at: '
date -u '+%Y-%m-%dT%H:%M:%SZ'
uname -sr
printf 'Requested parameters: %s\n' "$TASK_HUB_PARAMETERS"
