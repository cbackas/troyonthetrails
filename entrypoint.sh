#!/bin/bash

if [[ $# -gt 0 ]]; then
    # check if the command is "map_service"
    if [ "$1" = "map_service" ]; then
        # run chromium-driver in the background
        /usr/bin/chromedriver --no-sandbox --headless --disable-gpu --port=4444 &
    fi
    dumb-init -- "$@"
fi
