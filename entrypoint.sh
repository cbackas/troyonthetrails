#!/bin/bash

if [[ $# -gt 0 ]]; then
    dumb-init -- "$@"
fi
