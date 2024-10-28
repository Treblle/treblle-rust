#!/bin/bash

# Set up colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

# Test validation thresholds
THRESHOLD_HTTP_SUCCESS_RATE=0.95
THRESHOLD_TREBLLE_CAPTURE_RATE=0.95
THRESHOLD_SENSITIVE_DATA_MASK_RATE=1.0
THRESHOLD_BLACKLIST_EFFECTIVENESS=1.0

echo -e "${YELLOW}Validating Axum middleware tests...${NC}"

# Function to check if value meets threshold
check_threshold() {
    local value=$1
    local threshold=$2
    local message=$3

    if (( $(echo "$value >= $threshold" | bc -l) )); then
        echo -e "${GREEN}✓ $message: $value${NC}"
        return 0
    else
        echo -e "${RED}✗ $message: $value (threshold: $threshold)${NC}"
        return 1
    fi
}

# Get metrics from k6 output
success_rate=$(cat k6_metrics.json | jq '.metrics."http_req_failed".values.rate')
treblle_capture_rate=$(cat k6_metrics.json | jq '.metrics."treblle_requests".values.count / .metrics."json_requests".values.count')
sensitive_mask_rate=$(cat k6_metrics.json | jq '.metrics."sensitive_data_masked".values.rate')
blacklist_effectiveness=$(cat k6_metrics.json | jq '.metrics."blacklisted_requests_ignored".values.rate')

# Validate metrics
failed=0

check_threshold $success_rate $THRESHOLD_HTTP_SUCCESS_RATE "HTTP Success Rate" || failed=1
check_threshold $treblle_capture_rate $THRESHOLD_TREBLLE_CAPTURE_RATE "Treblle Capture Rate" || failed=1
check_threshold $sensitive_mask_rate $THRESHOLD_SENSITIVE_DATA_MASK_RATE "Sensitive Data Masking" || failed=1
check_threshold $blacklist_effectiveness $THRESHOLD_BLACKLIST_EFFECTIVENESS "Blacklist Effectiveness" || failed=1

# Check mock Treblle API logs
echo -e "\n${YELLOW}Checking Mock Treblle API logs...${NC}"
TREBLLE_LOGS=$(docker compose logs mock-treblle-api)

# Validate payload structure
INVALID_PAYLOADS=$(echo "$TREBLLE_LOGS" | grep "Invalid payload structure" | wc -l)
if [ $INVALID_PAYLOADS -eq 0 ]; then
    echo -e "${GREEN}✓ All payloads valid${NC}"
else
    echo -e "${RED}✗ Found $INVALID_PAYLOADS invalid payloads${NC}"
    failed=1
fi

# Final result
if [ $failed -eq 0 ]; then
    echo -e "\n${GREEN}All validations passed!${NC}"
    exit 0
else
    echo -e "\n${RED}Validation failed!${NC}"
    exit 1
fi