#!/bin/sh

CERT_DIR="/app/certs"
CRT_FILE="$CERT_DIR/server.crt"
KEY_FILE="$CERT_DIR/server.key"

# Create certs directory if it doesn't exist
mkdir -p $CERT_DIR

# Check if certificates already exist
if [ ! -f "$CRT_FILE" ] || [ ! -f "$KEY_FILE" ]; then
    echo "Self-signed certificates not found. Generating..."
    openssl req -x509 -nodes -days 365 -newkey rsa:2048 \
        -keyout "$KEY_FILE" \
        -out "$CRT_FILE" \
        -subj "/CN=localhost"
    echo "Self-signed certificates generated."
else
    echo "Certificates already exist. Skipping generation."
fi