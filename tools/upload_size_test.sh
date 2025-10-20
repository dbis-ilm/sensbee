#!/bin/bash

# Sends a JSON payload of a specified size to a given URL.
# The payload is written to a temporary file before the request.
# The script is a test that expects an HTTP 500 status code upon success.
#
# Usage: ./send_json_payload.sh [payload_size_in_kb] [url]
#
# Example: ./send_json_payload.sh 5000 "http://localhost:8080/api/sensors/<sensor_id>/data/ingest"

# --- Define Color Codes ---
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[0;33m'
CYAN='\033[0;36m'
RESET='\033[0m'

# --- Configuration ---
# Payload size in KB, from the first argument or a default value.
payload_size_kb=${1:-5000}

# Target URL, from the second argument or a default value.
url=${2:-"http://localhost:8080/api/sensors/<sensor_id>/data/ingest"}

# --- Script Logic ---

# Creates a temporary file for the JSON payload.
# The trap command ensures the file is deleted on script exit.
payload_file=$(mktemp)
trap "rm -f $payload_file" EXIT

# Exits if the URL is not provided.
if [ -z "$url" ]; then
    echo -e "${YELLOW}Usage:${RESET} $0 [payload_size_in_kb] [url]"
    echo -e "       (e.g., $0 5000 'http://localhost:8080/api/sensors/<sensor_id>/data/ingest')"
    exit 1
fi

echo -e "${CYAN}--- Preparing Payload ---${RESET}"
echo -e "Target URL: ${YELLOW}$url${RESET}"
echo -e "Payload size: ${YELLOW}${payload_size_kb} KB${RESET}"

# Calculates the string length for the JSON object.
string_length=$(( (payload_size_kb * 1024) - 50 ))

# Generates a string of the required length.
payload_string=$(printf 'a%.0s' $(seq 1 $string_length))

# Creates the JSON payload and writes it to the temporary file.
printf '{"name": "test_payload", "size": %d, "data": "%s"}' \
    $payload_size_kb "$payload_string" > "$payload_file"

echo -e "${CYAN}--- Sending Request ---${RESET}"
echo -e "Payload (first 100 characters): ${YELLOW}$(<"$payload_file" head -c 100)...${RESET}"
echo -e "Payload file: ${YELLOW}$payload_file${RESET}"
echo -e "Payload length (bytes): ${YELLOW}$(stat -c%s "$payload_file")${RESET}"

# Sends a POST request using curl and captures the HTTP status code.
# The -d @file syntax tells curl to read the body from the file.
http_code=$(curl -i -vv -o /dev/null -w "%{http_code}" \
    -X POST \
    -H "Content-Type: application/json" \
    -d @"$payload_file" \
    "$url")

echo -e "${CYAN}--- Request Complete ---${RESET}"
echo -e "HTTP Status Code: ${YELLOW}$http_code${RESET}"

# --- Test Assertion ---
# Checks if the HTTP status code is the expected 500.
if [ "$http_code" -eq 500 ]; then
    echo -e "${GREEN}Test Succeeded:${RESET} Expected HTTP 500 status code received."
    exit 0
else
    echo -e "${RED}Test Failed:${RESET} Unexpected HTTP status code."
    echo -e "${RED}Expected: 500${RESET}"
    echo -e "${RED}Actual: $http_code${RESET}"
    exit 1
fi