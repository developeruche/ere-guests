#!/bin/bash
set -e

# Path to your secret key (generated via 'minisign -G')
# Defaults to minisign.key in current dir, change if yours is elsewhere (e.g., ~/.minisign/minisign.key)
SECRET_KEY_PATH="minisign.key" 
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

# Check if Minisign is installed
if ! command -v minisign &> /dev/null; then
    echo "Error: 'minisign' is not installed. Please install it (e.g., 'brew install minisign')."
    exit 1
fi

# Check if Secret Key exists
if [ ! -f "$SECRET_KEY_PATH" ]; then
    echo "Error: Secret key not found at '$SECRET_KEY_PATH'."
    echo "Run 'minisign -G' to generate a keypair if you haven't yet."
    exit 1
fi


mkdir -p "$SIG_DIR"

FILENAME=$(basename "$FILE_PATH")
SIG_FILE="$SIG_DIR/${FILENAME}.minisig"

echo "Signing artifact: $FILE_PATH"

# -S: Sign
# -m: Message (file)
# -s: Secret key path
# -x: Output signature path
# -t: Trusted comment (adds the filename inside the signature for extra security)
minisign -Sm "$FILE_PATH" -s "$SECRET_KEY_PATH" -x "$SIG_FILE" -t "$FILENAME"

echo "✅ Signature created: $SIG_FILE"
echo " To verify this file later, a user needs your public key (minisign.pub) and runs:"
echo " minisign -Vm $FILE_PATH -P <YOUR_PUBLIC_KEY_STRING> -x $SIG_FILE"
echo " OR if they have the pubkey file:"
echo " minisign -Vm $FILE_PATH -p minisign.pub -x $SIG_FILE"




# The ver can be done in two ways, using the .pub file or the key string
# minisign -Vm output/stateless-validator-ethrex-risc0 -P RWSnI6ppYGE0AWGxlA8VQZMLmhRfKC4+BTojGXw4RlOrqRkrIS6ZIbxi -x elf-signatures/stateless-validator-ethrex-risc0.minisig
# or
# minisign -Vm output/stateless-validator-ethrex-risc0 -p minisign.pub -x elf-signatures/stateless-validator-ethrex-risc0.minisig