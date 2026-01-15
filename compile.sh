#!/bin/bash
set -e # Exit immediately if a command exits with a non-zero status


# Replace this with the specific tag you need to use
TAG="0.0.16-0f3ef24" 
ALL_ZKVMS=("airbender" "openvm" "pico" "risc0" "sp1" "zisk")


GUEST_PATH_ARG=$1
ZKVM_ARG=$2

if [ -z "$GUEST_PATH_ARG" ]; then
    echo "Usage: $0 <path-to-guest> [zkvm|all]"
    echo "Example: $0 bin/empty sp1"
    echo "Example: $0 bin/empty all"
    exit 1
fi

# Determine which zkVMs to build for
if [ -z "$ZKVM_ARG" ] || [ "$ZKVM_ARG" == "all" ]; then
    TARGET_ZKVMS=("${ALL_ZKVMS[@]}")
else
    # Check if the requested zkVM is in our supported list
    VALID_ZKVM=false
    for vm in "${ALL_ZKVMS[@]}"; do
        if [ "$vm" == "$ZKVM_ARG" ]; then
            VALID_ZKVM=true
            break
        fi
    done
    
    if [ "$VALID_ZKVM" = false ]; then
        echo "Error: '$ZKVM_ARG' is not a valid zkVM. Supported: ${ALL_ZKVMS[*]}"
        exit 1
    fi
    TARGET_ZKVMS=("$ZKVM_ARG")
fi


mkdir -p output


echo "Using Tag: $TAG"
echo "Targeting Guests at: $GUEST_PATH_ARG"

for ZKVM in "${TARGET_ZKVMS[@]}"; do
    IMAGE="ghcr.io/eth-act/ere/ere-compiler-${ZKVM}:${TAG}"
    
    # Extract just the folder name (e.g., "bin/empty" -> "empty") for naming the output
    GUEST_NAME=$(basename "$GUEST_PATH_ARG")
    
    echo "Compiling $GUEST_NAME for $ZKVM..."
    
    # 1. Pull Image (silently unless error, to reduce noise)
    echo "  -> Pulling image..."
    docker pull --platform linux/amd64 "$IMAGE" > /dev/null
    
    # 2. Compile
    echo "  -> Compiling..."
    docker run \
        --platform linux/amd64 \
        --rm \
        -v "$PWD":/ere-guests \
        -v "$PWD/output":/output \
        "$IMAGE" \
        --compiler-kind rust-customized \
        --guest-path "/ere-guests/${GUEST_PATH_ARG}/${ZKVM}" \
        --output-path "/output/${GUEST_NAME}-${ZKVM}"
        
    echo "✅ Success: output/${GUEST_NAME}-${ZKVM}"
done