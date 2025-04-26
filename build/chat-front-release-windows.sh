#!/bin/bash

# Build the release
dx build --bin=chat-front --release --platform=desktop --features="chat-front chat-out auth-out"

# Navigate to the release directory
cd target/dx/chat-front/release/windows || exit 1

# Rename the folder
rm -rf artilect-chat artilect-chat.zip
mv app artilect-chat

# Create zip archive (using PowerShell since we're on Windows)
powershell -Command "Compress-Archive -Path artilect-chat -DestinationPath artilect-chat.zip -Force"

# Get and display the full path (using Windows path format)
FULLPATH=$(pwd -W)/artilect-chat.zip
echo "Release archive created at: $FULLPATH"
