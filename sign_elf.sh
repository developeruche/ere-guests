#!/bin/bash
set -e

# Optional: Hardcode a specific key ID if you have multiple keys.
# Leave empty to use the default key.
GPG_KEY_ID="" 
SIG_DIR="elf-signatures"

print_usage() {
    echo "Usage: $0 <path-to-elf-file>"
    echo "Example: $0 output/empty-sp1"
    exit 1
}

FILE_PATH=$1

if [ -z "$FILE_PATH" ]; then
    echo "Error: No file path provided."
    print_usage
fi

if [ ! -f "$FILE_PATH" ]; then
    echo "Error: File '$FILE_PATH' not found."
    exit 1
fi

# Check if GPG is installed
if ! command -v gpg &> /dev/null; then
    echo "Error: 'gpg' is not installed. Please install it first."
    exit 1
fi


mkdir -p "$SIG_DIR"


FILENAME=$(basename "$FILE_PATH")

SIG_FILE="$SIG_DIR/${FILENAME}.asc"


echo "Signing artifact: $FILE_PATH"

# Construct the GPG command
CMD="gpg --detach-sign --armor --output $SIG_FILE"

# Add Key ID argument if specified
if [ -n "$GPG_KEY_ID" ]; then
    CMD="$CMD --local-user $GPG_KEY_ID"
fi

# Execute signing
$CMD "$FILE_PATH"

echo "✅ Signature created: $SIG_FILE"

echo " To verify this file later, a user can run:"
echo " gpg --verify $SIG_FILE $FILE_PATH"