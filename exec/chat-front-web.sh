#!/bin/bash
# cd ../../../
# Load environment variables from .env, stripping out \r
# export $(envsubst < .env | tr -d '\r' | xargs)
# source .env

# Extract just the HOST variable from .env
HOST=$(grep '^HOST=' .env | cut -d '=' -f2 | tr -d '\r')

# Run the frontend with the loaded environment
dx serve --bin=chat-front --addr="$HOST" --port=3000 --platform=web --features="chat-front chat-out auth-out"
# kinda weird but that's how you trim the string i guess...
